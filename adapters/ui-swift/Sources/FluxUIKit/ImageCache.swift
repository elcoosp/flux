//  ImageCache.swift
//  FluxUIKit — host-side image cache for the `Image` primitive (FLUX-039).
//
//  Caching is a host concern: the Flux `Image` node only carries a `source`
//  prop (a dev-server asset path or a remote URL); the primitive adds no wire
//  field. This cache is what makes repeated loads cheap and keeps the dev server
//  (or remote origin) from being re-fetched on every frame / reconciliation.
//
//  Design:
//  - **URLCache (disk + memory).** iOS's `URLCache` provides an on-disk and
//    in-memory HTTP cache out of the box, so a repeat load of the same asset is
//    served from the cache without hitting the network. We configure a dedicated
//    `URLCache` (separate from the shared one) so Flux image traffic is bounded
//    and evictable independently.
//  - **Single-flight.** Concurrent requests for the same URL share one in-flight
//    task, so a list of identical images triggers a single network round-trip.
//  - **Failures are not cached**, so a transiently-missing asset can retry on the
//    next load instead of being pinned to a permanent error.
//
//  The cache is an `actor` (Swift 6): its mutable state (the in-flight task map)
//  is isolated, and callers `await` its methods from the `@MainActor` adapters.

import Foundation

/// Host-side image cache for the `Image` primitive (FLUX-039).
///
/// Use `shared` for the app-wide cache, or construct a dedicated instance for a
/// test / scoped session.
public actor ImageCache {
    /// The decoded result of a load request.
    public enum Result: Sendable {
        /// Image bytes decoded from the response body.
        case success(Data)
        /// The asset could not be fetched (missing, offline, decode error).
        case failure
    }

    /// Errors surfaced by `ImageCache`.
    public enum CacheError: Error {
        /// The `source` could not be resolved to a valid URL.
        case invalidURL
    }

    /// Fetches image data for an absolute URL, returning it from the
    /// `URLCache` (disk + memory) when present, otherwise performing a
    /// single-flight network fetch. Concurrent callers for the same URL share
    /// one in-flight task.
    ///
    /// - Parameter url: the absolute asset/remote URL.
    /// - Returns: `.success(data)` on a 2xx response with a body, or
    ///   `.failure` on network/decode error or non-success status.
    public func get(_ url: URL) async -> Result {
        // Serve from the underlying URLCache (memory then disk) without a
        // network round-trip when the response is still fresh.
        if let cached = urlCache.cachedResponse(for: URLRequest(url: url)) {
            return .success(cached.data)
        }

        // Join an in-flight fetch if one already owns this URL (single-flight).
        if let waiter = inFlight[url] {
            return await withCheckedContinuation { cont in
                waiter.append(cont)
            }
        }

        let waiter = InFlightWaiter()
        inFlight[url] = waiter

        let result = await performFetch(url)
        inFlight[url] = nil
        waiter.resumeAll(result)
        return result
    }

    /// Drops all cached responses (memory + disk). Call on a cold reconnect /
    /// "reload from server" so stale bitmaps are not served after a restart.
    public func clear() {
        urlCache.removeAllCachedResponses()
    }

    /// Builds the absolute dev-server asset URL for an `Image` node's `source`
    /// prop. The dev server joins `<source>` onto the project root, so we append
    /// it verbatim to the asset base (e.g. `source = "assets/logo.png"` →
    /// `http://localhost:7332/assets/assets/logo.png`).
    ///
    /// A `source` that is already an absolute `http(s)://` URL is returned
    /// unchanged (remote images need no host rewriting).
    ///
    /// - Parameters:
    ///   - source: the `Image` `source` prop value.
    ///   - assetBase: the dev-server asset base, e.g. `"http://localhost:7332/assets/"`.
    /// - Throws: `CacheError.invalidURL` if `source` cannot form a URL.
    public func resolveURL(_ source: String, assetBase: String) throws -> URL {
        if source.hasPrefix("http://") || source.hasPrefix("https://"),
           let url = URL(string: source) {
            return url
        }
        let base = assetBase.hasSuffix("/") ? assetBase : assetBase + "/"
        let path = source.hasPrefix("/") ? String(source.dropFirst()) : source
        guard let url = URL(string: base + path) else {
            throw CacheError.invalidURL
        }
        return url
    }

    // MARK: - Internals

    private let urlCache: URLCache
    private let session: URLSession
    /// In-flight fetches keyed by URL, enabling single-flight coalescing.
    private var inFlight: [URL: InFlightWaiter] = [:]

    /// Creates a cache with the given capacity (in bytes) for the on-disk and
    /// in-memory stores. Defaults are sized for a dev session (bounded so a long
    /// session cannot grow the disk footprint without limit).
    ///
    /// - Parameters:
    ///   - memoryCapacity: max in-memory bytes; defaults to 32 MB.
    ///   - diskCapacity: max on-disk bytes; defaults to 256 MB.
    ///   - session: the `URLSession` used for fetches; defaults to an ephemeral
    ///     session whose cache is the dedicated `urlCache`. Injected by tests to
    ///     route through a stub protocol.
    ///   - urlCache: the `URLCache` used for disk + memory storage; defaults to a
    ///     fresh instance. Must be the *same* object as `session`'s
    ///     `config.urlCache` for repeat loads to be served from cache.
    public init(memoryCapacity: Int = 32 * 1024 * 1024,
                diskCapacity: Int = 256 * 1024 * 1024,
                session: URLSession? = nil,
                urlCache: URLCache? = nil) {
        let cache = urlCache ?? URLCache(memoryCapacity: memoryCapacity, diskCapacity: diskCapacity)
        self.urlCache = cache
        if let session {
            self.session = session
        } else {
            let config = URLSessionConfiguration.ephemeral
            config.urlCache = cache
            config.requestCachePolicy = .returnCacheDataElseLoad
            config.timeoutIntervalForResource = 5
            self.session = URLSession(configuration: config)
        }
    }

    /// A single shared instance for the app-wide image cache.
    public static let shared = ImageCache()

    private func performFetch(_ url: URL) async -> Result {
        await withCheckedContinuation { (cont: CheckedContinuation<Result, Never>) in
            let task = session.dataTask(with: url) { data, response, _ in
                guard let http = response as? HTTPURLResponse,
                      (200..<300).contains(http.statusCode),
                      let data, !data.isEmpty else {
                    cont.resume(returning: .failure)
                    return
                }
                cont.resume(returning: .success(data))
            }
            task.resume()
        }
    }

    /// A list of continuations waiting on a single in-flight fetch for one URL.
    /// All are resumed with the same result when the fetch completes.
    private final class InFlightWaiter: @unchecked Sendable {
        private var continuations: [CheckedContinuation<Result, Never>] = []
        private let lock = NSLock()

        func append(_ continuation: CheckedContinuation<Result, Never>) {
            lock.lock()
            continuations.append(continuation)
            lock.unlock()
        }

        func resumeAll(_ result: Result) {
            lock.lock()
            let waiters = continuations
            continuations.removeAll()
            lock.unlock()
            for c in waiters { c.resume(returning: result) }
        }
    }
}
