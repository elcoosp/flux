//  AdapterRegistryTests.swift
//  FLUX-017: `AdapterRegistry` — the `ComponentId` → adapter mapping built from
//  the Init frame's string table.
//
//  Mirrors the Kotlin `AdapterRegistryTest` (T-506): pins resolution of interned
//  component ids to adapters, null for unknown ids, coverage of every stdlib
//  component, and kind-vs-component resolution consistency.

import XCTest
@testable import FluxHost
@testable import FluxUIKit

final class AdapterRegistryTests: XCTestCase {
    /// Seeds a registry whose table resolves the given (id → name) pairs.
    private func makeRegistry(_ entries: [(UInt32, String)]) -> AdapterRegistry {
        let table = MaterializationStringTable()
        for (id, name) in entries {
            table.store(id: id, value: name)
        }
        return AdapterRegistry(table: table)
    }

    /// Resolves adapter by component id from the string table.
    func testResolvesAdapterByComponentId() {
        let registry = makeRegistry([
            (100, "Text"),
            (200, "Button"),
            (300, "Column"),
        ])
        XCTAssertEqual(registry.make(for: 100, executor: nil)?.kind, "Text")
        XCTAssertEqual(registry.make(for: 200, executor: nil)?.kind, "Button")
        XCTAssertEqual(registry.make(for: 300, executor: nil)?.kind, "Column")
    }

    /// Returns nil for an unknown component id.
    func testReturnsNilForUnknownComponentId() {
        let registry = makeRegistry([])
        XCTAssertNil(registry.make(for: 999, executor: nil))
    }

    /// Every stdlib component the Init frame can declare resolves to an adapter.
    func testResolvesEveryStdlibComponent() {
        // The seven core stdlib components (column through router), each mapped
        // from a server-interned id to its capitalized adapter name.
        let ids: [UInt32] = [100, 101, 102, 103, 104, 105, 106]
        let kinds = ["Column", "Text", "Button", "Row", "TextInput", "Screen", "Router"]
        let registry = makeRegistry(Array(zip(ids, kinds)))
        for (id, name) in zip(ids, kinds) {
            let adapter = registry.make(for: id, executor: nil)
            XCTAssertNotNil(adapter, "component id \(id) (\(name)) should resolve to an adapter")
        }
    }

    /// Resolving by component id matches resolving by name (same factory path).
    func testComponentIdAndNameResolutionAreConsistent() {
        let registry = makeRegistry([(200, "Text")])
        let viaId = registry.make(for: 200, executor: nil)
        let viaName = registry.make(named: "Text", executor: nil)
        XCTAssertNotNil(viaId)
        XCTAssertNotNil(viaName)
        XCTAssertEqual(viaId?.kind, viaName?.kind)
    }

    /// Each `make` call produces a fresh adapter instance (FLUX-007: no shared
    /// singletons across sibling nodes).
    func testMakeProducesFreshInstancePerCall() {
        let registry = makeRegistry([(1, "Text")])
        let first = registry.make(for: 1, executor: nil)
        let second = registry.make(for: 1, executor: nil)
        XCTAssertNotEqual(ObjectIdentifier(first!), ObjectIdentifier(second!),
            "each resolve must build a fresh adapter instance")
    }
}
