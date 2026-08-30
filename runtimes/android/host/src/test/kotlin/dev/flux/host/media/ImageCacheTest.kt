package dev.flux.host.media

import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * FLUX-039: the host-side image cache must serve a repeat load from memory and
 * collapse concurrent same-URL requests into a single fetch (single-flight), so
 * the dev server is hit at most once per asset URL.
 */
class ImageCacheTest {
    /** A fake fetcher that records how many times each URL was requested. */
    private class FakeFetcher : ImageCache.ImageFetcher {
        val calls = mutableMapOf<String, Int>()
        val lock = Mutex()
        var body: ByteArray = byteArrayOf(1, 2, 3)
        var failFor: Set<String> = emptySet()

        override suspend fun fetch(url: String): ByteArray? {
            lock.withLock { calls[url] = calls.getOrDefault(url, 0) + 1 }
            if (url in failFor) return null
            return body
        }
    }

    @Test
    fun `repeat load hits the cache without a second fetch`() =
        runBlocking {
            val fetcher = FakeFetcher()
            val cache = ImageCache(fetcher, maxEntries = 8)

            val first = cache.get("http://localhost:7332/assets/logo.png")
            val second = cache.get("http://localhost:7332/assets/logo.png")

            assertTrue(first is ImageCache.Result.Success, "first load should succeed")
            assertTrue(second is ImageCache.Result.Success, "second load should hit cache")
            assertEquals(1, fetcher.calls["http://localhost:7332/assets/logo.png"])
        }

    @Test
    fun `concurrent same-URL loads collapse into a single fetch`() =
        runBlocking {
            val fetcher = FakeFetcher()
            val cache = ImageCache(fetcher, maxEntries = 8)

            val results =
                (1..16)
                    .map { async { cache.get("http://localhost:7332/assets/logo.png") } }
                    .awaitAll()

            assertEquals(16, results.size)
            assertTrue(results.all { it is ImageCache.Result.Success })
            assertEquals(1, fetcher.calls["http://localhost:7332/assets/logo.png"])
        }

    @Test
    fun `concurrent distinct-URL loads each fetch once`() =
        runBlocking {
            val fetcher = FakeFetcher()
            val cache = ImageCache(fetcher, maxEntries = 8)

            val results =
                (0..7)
                    .map { i ->
                        async { cache.get("http://localhost:7332/assets/img$i.png") }
                    }.awaitAll()

            assertEquals(8, results.size)
            assertEquals(8, fetcher.calls.size, "each distinct URL fetched once")
            (0..7).forEach { i ->
                assertEquals(1, fetcher.calls["http://localhost:7332/assets/img$i.png"])
            }
        }

    @Test
    fun `failed loads are not cached so they can retry`() =
        runBlocking {
            val fetcher = FakeFetcher()
            fetcher.failFor = setOf("http://localhost:7332/assets/missing.png")
            val cache = ImageCache(fetcher, maxEntries = 8)

            val first = cache.get("http://localhost:7332/assets/missing.png")
            val second = cache.get("http://localhost:7332/assets/missing.png")

            assertTrue(first is ImageCache.Result.Failure, "missing asset should fail")
            assertTrue(second is ImageCache.Result.Failure, "missing asset should fail again")
            assertEquals(
                2,
                fetcher.calls["http://localhost:7332/assets/missing.png"],
                "failure is not cached, so a later load retries",
            )
        }

    @Test
    fun `least-recently-used entries are evicted`() =
        runBlocking {
            val fetcher = FakeFetcher()
            val cache = ImageCache(fetcher, maxEntries = 2)

            // a(1) then b(2): both fit.
            cache.get("http://localhost:7332/assets/a.png")
            cache.get("http://localhost:7332/assets/b.png")
            // c(3) enters → a is the LRU and is evicted (capacity 2).
            cache.get("http://localhost:7332/assets/c.png")
            assertEquals(1, fetcher.calls["http://localhost:7332/assets/a.png"])

            // Touch b so c becomes the LRU, then d evicts c.
            cache.get("http://localhost:7332/assets/b.png")
            cache.get("http://localhost:7332/assets/d.png")

            // `a` and `c` were both evicted and must be re-fetched on access.
            cache.get("http://localhost:7332/assets/a.png")
            cache.get("http://localhost:7332/assets/c.png")
            assertEquals(2, fetcher.calls["http://localhost:7332/assets/a.png"])
            assertEquals(2, fetcher.calls["http://localhost:7332/assets/c.png"])
            // `b` and `d` survived (recently used) → fetched exactly once.
            assertEquals(1, fetcher.calls["http://localhost:7332/assets/b.png"])
            assertEquals(1, fetcher.calls["http://localhost:7332/assets/d.png"])
        }

    @Test
    fun `clear drops all cached entries`() =
        runBlocking {
            val fetcher = FakeFetcher()
            val cache = ImageCache(fetcher, maxEntries = 8)

            cache.get("http://localhost:7332/assets/logo.png")
            cache.clear()
            cache.get("http://localhost:7332/assets/logo.png")

            assertEquals(2, fetcher.calls["http://localhost:7332/assets/logo.png"])
        }
}
