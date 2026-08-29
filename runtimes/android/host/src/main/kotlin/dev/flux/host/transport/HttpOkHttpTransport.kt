package dev.flux.host.transport

import dev.flux.host.vm.HttpTransport
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.util.concurrent.TimeUnit

/**
 * Production `Http` capability transport for the Android host (FLUX-047):
 * implements [HttpTransport] over OkHttp. The executor's async resolver calls
 * [request] (which runs on the reactive dispatcher's IO context) to perform the
 * real network call and return the response body text.
 *
 * @property client the shared OkHttp client; a fresh instance with a sane
 *   timeout is used when omitted.
 * @property connectTimeoutSeconds connection timeout, in seconds.
 * @property readTimeoutSeconds read timeout, in seconds.
 */
public class HttpOkHttpTransport(
    private val client: OkHttpClient =
        OkHttpClient
            .Builder()
            .connectTimeout(15, TimeUnit.SECONDS)
            .readTimeout(15, TimeUnit.SECONDS)
            .build(),
) : HttpTransport {
    private val jsonMediaType = "application/json; charset=utf-8".toMediaType()

    override fun request(
        method: String,
        url: String,
        body: String?,
    ): String {
        val requestBuilder =
            Request
                .Builder()
                .url(url)
                .header("Accept", "application/json")
        when (method.uppercase()) {
            "POST", "PUT", "PATCH", "DELETE" -> {
                val payload = (body ?: "{}").toRequestBody(jsonMediaType)
                requestBuilder.method(method.uppercase(), payload)
            }
            else -> requestBuilder.get()
        }
        val response = client.newCall(requestBuilder.build()).execute()
        return try {
            response.body?.string().orEmpty()
        } finally {
            response.close()
        }
    }
}
