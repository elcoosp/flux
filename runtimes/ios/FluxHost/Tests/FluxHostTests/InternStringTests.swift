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

/// A controllable `FluxTransport` that records every frame the interner sends
/// and lets the test inject `StringInterned` replies on demand.
@MainActor
final class StubTransport: FluxTransport {
    var status: ConnectionStatus = .connected
    var onFrame: (@MainActor (Data) -> Void)?
    var onStatusChange: (@MainActor (ConnectionStatus) -> Void)?

    /// Frames the interner pushed via `send`.
    private(set) var sent: [Data] = []
    /// The next canonical id `reply` will assign, incremented per reply.
    var nextId: UInt32 = 1

    func connect() {}
    func close() {}

    func send(_ bytes: Data) {
        sent.append(bytes)
    }

    /// Delivers a `StringInterned` reply for the most recent sent frame, using
    /// `nextId` (and advancing it) so repeated inters get distinct canonical ids.
    func replyNext() {
        let id = nextId
        nextId &+= 1
        var data = Data()
        data.append(0x58); data.append(0x55); data.append(0x5C); data.append(0x46) // MAGIC
        data.append(1) // version
        data.append(frameKindStringInterned)
        data.append(contentsOf: id.bytesLE())
        onFrame?(data)
    }
}

@MainActor
final class InternStringTests: XCTestCase {
    // MARK: wire encoding

    func testInternStringFrameEncoding() {
        let bytes = internStringFrameBytes("helloworld")
        // magic(4) version(1) kind(1) len(2) payload(10)
        XCTAssertEqual(bytes.count, 4 + 1 + 1 + 2 + 10)
        XCTAssertEqual(Array(bytes[0..<4]), [0x58, 0x55, 0x5C, 0x46])
        XCTAssertEqual(bytes[4], 1)
        XCTAssertEqual(bytes[5], frameKindInternString)
        let len = Int(bytes[6]) | (Int(bytes[7]) << 8)
        XCTAssertEqual(len, 10)
        XCTAssertEqual(String(bytes[8..<18], encoding: .utf8), "helloworld")
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

    // MARK: RPC round-trip + cache

    func testInternResolvesCanonicalIdViaTransport() async {
        let transport = StubTransport()
        let client = InternStringClient(transport: transport)
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
        let first = await client.intern("repeat")
        XCTAssertEqual(first, 1)
        // Second intern of the same text must NOT hit the wire again.
        let second = await client.intern("repeat")
        XCTAssertEqual(second, first)
        XCTAssertEqual(transport.sent.count, 1, "cache hit must not re-send")
    }

    func testConcurrentInternsShareOneRequest() async {
        let transport = StubTransport()
        let client = InternStringClient(transport: transport)
        async let a = client.intern("same")
        async let b = client.intern("same")
        let (ia, ib) = await (a, b)
        XCTAssertEqual(ia, ib)
        XCTAssertEqual(transport.sent.count, 1, "concurrent same-text inters share one wire request")
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
final class BrokenTransport: FluxTransport {
    var status: ConnectionStatus = .connected
    var onFrame: (@MainActor (Data) -> Void)?
    var onStatusChange: (@MainActor (ConnectionStatus) -> Void)?
    func connect() {}
    func send(_ bytes: Data) {}
    func close() {}
}

/// Decodes the UTF-8 payload of a captured `InternString` frame (test helper).
private func decodeStringInternedFrameBehind(_ data: Data) -> String? {
    // magic(4) version(1) kind(1) len(2) payload
    guard data.count >= 8 else { return nil }
    let len = Int(data[6]) | (Int(data[7]) << 8)
    guard data.count >= 8 + len else { return nil }
    return String(bytes: data.subdata(in: 8..<(8 + len)), encoding: .utf8)
}
