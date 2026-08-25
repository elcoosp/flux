//  StringTablePerfTests.swift
//  Perf #7 / R4 — O(1) reverse string lookup in `StringTable`.
//
//  `id(for:)` previously did a linear `strings.first(where:)` scan per native
//  event, which is O(n) over the whole table. It must now resolve via a
//  `[String: UInt32]` reverse index in O(1). This test interns 500 strings,
//  then resolves each repeatedly and asserts both correctness and that the
//  lookup stays cheap (a regression guard against re-introducing the scan).

import XCTest
@testable import FluxApp

final class StringTablePerfTests: XCTestCase {
    /// Interning many strings then resolving them by value must hit the O(1)
    /// reverse index, not a linear scan.
    func testReverseLookupIsConstantTime() {
        var table = StringTable()
        let count = 500
        for i in 0..<count {
            table.intern(UInt32(i), "value-\(i)")
        }

        // Every reverse lookup must return the correct id.
        for i in 0..<count {
            XCTAssertEqual(table.id(for: "value-\(i)"), UInt32(i))
        }

        // Repeated lookups of the same value stay O(1): 50k resolutions of a
        // value near the end of the table must complete well under a frame
        // budget (2 ms is the tap-latency budget from the perf review).
        let start = Date()
        for _ in 0..<50_000 {
            _ = table.id(for: "value-\(count - 1)")
        }
        let elapsed = Date().timeIntervalSince(start)
        XCTAssertLessThan(elapsed, 0.05, "reverse lookup of 50k values took \(elapsed)s; expected O(1)")
    }

    /// Forward `intern(id:value)` and reverse `id(for:value)` must agree and
    /// stay consistent after many interleaved operations.
    func testForwardAndReverseConsistency() {
        var table = StringTable()
        table.intern(1, "alpha")
        table.intern(2, "beta")
        XCTAssertEqual(table.id(for: "alpha"), 1)
        XCTAssertEqual(table.id(for: "beta"), 2)
        // A brand-new value interns under a fresh high-range id.
        let fresh = table.id(for: "gamma")
        XCTAssertNotEqual(fresh, 0)
        XCTAssertEqual(table.lookup(fresh), "gamma")
    }
}
