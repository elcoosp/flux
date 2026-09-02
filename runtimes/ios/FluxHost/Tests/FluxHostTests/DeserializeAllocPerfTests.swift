//  DeserializeAllocPerfTests.swift
//  Perf #8 — reduce per-frame deserializer allocation churn.
//
//  The deserializer allocates the `props`/`children`/`handlers` (and value)
//  arrays per frame. For high-frequency frames this is ARC churn. Those
//  collections are value-semantic and ContiguousArray-friendly, so the
//  hot-path arrays use `ContiguousArray` and are converted to `Array` only at
//  the `ShadowNode`/`FluxFrame` boundary. This must not change decoded output.
//
//  The test pins decode correctness and acts as a regression guard: decoding a
//  non-trivial frame many times must stay within a throughput budget.

import XCTest

@testable import FluxHost

private func u16(_ v: UInt16) -> [UInt8] { withUnsafeBytes(of: v.littleEndian) { Array($0) } }
private func u32(_ v: UInt32) -> [UInt8] { withUnsafeBytes(of: v.littleEndian) { Array($0) } }

/// Builds an Init frame with a root carrying `propCount` props and `childCount`
/// children, to exercise the per-node array allocations.
private func makeFrame(propCount: Int, childCount: Int) -> [UInt8] {
    var node: [UInt8] = []
    node += u32(1)        // id
    node += [0x01]        // kind = Primitive
    node += u32(0)        // component_id
    node += u16(UInt16(propCount))
    for i in 0..<propCount {
        node += u16(UInt16(i))
        node += [0x04] + u32(UInt32(100 + i)) // Str(id)
    }
    node += u16(UInt16(childCount))
    for c in 0..<childCount {
        node += [0x01] + u32(UInt32(10 + c)) // child Node(id)
    }
    node += u16(0)        // handler_count
    node += u32(0) + u32(0) + u32(0) // span

    let body = u16(0) + u16(0) + u16(0) + node
    return u32(FrameDeserializer.magic) + [FrameDeserializer.protocolVersion] + u32(0) + [0x01] + body
}

final class DeserializeAllocPerfTests: XCTestCase {
    /// Decoded output is identical regardless of the backing storage used
    /// internally for the per-node arrays.
    func testDecodedStructureUnchanged() throws {
        let frame = makeFrame(propCount: 3, childCount: 2)
        let decoded = try FrameDeserializer.decode(frame)
        let root = try XCTUnwrap(decoded.root)
        XCTAssertEqual(root.id, 1)
        XCTAssertEqual(root.props.count, 3)
        XCTAssertEqual(root.prop(0), .str(100))
        XCTAssertEqual(root.prop(2), .str(102))
        XCTAssertEqual(root.childCount, 2)
        guard case let .node(c0) = root.children[0] else { XCTFail("expected node child"); return }
        XCTAssertEqual(c0, 10)
        guard case let .node(c1) = root.children[1] else { XCTFail("expected node child"); return }
        XCTAssertEqual(c1, 11)
    }

    /// Decoding many frames must stay within a throughput budget — a regression
    /// guard against re-introducing heavy per-frame allocation churn.
    func testDecodeThroughput() throws {
        let frame = makeFrame(propCount: 4, childCount: 4)
        let start = Date()
        let iterations = 5_000
        for _ in 0..<iterations {
            _ = try FrameDeserializer.decode(frame)
        }
        let elapsed = Date().timeIntervalSince(start)
        XCTAssertLessThan(elapsed, 1.0, "decoding \(iterations) frames took \(elapsed)s")
    }
}
