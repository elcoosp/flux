package dev.flux.host.media

import com.sun.net.httpserver.HttpServer
import java.net.InetSocketAddress
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.runBlocking
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test

/**
 * FLUX-039 integration proof: the [ImageCache] performs a *real* HTTP round-trip
 * (not a faked fetcher) against a local server and serves a repeat load from the
 * in-memory cache, so the dev server is hit exactly once per asset URL. This is
 * the host-side analogue of "the cache path is hit on a repeat load" — executed
 * on the plain JVM, so it does not need an emulator.
 */
class ImageCacheHttpTest {
    private lateinit var server: HttpServer
    private lateinit var baseUrl: String
    private val requestCount = AtomicInteger(0)

    @BeforeEach
    fun startServer() {
        server = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        // A tiny valid 1x1 PNG so the body is non-empty and decodable downstream.
        val png =
            byteArrayOf(
                0x89.toByte(), 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
                0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
                0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
                0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4.toByte(),
                0x89.toByte(), 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41,
                0x54, 0x78, 0x9C.toByte(), 0x63, 0x00, 0x01, 0x00, 0x00,
                0x05, 0x00, 0x01, 0x0D.toByte(), 0x0A.toByte(), 0x2D.toByte(), 0x69, 0x00, 0x00, 0x00, 0x00,
                0x49, 0x45, 0x4E, 0x44, 0xAE.toByte(), 0x42, 0x60, 0x82.toByte(),
            )
        server.createContext("/assets") { exchange ->
            // The adapter resolves `source = "assets/logo.png"` against an
            // `assetBase` of `.../assets/`, yielding `.../assets/assets/logo.png`
            // (the dev server joins `source` onto the project root). Serve any
            // `/assets/*` request so the double `/assets` path the contract
            // produces is handled exactly as the real dev server would — except a
            // path containing "nope" simulates a missing asset (404).
            val path = exchange.requestURI.path
            requestCount.incrementAndGet()
            if (path.contains("nope")) {
                exchange.sendResponseHeaders(404, -1)
                exchange.close()
                return@createContext
            }
            exchange.responseHeaders.add("Content-Type", "image/png")
            exchange.sendResponseHeaders(200, png.size.toLong())
            exchange.responseBody.use { it.write(png) }
            exchange.close()
        }
        server.start()
        val port = server.address.port
        baseUrl = "http://127.0.0.1:$port/assets/"
    }

    @AfterEach
    fun stopServer() {
        if (::server.isInitialized) server.stop(0)
    }

    @Test
    fun `repeat load hits the server exactly once`() =
        runBlocking {
            val cache = ImageCache(OkHttpImageFetcher(), maxEntries = 8)

            val first = cache.get(cache.resolveUrl("assets/logo.png", baseUrl))
            val second = cache.get(cache.resolveUrl("assets/logo.png", baseUrl))

            assertTrue(first is ImageCache.Result.Success, "first load should succeed")
            assertTrue(second is ImageCache.Result.Success, "repeat load should hit cache")
            assertEquals(1, requestCount.get(), "server must be hit exactly once")
            val bytes = (second as ImageCache.Result.Success).bytes
            assertTrue(bytes.isNotEmpty(), "decoded bytes must be non-empty")
        }

    @Test
    fun `missing asset reports failure without caching the miss`() =
        runBlocking {
            val cache = ImageCache(OkHttpImageFetcher(), maxEntries = 8)
            val url = cache.resolveUrl("assets/nope.png", baseUrl)
            val first = cache.get(url)
            val second = cache.get(url)
            assertTrue(first is ImageCache.Result.Failure)
            assertTrue(second is ImageCache.Result.Failure)
            assertEquals(2, requestCount.get(), "missing asset retries on each load")
        }
}
