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
        let setArgs = FluxValue.record([(0, .str(7)), (1, .list([.int(1), .int(2), .int(3)]))])
        let written = try CapabilityRegistry.dev.lookup(2, 1)!(2, 1, setArgs, &signals)
        XCTAssertEqual(written, 95, "Storage.set returns its result-cell id")

        // Storage.get(key=Str(7)) → cap 2, method 2 exposes the persisted list via cell 95.
        let getArgs = FluxValue.record([(0, .str(7))])
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
        XCTAssertEqual(out, 99, "Camera.takePicture returns its result-cell id (99)")
        XCTAssertEqual(signals.read(99), .int(7), "Camera.takePicture echoes into signal 99 (oracle parity)")
    }

    func testStorageDeleteClearsValue() throws {
        var signals: any SignalStore = InMemorySignals()
        let key = FluxValue.record([(0, .str(11))])
        let value = FluxValue.record([(0, .str(11)), (1, .list([.int(9)]))])
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

        let key = FluxValue.record([(0, .str(7))])
        let value = FluxValue.record([(0, .str(7)), (1, .list([.int(1), .int(2), .int(3)]))])

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

    // MARK: - FLUX-049: permission gate

    /// A `CALL_CAP` to a capability whose required OS permission is denied must
    /// fault as `capabilityDenied` (surfaced as a red banner), never panic.
    /// Drives a real `CALL_CAP` through the VM with a deny-all checker.
    func testDeniedPermissionFaultsCallCapAsCapabilityDenied() throws {
        var signals: any SignalStore = InMemorySignals()
        // CALL_CAP r1, (1,1), args=r0 ; HALT — Camera.takePicture requires .camera.
        let bytecode: [UInt8] = [
            0x90, // CALL_CAP
            1,    // result reg r1
            1, 0, 0, 0, // capId = 1
            1, 0, // methodId = 1
            0,    // args reg r0
            0x00, // HALT
        ]
        var caught: VmError?
        do {
            _ = try FluxBytecodeVM.run(
                bytecode,
                signals: &signals,
                payload: .null,
                capRegistry: .dev,
                permissions: DenyAllPermissionChecker()
            )
            XCTFail("denied cap must fail, never succeed")
        } catch let err as VmError {
            caught = err
        }
        XCTAssertEqual(caught?.kind, .capabilityDenied, "denied cap must surface capabilityDenied")
    }

    /// The gate is fail-closed: an unknown capability id (no permission entry)
    /// is denied the same way, not silently resolved.
    func testUnknownCapabilityIdIsDeniedNotResolved() throws {
        var signals: any SignalStore = InMemorySignals()
        // CALL_CAP r1, (99,1) — capability 99 is not in the host's table.
        let bytecode: [UInt8] = [
            0x90,
            1,
            99, 0, 0, 0, // capId = 99
            1, 0,
            0,
            0x00,
        ]
        var caught: VmError?
        do {
            _ = try FluxBytecodeVM.run(
                bytecode,
                signals: &signals,
                payload: .null,
                capRegistry: .dev,
                permissions: AllowAllPermissionChecker()
            )
            XCTFail("unknown cap must fail")
        } catch let err as VmError {
            caught = err
        }
        XCTAssertEqual(caught?.kind, .capabilityDenied, "unknown cap id is denied (fail-closed)")
    }

    /// A granted permission resolves the call normally (no gate fault).
    func testGrantedPermissionResolvesCallCapNormally() throws {
        var signals: any SignalStore = InMemorySignals()
        // Router.navigate (3,1) requires PermissionKind.none -> always granted.
        let bytecode: [UInt8] = [
            0x90,
            1,
            3, 0, 0, 0, // capId = 3
            1, 0,
            0,
            0x00,
        ]
        // Must not throw — granted cap resolves normally.
        _ = try FluxBytecodeVM.run(
            bytecode,
            signals: &signals,
            payload: .null,
            capRegistry: .dev,
            permissions: AllowAllPermissionChecker()
        )
    }

    // MARK: - FLUX-048 / FLUX-046: escape-valve capabilities

    /// WebView (cap 12) records `src` in signal 82 and needs no OS permission.
    func testWebViewLoadRecordsSrcInSignal82() throws {
        var signals: any SignalStore = InMemorySignals()
        let srcId: UInt32 = 240
        let cell = try CapabilityRegistry.dev.lookup(12, 1)!(12, 1, .record([(0, .int(Int64(srcId)))]), &signals)
        XCTAssertEqual(cell, 82, "WebView.load returns its result-cell id")
        XCTAssertEqual(signals.read(82), .record([(0, .int(Int64(srcId)))]), "WebView.load records the requested src into signal 82")
    }

    /// NativeModule (cap 13) records the requested SDK call in signal 83 and is
    /// gated by `.native`.
    func testNativeModuleInvokeRecordsRequestInSignal83() throws {
        var signals: any SignalStore = InMemorySignals()
        let nameId: UInt32 = 241
        let cell = try CapabilityRegistry.dev.lookup(13, 1)!(13, 1, .record([(0, .int(Int64(nameId)))]), &signals)
        XCTAssertEqual(cell, 83, "NativeModule.invoke returns its result-cell id")
        XCTAssertEqual(signals.read(83), .record([(0, .int(Int64(nameId)))]), "NativeModule.invoke records the requested SDK call into signal 83")
    }

    // MARK: - FLUX-045: six concrete native capabilities

    /// DeepLink.openURL (cap 10) records the requested url into signal 44 through
    /// the live `CapabilityRegistry.dev` registry (the concrete caps must be
    /// reachable from CALL_CAP, not only advertised in the HelloFrame handshake).
    func testDeepLinkOpenURLRecordsUrlInSignal44() throws {
        var signals: any SignalStore = InMemorySignals()
        let urlId: UInt32 = 245
        let args: FluxHost.FluxValue = .record([(UInt16(0), .str(urlId))])
        let cell = try CapabilityRegistry.dev.lookup(10, 1)!(10, 1, args, &signals)
        XCTAssertEqual(cell, 44, "DeepLink.openURL returns its result-cell id")
        XCTAssertEqual(signals.read(44), args, "DeepLink.openURL records the requested url into signal 44")
    }

    /// FileSystem.write then read (caps 9,2 / 9,1) round-trips through the live
    /// `CapabilityRegistry.dev` registry; contents persist under a derived signal id.
    func testFileSystemWriteThenReadRoundTrips() throws {
        var signals: any SignalStore = InMemorySignals()
        let pathId: UInt32 = 246
        let value: FluxHost.FluxValue = .str(999)
        let writeCell = try CapabilityRegistry.dev.lookup(9, 2)!(9, 2, FluxHost.FluxValue.record([(UInt16(0), .str(pathId)), (UInt16(1), value)]), &signals)
        XCTAssertEqual(signals.read(writeCell), value, "FileSystem.write echoes the written value through its result cell")
        XCTAssertEqual(signals.read(UInt32(900_000) + pathId), value, "FileSystem.write persists the value under the derived signal id")

        let readCell = try CapabilityRegistry.dev.lookup(9, 1)!(9, 1, FluxHost.FluxValue.record([(UInt16(0), .str(pathId))]), &signals)
        XCTAssertEqual(signals.read(readCell), value, "FileSystem.read returns the written value")
    }
}
