package dev.flux.host.media

import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/**
 * Host-side image cache for the `Image` primitive (FLUX-039).
 *
 * Caching is a host concern — the Flux `Image` node only carries a `source`
 * prop (a dev-server asset path or a remote URL); the primitive deliberately
 * adds no wire field. The cache is what makes repeated loads cheap and keeps
 * the dev server (or remote origin) from being re-fetched on every frame /
 * reconciliation.
 *
 * Design:
 * - **LRU eviction.** A bounded in-memory map keyed by absolute URL keeps memory
 *   bounded on long sessions; the most-recently-used entry survives eviction.
 * - **Single-flight.** Concurrent requests for the same URL share one in-flight
 *   fetch (see [inFlight]) so a list of identical images triggers a single
 *   network round-trip, not N.
 * - **Fetch is injectable** via [ImageFetcher] so this class is JVM-testable
 *   without a network or emulator. Production wires [OkHttpImageFetcher].
 * - **Failures are not cached**, so a transiently-missing asset can retry on the
 *   next load without being pinned to a permanent error.
 *
 * The cache is confined to the reactive dispatcher (ADR-0027) at its call
 * sites; the class itself is thread-safe via its own mutex so it may also be
 * touched from the single host thread without extra ceremony.
 *
 * @param fetcher fetches raw bytes for an absolute URL; returns `null` on
 *   network/decode failure (the cache then reports [Result.Failure] and does
 *   not store the miss).
 * @param maxEntries maximum number of decoded images to retain in memory.
 */
public class ImageCache(
    private val fetcher: ImageFetcher,
    private val maxEntries: Int = DEFAULT_MAX_ENTRIES,
) {
    /** Fetches the raw bytes for an absolute URL. Returns `null` on failure. */
    public fun interface ImageFetcher {
        public suspend fun fetch(url: String): ByteArray?
    }

    /** The outcome of a [get] request. */
    public sealed interface Result {
        /** The image bytes are available. */
        public data class Success(
            val bytes: ByteArray,
        ) : Result {
            override fun equals(other: Any?): Boolean =
                other is Success && bytes.contentEquals(other.bytes)

            override fun hashCode(): Int = bytes.contentHashCode()
        }

        /** The asset could not be fetched (missing, offline, decode error). */
        public data object Failure : Result
    }

    private data class Entry(
        val bytes: ByteArray,
        var touch: Long,
    ) {
        override fun equals(other: Any?): Boolean =
            other is Entry && bytes.contentEquals(other.bytes) && touch == other.touch

        override fun hashCode(): Int = 31 * bytes.contentHashCode() + touch.hashCode()
    }

    private val mutex = Mutex()
    private val entries = LinkedHashMap<String, Entry>()
    private val inFlight = LinkedHashMap<String, MutableList<CompletableBytes>>()
    private var clock = 0L

    /**
     * Returns the cached image bytes for [url], fetching (and caching) them on a
     * miss. Concurrent callers for the same [url] share a single fetch.
     *
     * @param url the absolute asset/remote URL.
     */
    public suspend fun get(url: String): Result {
        // Fast path: either a cache hit, or a join slot into an in-flight fetch.
        // The mutex is released before any await so other URLs keep flowing.
        val joinSlot =
            mutex.withLock {
                val hit = entries[url]
                if (hit != null) {
                    hit.touch = ++clock
                    return Result.Success(hit.bytes)
                }
                val waiters = inFlight[url]
                if (waiters != null) {
                    // Another caller already owns the fetch — share its result.
                    val slot = CompletableBytes()
                    waiters.add(slot)
                    slot
                } else {
                    // No entry and no in-flight fetch: claim ownership atomically
                    // so exactly one concurrent caller performs the real fetch.
                    inFlight[url] = mutableListOf()
                    null
                }
            }
        if (joinSlot != null) return joinSlot.await()

        // Slow path: this caller owns the fetch for [url].
        val bytes = fetcher.fetch(url)
        return if (bytes != null) {
            val entry = Entry(bytes, ++clock)
            mutex.withLock {
                entries[url] = entry
                evictIfNeededLocked()
                // Resolve every waiter (including joiners) with the bytes.
                inFlight.remove(url)?.forEach { it.complete(Result.Success(bytes)) }
            }
            Result.Success(bytes)
        } else {
            mutex.withLock {
                // Failure is not cached; wake any waiters with the failure so they
                // can retry on a later load.
                inFlight.remove(url)?.forEach { it.complete(Result.Failure) }
            }
            Result.Failure
        }
    }

    /**
     * Drops every cached entry. Used on a cold reconnect / "reload from server"
     * so stale bitmaps are not served after a host restart.
     */
    public suspend fun clear() {
        mutex.withLock { entries.clear() }
    }

    /**
     * Builds the absolute dev-server asset URL for an `Image` node's `source`
     * prop. The dev server joins `<source>` onto the project root, so we append
     * it verbatim to the asset base (e.g. `source = "assets/logo.png"` →
     * `http://localhost:7332/assets/assets/logo.png`).
     *
     * A `source` that is already an absolute `http(s)://` URL is returned
     * unchanged (remote images need no host rewriting).
     *
     * @param source the `Image` `source` prop value.
     * @param assetBase the dev-server asset base, e.g. `http://localhost:7332/assets/`.
     */
    public fun resolveUrl(
        source: String,
        assetBase: String,
    ): String {
        if (source.startsWith("http://") || source.startsWith("https://")) return source
        val base = if (assetBase.endsWith("/")) assetBase else "$assetBase/"
        val path = if (source.startsWith("/")) source.removePrefix("/") else source
        return base + path
    }

    private fun evictIfNeededLocked() {
        while (entries.size > maxEntries) {
            val oldestKey =
                entries.entries.minByOrNull { it.value.touch }?.key
                    ?: break
            entries.remove(oldestKey)
        }
    }

    private class CompletableBytes {
        // A minimal single-slot await/signal. Implemented with a coroutine
        // CompletableDeferred for correctness without extra dependencies.
        private val deferred =
            kotlinx.coroutines.CompletableDeferred<Result>()

        fun complete(result: Result) = deferred.complete(result)

        suspend fun await(): Result = deferred.await()
    }

    public companion object {
        /** Default maximum number of decoded images retained in memory. */
        public const val DEFAULT_MAX_ENTRIES: Int = 64
    }
}
