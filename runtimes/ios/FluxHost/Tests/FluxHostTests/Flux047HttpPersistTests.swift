//  Flux047HttpPersistTests.swift
//  FLUX-047: native `Http` (cap 14) + `Persist` (cap 15) host bodies.
//
//  These exercise the *package-only* `FluxHost` surface so they run without the
//  `FluxApp` app shell (whose `IOSNativeCapabilityHost` belongs to a different
//  lane). The impls under test are the ones wired by `concreteCapabilityEntries`
//  / `httpPersistEntries` — exactly the bodies shipped to the running app.

import XCTest
import FluxUIKit

@testable import FluxHost

/// FLUX-047: `Persist` (cap 15) synchronous storage + `Http` (cap 14) async park.
final class Flux047HttpPersistTests: XCTestCase {
    // MARK: - Persist (cap 15)

    func testPersistPutThenGetRoundTrips() throws {
        var signals: any SignalStore = InMemorySignals()
        let key: FluxValue = .str(7)
        let value: FluxValue = .list([.int(1), .int(2)])
        let putArgs: FluxValue = .record([(0, key), (1, value)])
        let putCell = try CapabilityRegistry.dev.lookup(15, 1)!(15, 1, putArgs, &signals)
        XCTAssertTrue(signals.read(putCell) is FluxValue.ListVal, "Persist.put returns the stored value")

        let getArgs: FluxValue = .record([(0, key)])
        let getCell = try CapabilityRegistry.dev.lookup(15, 2)!(15, 2, getArgs, &signals)
        XCTAssertEqual(signals.read(getCell), value, "Persist.get returns the value put earlier")
    }

    func testPersistQueryEnumeratesEntries() throws {
        let backend = InMemoryStorageBackend()
        let registry = CapabilityRegistry.makeDev(backend: backend)
        var signals: any SignalStore = InMemorySignals()
        registry.lookup(15, 1)!(15, 1, .record([(0, .str(100)), (1, .int(11))]), &signals)
        registry.lookup(15, 1)!(15, 1, .record([(0, .str(200)), (1, .int(22))]), &signals)
        let queryCell = try registry.lookup(15, 3)!(15, 3, .null, &signals)
        guard case let .list(items) = signals.read(queryCell) else {
            XCTFail("Persist.query returns a list")
            return
        }
        XCTAssertEqual(items.count, 2, "Persist.query lists both stored entries")
    }

    func testPersistDeleteClearsValue() throws {
        var signals: any SignalStore = InMemorySignals()
        let key: FluxValue = .str(7)
        _ = try CapabilityRegistry.dev.lookup(15, 1)!(15, 1, .record([(0, key), (1, .int(99))]), &signals)
        _ = try CapabilityRegistry.dev.lookup(15, 4)!(15, 4, .record([(0, key)]), &signals)
        let getCell = try CapabilityRegistry.dev.lookup(15, 2)!(15, 2, .record([(0, key)]), &signals)
        XCTAssertEqual(signals.read(getCell), .null, "Persist.get is null after Persist.delete")
    }

    // MARK: - Http (cap 14)

    func testHttpFetchReturnsPendingCell() throws {
        var signals: any SignalStore = InMemorySignals()
        // Http.fetch(url) (14,1) allocates a Pending cell and parks it.
        let cell = try CapabilityRegistry.dev.lookup(14, 1)!(14, 1, .record([(0, .str(42))]), &signals)
        XCTAssertEqual(signals.cellState(cell), .pending, "Http.fetch parks the cell as Pending")
    }

    func testHttpGetJsonResolvesToRecordViaResolver() async throws {
        let table = MaterializationStringTable()
        table.store(id: 42, value: "http://example.test/data.json")
        let store = HttpRequestStore()
        let transport = MockHttpTransport(response: #"{"ok":true,"n":3}"#)
        let resolver = CapabilityRegistry.makeHttpResolver(
            store: store,
            transport: transport,
            tableProvider: { table }
        )
        var signals: any SignalStore = InMemorySignals()
        let cell = try CapabilityRegistry.dev.lookup(14, 2)!(14, 2, .record([(0, .str(42))]), &signals)
        XCTAssertEqual(signals.cellState(cell), .pending, "Http.getJson parks the cell")
        let settled = await resolver.resolve(.int(Int64(cell)))
        XCTAssertTrue(settled is FluxValue.RecordVal, "Http.getJson response parses to a RecordVal")
    }

    /// A `HttpTransport` that returns a canned response (no network).
    private struct MockHttpTransport: HttpTransport {
        let response: String
        func request(method: String, url: String, body: String?) -> String { response }
    }
}
