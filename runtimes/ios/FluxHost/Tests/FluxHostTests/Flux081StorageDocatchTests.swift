//  Flux081StorageDocatchTests.swift
//  FLUX-081: iOS `UserDefaultsStorageBackend` — `do/catch` + `StorageError`,
//  drop `synchronize()`.
//
//  The backend must fail *observably*, never silently: a `put` whose value
//  cannot be JSON-encoded must record a `StorageError` and leave the key
//  absent (corrupt-treat-as-absent, matching Android's contract), instead of
//  swallowing the failure with `try?` and storing `nil` so `Storage.set`
//  appears to succeed.

import XCTest
import FluxUIKit

@testable import FluxHost

final class Flux081StorageDocatchTests: XCTestCase {
    override func tearDown() {
        RecoverableErrorReporter.shared.reset()
    }

    /// A `FluxValue.float(.nan)` cannot be represented as JSON (`NaN` is not
    /// valid JSON), so `FluxValueJSON.encode` throws. The backend must record
    /// the error and NOT silently leave a `nil` value that looks like success.
    func testEncodeFailureIsRecordedAndKeyAbsent() {
        let backend = UserDefaultsStorageBackend()
        RecoverableErrorReporter.shared.reset()

        backend.put(1234, .float(.nan))

        XCTAssertNotNil(
            RecoverableErrorReporter.shared.lastRecordedDescription,
            "encode failure must be recorded, not swallowed"
        )
        XCTAssertTrue(
            RecoverableErrorReporter.shared.lastRecordedDescription?.contains("encode failed") ?? false,
            "recorded error names the encode failure"
        )
        XCTAssertNil(backend.get(1234), "key must be absent after a failed encode (no silent no-op)")
    }

    /// A `get` on corrupt bytes records a decode failure and returns `nil`
    /// rather than swallowing the throw with `try?`.
    func testDecodeFailureIsRecordedAndReturnsNil() {
        let suite = "flux.081.decode.\(UUID().uuidString)"
        defer { UserDefaults(suiteName: suite)?.removePersistentDomain(forName: suite) }
        let backend = UserDefaultsStorageBackend(suite: suite)
        RecoverableErrorReporter.shared.reset()

        // Plant a non-JSON payload directly under the backend's namespaced key.
        let defaults = UserDefaults(suiteName: suite)!
        defaults.set(Data("not valid json".utf8), forKey: "flux.storage.7")

        let result = backend.get(7)
        XCTAssertNil(result, "decode failure yields nil, never a crash")
        XCTAssertNotNil(
            RecoverableErrorReporter.shared.lastRecordedDescription,
            "decode failure must be recorded, not swallowed"
        )
        XCTAssertTrue(
            RecoverableErrorReporter.shared.lastRecordedDescription?.contains("decode failed") ?? false,
            "recorded error names the decode failure"
        )
    }

    /// `entries()` must surface a corrupt stored entry instead of `continue`-
    /// silently skipping it: a corrupt entry records a decode failure and the
    /// enumeration still returns the well-formed entries.
    func testEntriesSurfacesCorruptEntryAndKeepsValidOnes() {
        let suite = "flux.081.entries.\(UUID().uuidString)"
        defer { UserDefaults(suiteName: suite)?.removePersistentDomain(forName: suite) }
        let backend = UserDefaultsStorageBackend(suite: suite)
        RecoverableErrorReporter.shared.reset()

        let defaults = UserDefaults(suiteName: suite)!
        // Valid entry (id 1) plus a corrupt entry (id 2).
        defaults.set(try? FluxValueJSON.encode(.int(42)), forKey: "flux.storage.1")
        defaults.set(Data("garbage".utf8), forKey: "flux.storage.2")

        let all = backend.entries()
        XCTAssertEqual(all[1], .int(42), "valid entry is enumerated")
        XCTAssertNil(all[2], "corrupt entry is not enumerated as a value")
        XCTAssertNotNil(
            RecoverableErrorReporter.shared.lastRecordedDescription,
            "corrupt entry decode failure is recorded"
        )
    }
}
