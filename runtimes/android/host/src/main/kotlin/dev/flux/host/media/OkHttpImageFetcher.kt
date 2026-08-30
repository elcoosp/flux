package dev.flux.host.media

import kotlinx.coroutines.suspendCancellableCoroutine
import kotlin.coroutines.resume
import okhttp3.Call
import okhttp3.Callback
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import java.io.IOException
import java.util.concurrent.TimeUnit

/**
 * Fetches image bytes over HTTP(S) via OkHttp (FLUX-039 host-side loading).
 *
 * The dev server serves bundled assets from its HTTP asset route
 * (`http://localhost:7332/assets/<src>`); remote `Image` sources are arbitrary
 * URLs. The [ImageCache] provides the LRU memory layer above this fetcher — this
 * fetcher only performs the network round-trip and surfaces failures as `null`
 * so the cache reports [ImageCache.Result.Failure] and does not pin the miss.
 *
 * The call is fully asynchronous (OkHttp `enqueue` + a cancellable coroutine),
 * so it never blocks the reactive dispatcher (the main thread in production per
 * ADR-0027). A short timeout is used because the dev server is local: a slow
 * response indicates a real problem rather than latent latency.
 */
public class OkHttpImageFetcher(
    private val client: OkHttpClient = DEFAULT_CLIENT,
) : ImageCache.ImageFetcher {
    override suspend fun fetch(url: String): ByteArray? =
        suspendCancellableCoroutine { cont ->
            val request = Request.Builder().url(url).get().build()
            val call = client.newCall(request)
            cont.invokeOnCancellation { call.cancel() }
            call.enqueue(
                object : Callback {
                    override fun onFailure(
                        call: Call,
                        e: IOException,
                    ) {
                        if (cont.isActive) cont.resume(null)
                    }

                    override fun onResponse(
                        call: Call,
                        response: Response,
                    ) {
                        try {
                            if (!response.isSuccessful) {
                                if (cont.isActive) cont.resume(null)
                                return
                            }
                            val bytes = response.body?.bytes()
                            if (cont.isActive) cont.resume(bytes)
                        } finally {
                            response.close()
                        }
                    }
                },
            )
        }

    public companion object {
        private val DEFAULT_CLIENT: OkHttpClient =
            OkHttpClient
                .Builder()
                .callTimeout(5_000, TimeUnit.MILLISECONDS)
                .build()
    }
}
