//  InternStringTests.swift
//  TDD coverage for the `InternString` RPC (brittleness 4c) and the release
//  compile-out of the devtools "trace" sink (brittleness 8c).
//
//  These tests pin the new canonical-id contract: derived strings are interned
//  through the dev server and receive a low (< STRING_ID_CANONICAL_CEILING) id;
//  the host never mints a synthetic high-range id locally. They also assert the
//  wire encoding/decoding of the `InternString` / `StringInterned` frames.

import XCTest
import Foundation
@testable import FluxHost

/// A controllable `InternStringTransport` that records every frame the interner
/// sends and automatically delivers a `StringInterned` reply so `intern` resolves
/// deterministically (no resume-before-register race on the main actor).
@MainActor
final class StubTransport: InternStringTransport {
    /// Frames the interner pushed via `send`.
    private(set) var sent: [Data] = []
    /// The next canonical id a reply will carry, incremented per reply.
    var nextId: UInt32 = 1
    /// Sink the test wires to `InternStringClient.handleResponse`, so a simulated
    /// server reply resumes the awaiting `intern` call.
    var onReply: (@MainActor (Data) -> Void)?

    func send(_ bytes: Data) {
        sent.append(bytes)
        replyNext()
    }

    /// Delivers a `StringInterned` reply for the most recent sent frame, using
    /// `nextId` (and advancing it) so repeated inters get distinct canonical ids.
    /// `send` calls this automatically; tests may also call it directly.
    func replyNext() {
        let id = nextId
        nextId &+= 1
        var data = Data()
        data.append(0x58); data.append(0x55); data.append(0x5C); data.append(0x46) // MAGIC
        data.append(1) // version
        data.append(frameKindStringInterned)
        data.append(contentsOf: [UInt8(id & 0xFF), UInt8((id >> 8) & 0xFF),
                                  UInt8((id >> 16) & 0xFF), UInt8((id >> 24) & 0xFF)])
        onReply?(data)
    }
}

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

    // MARK: RPC round-trip + cache

    func testInternResolvesCanonicalIdViaTransport() async {
        let transport = StubTransport()
        let client = InternStringClient(transport: transport)
        transport.onReply = { client.handleResponse($0) }
        let t = Task { await client.intern("hello") }
        // The interner must have sent an InternString frame before resolving.
        XCTAssertEqual(transport.sent.count, 1)
        XCTAssertEqual(decodeStringInternedFrameBehind(transport.sent[0]), "hello")
        transport.replyNext()
        let id = await t.value
        XCTAssertEqual(id, 1)
        XCTAssertLessThan(id, stringIdCanonicalCeiling)
    }

    func testInternCachesCanonicalId() async {
        let transport = StubTransport()
        let client = InternStringClient(transport: transport)
        transport.onReply = { client.handleResponse($0) }
        // Kick the first intern, deliver the server reply, then it must cache.
        let t = Task { await client.intern("repeat") }
        transport.replyNext()
        let first = await t.value
        XCTAssertEqual(first, 1)
        // Second intern of the same text must NOT hit the wire again.
        let second = await client.intern("repeat")
        XCTAssertEqual(second, first)
        XCTAssertEqual(transport.sent.count, 1, "cache hit must not re-send")
    }

    func testOfflineInternDegradesToZero() async {
        // No transport attached: the interner must not trap, returning the
        // offline id 0 (mirrors EmptyStringTable behaviour).
        let client = InternStringClient(transport: BrokenTransport())
        let id = await client.intern("anything")
        XCTAssertEqual(id, 0)
    }

    // MARK: release compile-out

    #if DEBUG
    func testTelemetrySinkSymbolExistsInDebug() {
        // In DEBUG the devtools "trace" sink is compiled; the emit entry point
        // must be callable. (Verified by the call sites in the VM / signal graph.)
        fluxDevtoolsSetSink(nil)
        XCTAssertTrue(true)
    }
    #endif
}

/// A transport that loses its reference immediately, exercising the offline
/// degradation path of `InternStringClient`.
@MainActor
final class BrokenTransport: InternStringTransport {
    func send(_ bytes: Data) {}
}

/// Decodes the UTF-8 payload of a captured `InternString` frame (test helper).
private func decodeStringInternedFrameBehind(_ data: Data) -> String? {
    // magic(4) version(1) kind(1) len(2) payload
    guard data.count >= 8 else { return nil }
    let len = Int(data[6]) | (Int(data[7]) << 8)
    guard data.count >= 8 + len else { return nil }
    return String(bytes: data.subdata(in: 8..<(8 + len)), encoding: .utf8)
}
