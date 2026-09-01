//  InternStringTests.swift
//  TDD coverage for the `InternString` / `StringInterned` wire-frame encoding
//  and the canonical-id ceiling guard (Appendix D §D.12.6 / §D.12.7, FLUX-084).
//
//  Under the current local-interning model (§AGENTS.md §3.8) the host interns
//  derived strings into the shared `MaterializationStringTable` — no `InternString`
//  RPC to the dev server. These tests pin the wire-frame encoding/decoding
//  helpers (retained for wire compatibility) and the canonical-id guard so the
//  frame format and ceiling contract stay exact.

import XCTest
import Foundation
@testable import FluxHost

@MainActor
final class InternStringTests: XCTestCase {
    // MARK: wire encoding

    func testInternStringFrameEncoding() {
        let bytes = internStringFrameBytes("helloworld")
        let raw = Array(bytes) // Materialize once as [UInt8] to index/compare.
        // magic(4) version(1) kind(1) len(2) payload(10)
        XCTAssertEqual(bytes.count, 4 + 1 + 1 + 2 + 10)
        XCTAssertEqual(Array(raw[0..<4]), [0x58, 0x55, 0x5C, 0x46])
        XCTAssertEqual(raw[4], 1)
        XCTAssertEqual(raw[5], frameKindInternString)
        let len = Int(raw[6]) | (Int(raw[7]) << 8)
        XCTAssertEqual(len, 10)
        let payload = raw[8..<18]
        XCTAssertEqual(String(decoding: payload, as: UTF8.self), "helloworld")
    }

    func testStringInternedFrameDecoding() {
        var data = Data()
        data.append(0x58); data.append(0x55); data.append(0x5C); data.append(0x46)
        data.append(1)
        data.append(frameKindStringInterned)
        data.append(contentsOf: UInt32(42).bytesLE())
        XCTAssertEqual(decodeStringInternedFrame([UInt8](data)), 42)
    }

    func testStringInternedFrameRejectsGarbage() {
        XCTAssertNil(decodeStringInternedFrame([0x58, 0x55])) // too short
        XCTAssertNil(decodeStringInternedFrame([0, 0, 0, 0, 1, frameKindStringInterned, 0, 0, 0, 0])) // bad magic
    }

    // MARK: FLUX-084 — canonical-id ceiling guard

    func testCanonicalIdBelowCeilingPasses() throws {
        // Everyday canonical ids (server assigns densely from zero) must pass.
        XCTAssertNoThrow(try assertCanonicalStringId(0x0000_1234))
        XCTAssertNoThrow(try assertCanonicalStringId(stringIdCanonicalCeiling - 1))
    }

    func testCanonicalIdAtCeilingThrows() {
        // A >=ceiling id is a synthetic fallback that must never be emitted. The
        // guard must reject it so a wire path that synthesizes one fails loud.
        XCTAssertThrowsError(try assertCanonicalStringId(stringIdCanonicalCeiling)) { error in
            XCTAssertTrue(error is StringIdCeilingError)
        }
    }

    func testCanonicalIdAboveCeilingThrows() {
        XCTAssertThrowsError(try assertCanonicalStringId(0xFFFF_FFFF))
    }
}
