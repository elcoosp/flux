//
//  CapabilityRoundTripTests.swift
//  Real native capability round-trips (LANE-C): `Storage` persistence, `Router`
//  navigation recording, and `Camera` capture — the on-device dev stand-ins for
//  the real native backends (ADR-0045). Runs under the `FluxApp` scheme on the
//  iOS Simulator (the app target can import `FluxHost`).
//
//  The `CapabilityImpl` signature returns a **result-cell signal id** (the
//  unified sync/async contract, ADR-0045); the VM stores that id in the result
//  register and the impl has already written the value into the cell. So these
//  tests assert the returned cell id AND the value written into the cell.

import XCTest
import FluxUIKit

@testable import FluxHost

/// Exercises the full `CapabilityRegistry.dev` surface: `Storage` persistence,
/// `Router` navigation recording, and `Camera` capture — the on-device,
/// synchronous dev stand-ins for the real native backends (ADR-0045).
final class CapabilityRoundTripTests: XCTestCase {
    func testStorageSetThenGetRoundTrips() throws {
        var signals: any SignalStore = InMemorySignals()
        // Storage.set(key=Str(7), value=List[1,2,3]) → cap 2, method 1 returns cell id 95.
        let setArgs = VMValue.record([(0, .str(7)), (1, .list([.int(1), .int(2), .int(3)]))])
        let written = try CapabilityRegistry.dev.lookup(2, 1)!(2, 1, setArgs, &signals)
        XCTAssertEqual(written, 95, "Storage.set returns its result-cell id")

        // Storage.get(key=Str(7)) → cap 2, method 2 exposes the persisted list via cell 95.
        let getArgs = VMValue.record([(0, .str(7))])
        let gotCell = try CapabilityRegistry.dev.lookup(2, 2)!(2, 2, getArgs, &signals)
        XCTAssertEqual(gotCell, 95, "Storage.get returns its result-cell id")
        XCTAssertEqual(signals.read(95), .list([.int(1), .int(2), .int(3)]), "Storage.get returns the persisted value")
    }

    func testRouterNavigateRecordsTarget() throws {
        var signals: any SignalStore = InMemorySignals()
        let out = try CapabilityRegistry.dev.lookup(3, 1)!(3, 1, .str(42), &signals)
        XCTAssertEqual(out, 97, "Router.navigate returns its result-cell id")
        XCTAssertEqual(signals.read(97), .str(42), "Router.navigate records target string id in signal 97")
    }

    func testCameraTakeEchoesForOracleParity() throws {
        var signals: any SignalStore = InMemorySignals()
        let out = try CapabilityRegistry.dev.lookup(1, 1)!(1, 1, .record([(0, .int(7))]), &signals)
        XCTAssertEqual(out, 99, "Camera.take returns its result-cell id (99)")
        XCTAssertEqual(signals.read(99), .int(7), "Camera.take echoes into signal 99 (oracle parity)")
    }

    func testStorageDeleteClearsValue() throws {
        var signals: any SignalStore = InMemorySignals()
        let key = VMValue.record([(0, .str(11))])
        let value = VMValue.record([(0, .str(11)), (1, .list([.int(9)]))])
        _ = try CapabilityRegistry.dev.lookup(2, 1)!(2, 1, value, &signals)
        let beforeCell = try CapabilityRegistry.dev.lookup(2, 2)!(2, 2, key, &signals)
        XCTAssertEqual(beforeCell, 95, "Storage.get returns its result-cell id")
        XCTAssertEqual(signals.read(95), .list([.int(9)]), "value present before delete")
        _ = try CapabilityRegistry.dev.lookup(2, 3)!(2, 3, key, &signals)
        let afterCell = try CapabilityRegistry.dev.lookup(2, 2)!(2, 2, key, &signals)
        XCTAssertEqual(afterCell, 95, "Storage.get returns its result-cell id")
        XCTAssertEqual(signals.read(95), .null, "value cleared after delete")
    }

