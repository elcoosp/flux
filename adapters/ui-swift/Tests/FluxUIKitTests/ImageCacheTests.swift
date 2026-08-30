//  ImageCacheTests.swift
//  FluxUIKitTests — `ImageCache` (FLUX-039).

import XCTest
@testable import FluxUIKit

final class ImageCacheTests: XCTestCase {
    /// A `URLProtocol` stub that records the number of requests per URL and
    /// returns a fixed PNG-like payload (or fails, when configured). State is
    /// guarded by a lock so it is concurrency-safe under Swift 6 (the protocol's
    /// `startLoading` may run on a background session queue).
    private class StubURLProtocol: URLProtocol {
        private static let lock = NSLock()
        // Guarded by `lock`; marked unsafe for the concurrency checker because the
        // lock — not the actor system — provides the synchronization. Tests run
        // serially, and every access goes through a locked accessor below.
        private nonisolated(unsafe) static var _requestCounts: [String: Int] = [:]
        private nonisolated(unsafe) static var _shouldFail: Set<String> = []

        static var requestCounts: [String: Int] {
            lock.lock(); defer { lock.unlock() }
            return _requestCounts
        }
        static func recordRequest(_ url: String) {
            lock.lock(); defer { lock.unlock() }
            _requestCounts[url] = (_requestCounts[url] ?? 0) + 1
        }
        static var shouldFail: Set<String> {
            lock.lock(); defer { lock.unlock() }
            return _shouldFail
        }
        static func setShouldFail(_ urls: Set<String>) {
            lock.lock(); defer { lock.unlock() }
            _shouldFail = urls
        }
        static func reset() {
            lock.lock(); defer { lock.unlock() }
            _requestCounts = [:]
            _shouldFail = []
        }
        static let payload = Data([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])

        override class func canInit(with request: URLRequest) -> Bool { true }
        override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

        override func startLoading() {
            guard let url = request.url else {
                client?.urlProtocol(self, didFailWithError: URLError(.badURL))
                return
            }
            let key = url.absoluteString
            Self.recordRequest(key)
            if Self.shouldFail.contains(key) {
                let response = HTTPURLResponse(
                    url: url, statusCode: 404, httpVersion: nil, headerFields: nil
                )!
                client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
                client?.urlProtocol(self, didLoad: Data())
                client?.urlProtocolDidFinishLoading(self)
                return
            }
            let response = HTTPURLResponse(
                url: url, statusCode: 200, httpVersion: nil, headerFields: nil
            )!
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .allowed)
            client?.urlProtocol(self, didLoad: Self.payload)
            client?.urlProtocolDidFinishLoading(self)
        }

        override func stopLoading() {}
    }

    /// Builds a cache whose session routes through [StubURLProtocol].
    private func makeCache() -> ImageCache {
        let cacheStore = URLCache(memoryCapacity: 1_000_000, diskCapacity: 1_000_000)
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [StubURLProtocol.self]
        // Shared with the cache so repeat loads are served from URLCache (count 1).
        config.urlCache = cacheStore
        config.requestCachePolicy = .returnCacheDataElseLoad
        let session = URLSession(configuration: config)
        StubURLProtocol.reset()
        return ImageCache(
            memoryCapacity: 1_000_000,
            diskCapacity: 1_000_000,
            session: session,
            urlCache: cacheStore
        )
    }

    func testResolveUrlAppendsSourceToAssetBase() async throws {
        let cache = makeCache()
        let url = try await cache.resolveURL(
            "assets/logo.png", assetBase: "http://localhost:7332/assets/"
        )
        XCTAssertEqual(url.absoluteString, "http://localhost:7332/assets/assets/logo.png")
    }

    func testResolveUrlLeavesAbsoluteUrlUnchanged() async throws {
        let cache = makeCache()
        let url = try await cache.resolveURL(
            "https://example.com/x.png", assetBase: "http://localhost:7332/assets/"
        )
        XCTAssertEqual(url.absoluteString, "https://example.com/x.png")
    }

    func testRepeatLoadHitsCacheWithoutSecondFetch() async throws {
        let cache = makeCache()
        let url = try await cache.resolveURL(
            "assets/logo.png", assetBase: "http://localhost:7332/assets/"
        )
        let first = await cache.get(url)
        let second = await cache.get(url)
        guard case .success(let d1) = first, case .success = second else {
            XCTFail("both loads should succeed")
            return
        }
        XCTAssertEqual(d1.count, 8)
        XCTAssertEqual(StubURLProtocol.requestCounts[url.absoluteString], 1,
                       "repeat load must not re-fetch")
    }

    func testFailureIsNotCachedSoItRetries() async throws {
        let cache = makeCache()
        let url = try await cache.resolveURL(
            "assets/missing.png", assetBase: "http://localhost:7332/assets/"
        )
        StubURLProtocol.setShouldFail([url.absoluteString])
        let first = await cache.get(url)
        let second = await cache.get(url)
        XCTAssertTrue(first.isFailure)
        XCTAssertTrue(second.isFailure)
        XCTAssertEqual(StubURLProtocol.requestCounts[url.absoluteString], 2,
                       "failure is not cached, so it retries")
    }
}

extension ImageCache.Result {
    fileprivate var isFailure: Bool {
        switch self {
        case .failure: return true
        case .success: return false
        }
    }
}
