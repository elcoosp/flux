//  StorageBackendTests.swift
//  FluxHostTests — `StorageBackend` contract (FLUX-080).
//
//  Mirrors the Kotlin `StorageBackendTest` (T-506): pins the `put`/`get`/
//  `entries` round-trip, delete-on-nil, and process-restart persistence.
//  Corrupt-entry behavior (decode → nil, delete-on-read) is covered by
//  `Flux081StorageDocatchTests`; here we assert the happy-path contract and
//  cross-instance persistence through `UserDefaults`.

import XCTest
@testable import FluxHost

final class StorageBackendTests: XCTestCase {
    // MARK: - InMemory round-trip

    func testInMemoryPutGetRoundTrip() {
        let backend = InMemoryStorageBackend()
        backend.put(1, .int(42))
        XCTAssertEqual(backend.get(1), .int(42))
    }

    func testInMemoryDeleteClearsKey() {
        let backend = InMemoryStorageBackend()
        backend.put(1, .int(7))
        backend.put(1, nil)
        XCTAssertNil(backend.get(1))
    }

    func testInMemoryEntriesReturnsAllKeys() {
        let backend = InMemoryStorageBackend()
        backend.put(1, .int(10))
        backend.put(2, .str(99))
        backend.put(3, .bool(true))
        let all = backend.entries()
        XCTAssertEqual(all.count, 3)
        XCTAssertEqual(all[1], .int(10))
        XCTAssertEqual(all[2], .str(99))
        XCTAssertEqual(all[3], .bool(true))
    }

    func testInMemoryMissingKeyReturnsNil() {
        let backend = InMemoryStorageBackend()
        XCTAssertNil(backend.get(999))
    }

    func testInMemoryStringRoundTrip() {
        let backend = InMemoryStorageBackend()
        backend.put(5, .str(42))
        XCTAssertEqual(backend.get(5), .str(42))
    }

    // MARK: - UserDefaults persistence (process-restart parity)

    /// A fresh `UserDefaultsStorageBackend` over the same `UserDefaults` suite
    /// must observe a value written by a prior backend instance — mirroring
    /// the Kotlin "round-trip survives process restart" test.
    func testUserDefaultsPersistsAcrossBackendInstances() {
        let suite = "flux.080.rt.\(UUID().uuidString)"
        defer { UserDefaults(suiteName: suite)?.removePersistentDomain(forName: suite) }

        let writer = UserDefaultsStorageBackend(suite: suite)
        writer.put(99, .str(555))

        let reader = UserDefaultsStorageBackend(suite: suite)
        XCTAssertEqual(reader.get(99), .str(555), "value must come from disk, not an in-memory cache")
        XCTAssertEqual(reader.entries()[99], .str(555))
    }

    func testUserDefaultsDeleteRemovesKey() {
        let suite = "flux.080.del.\(UUID().uuidString)"
        defer { UserDefaults(suiteName: suite)?.removePersistentDomain(forName: suite) }

        let backend = UserDefaultsStorageBackend(suite: suite)
        backend.put(7, .int(123))
        XCTAssertEqual(backend.get(7), .int(123))

        backend.put(7, nil)
        XCTAssertNil(backend.get(7), "delete (nil put) must clear the key")
    }

    func testUserDefaultsEntriesEnumeratesAllStored() {
        let suite = "flux.080.ent.\(UUID().uuidString)"
        defer { UserDefaults(suiteName: suite)?.removePersistentDomain(forName: suite) }

        let backend = UserDefaultsStorageBackend(suite: suite)
        backend.put(1, .int(10))
        backend.put(2, .str(3))
        let all = backend.entries()
        XCTAssertEqual(all[1], .int(10))
        XCTAssertEqual(all[2], .str(3))
        XCTAssertEqual(all.count, 2)
    }
}