    /// LANE-C Task 1: `Storage` must persist across registry instances. We build
    /// a `UserDefaultsStorageBackend` over an isolated suite, write via one
    /// registry, drop it, recreate a registry over the SAME suite, and read the
    /// value back — proving it came from disk, not an in-memory cache.
    func testStoragePersistsAcrossRegistryRecreation() throws {
        let suite = "flux.lane-c.storage.\(UUID().uuidString)"
        defer { UserDefaults(suiteName: suite)?.removePersistentDomain(forName: suite) }

        let key = VMValue.record([(0, .str(7))])
        let value = VMValue.record([(0, .str(7)), (1, .list([.int(1), .int(2), .int(3)]))])

        // Write with the first registry (persistent backend).
        var firstSignals: any SignalStore = InMemorySignals()
        let first = CapabilityRegistry.makeDev(backend: UserDefaultsStorageBackend(suite: suite))
        _ = try first.lookup(2, 1)!(2, 1, value, &firstSignals)

        // Drop the registry instance entirely; only the disk suite survives.
        // A second registry over the same suite must observe the persisted value.
        var secondSignals: any SignalStore = InMemorySignals()
        let second = CapabilityRegistry.makeDev(backend: UserDefaultsStorageBackend(suite: suite))
        let gotCell = try second.lookup(2, 2)!(2, 2, key, &secondSignals)
        XCTAssertEqual(gotCell, 95, "Storage.get returns its result-cell id after recreation")
        XCTAssertEqual(
            secondSignals.read(95),
            .list([.int(1), .int(2), .int(3)]),
            "Storage value must survive registry recreation (real persistence)"
        )

        // Delete via the recreated registry; a fresh read must be null on disk.
        _ = try second.lookup(2, 3)!(2, 3, key, &secondSignals)
        var thirdSignals: any SignalStore = InMemorySignals()
        let third = CapabilityRegistry.makeDev(backend: UserDefaultsStorageBackend(suite: suite))
        _ = try third.lookup(2, 2)!(2, 2, key, &thirdSignals)
        XCTAssertEqual(thirdSignals.read(95), .null, "Storage.delete must clear the persisted value")
    }

    // MARK: - LANE-C Task 3: new capabilities (Clipboard / Geolocation)

    func testClipboardSetThenGetRoundTrips() throws {
        var signals: any SignalStore = InMemorySignals()
        // Clipboard.set(value=Str(7)) → cap 4, method 1 returns cell id 94.
        let setCell = try CapabilityRegistry.dev.lookup(4, 1)!(4, 1, .str(7), &signals)
        XCTAssertEqual(setCell, 94, "Clipboard.set returns its result-cell id")
        // Clipboard.get() → cap 4, method 2 exposes the value via cell 93.
        let getCell = try CapabilityRegistry.dev.lookup(4, 2)!(4, 2, .null, &signals)
        XCTAssertEqual(getCell, 93, "Clipboard.get returns its result-cell id")
        XCTAssertEqual(signals.read(93), .str(7), "Clipboard.get returns the value set earlier")
    }

    func testClipboardGetDefaultsToNull() throws {
        var signals: any SignalStore = InMemorySignals()
        let getCell = try CapabilityRegistry.dev.lookup(4, 2)!(4, 2, .null, &signals)
        XCTAssertEqual(getCell, 93, "Clipboard.get returns its result-cell id")
        XCTAssertEqual(signals.read(93), .null, "Clipboard.get defaults to null when nothing set")
    }

    func testGeolocationGetReturnsNullInDevHost() throws {
        var signals: any SignalStore = InMemorySignals()
        // Geolocation.get() → cap 5, method 1. The MLP dev host has no real
        // location provider, so it surfaces a deterministic `null` (no fix).
        let getCell = try CapabilityRegistry.dev.lookup(5, 1)!(5, 1, .null, &signals)
        XCTAssertEqual(getCell, 92, "Geolocation.get returns its result-cell id")
        XCTAssertEqual(signals.read(92), .null, "Geolocation.get is null on the dev host")
    }
}
