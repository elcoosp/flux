//  WireDecodeTests.swift
//  Unit tests for the Flux wire deserializer (Appendix D), FLUX-006 scope item 4.
//
//  Each test hand-builds a byte vector matching the normative layout in the
//  spec and asserts the decoded `FluxFrame`/`VMValue`/`ShadowNode`. There is
//  no shared binary fixture directory for the wire layer, so these are explicit
//  round-trip constructions that pin the byte contract.

import XCTest

@testable import FluxApp

/// Builds little-endian byte vectors tersely for the tests below.
private func u8(_ v: UInt8) -> [UInt8] { [v] }
private func u16(_ v: UInt16) -> [UInt8] { withUnsafeBytes(of: v.littleEndian) { Array($0) } }
private func u32(_ v: UInt32) -> [UInt8] { withUnsafeBytes(of: v.littleEndian) { Array($0) } }
private func u64(_ v: UInt64) -> [UInt8] { withUnsafeBytes(of: v.littleEndian) { Array($0) } }
private func i64(_ v: Int64) -> [UInt8] { withUnsafeBytes(of: v.littleEndian) { Array($0) } }
private func f64(_ v: Double) -> [UInt8] { withUnsafeBytes(of: v.bitPattern.littleEndian) { Array($0) } }
private func cat(_ parts: [[UInt8]]) -> [UInt8] { parts.reduce([], +) }

/// Encodes a `Value` the same way the dev server would (Appendix D §D.5).
private func encValue(_ v: VMValue) -> [UInt8] {
    switch v {
    case .null: return [0x00]
    case let .int(n): return cat([[0x01], i64(n)])
    case let .float(d): return cat([[0x02], f64(d)])
    case let .bool(b): return cat([[0x03], u8(b ? 1 : 0)])
    case let .str(id): return cat([[0x04], u32(id)])
    case let .handlerRef(id): return cat([[0x05], u32(id)])
    case let .list(items): return cat([[0x06], u16(UInt16(items.count)), items.flatMap(encValue)])
    case let .record(fields):
        return cat([[0x07], u16(UInt16(fields.count)),
                    fields.flatMap { u16($0.0) + encValue($0.1) }])
    }
}

final class WireDecodeTests: XCTestCase {
    /// A full Init frame with one Component node carrying two props and one
    /// child node, plus a state seed and an interned string.
    func testInitFrameRoundTrip() throws {
        // Root node (id=1, kind=Component=0, componentId=42).
        let rootNode: [UInt8] = cat([
            u32(1),                 // id
            [0x00],                 // kind = Component
            u32(42),                // component_id
            u16(2),                 // prop_count
            u16(0), encValue(.str(7)),   // prop 0 = Str(7)
            u16(1), encValue(.int(5)),   // prop 1 = Int(5)
            u16(1),                 // child_count
            [0x01], u32(2),         // child 0 = Node(2)
            u16(0),                 // handler_count
            u32(100), u32(0), u32(9), // span
        ])

        // A string table delta: string 7 -> "hello".
        let strings: [UInt8] = cat([
            u32(7), u16(5), Array("hello".utf8),
        ])

        // State seed: signal 9 = Int(123).
        let state: [UInt8] = cat([
            u16(1),
            u32(9), encValue(.int(123)),
        ])

        let body = cat([
            u16(0),            // patch_count
            u16(0),            // handler_count
            u16(1),            // string_count
            rootNode,
            strings,
            state,             // flags has_state_delta (bit 3)
        ])

        let frame = cat([
            u32(FrameDeserializer.magic), // magic
            [0x01],                       // version
            u32(0),                       // seq
            [0x09],                       // flags: full_tree (bit0) | has_state_delta (bit3)
            body,
        ])

        let decoded = try FrameDeserializer.decode(frame)
        XCTAssertEqual(decoded.version, 1)
        XCTAssertEqual(decoded.seq, 0)
        XCTAssertNotNil(decoded.root)
        let root = try XCTUnwrap(decoded.root)
        XCTAssertEqual(root.id, 1)
        XCTAssertEqual(root.kind, .component)
        XCTAssertEqual(root.componentId, 42)
        XCTAssertEqual(root.props.count, 2)
        XCTAssertEqual(root.prop(0), .str(7))
        XCTAssertEqual(root.prop(1), .int(5))
        XCTAssertEqual(root.childCount, 1)
        guard case let .node(childId) = root.children[0] else {
            XCTFail("expected Node child"); return
        }
        XCTAssertEqual(childId, 2)
        XCTAssertEqual(root.span.fileId, 100)
        XCTAssertEqual(root.span.start, 0)
        XCTAssertEqual(root.span.end, 9)

        XCTAssertEqual(decoded.strings.count, 1)
        XCTAssertEqual(decoded.strings[0].stringId, 7)
        XCTAssertEqual(decoded.strings[0].value, "hello")

        XCTAssertEqual(decoded.state.count, 1)
        XCTAssertEqual(decoded.state[0].signalId, 9)
        XCTAssertEqual(decoded.state[0].value, .int(123))
    }

    /// A delta frame carrying a single Update patch with one change and one removal.
    func testUpdatePatch() throws {
        // Update(id=3, changes=[(prop 2, Int(9))], removals=[prop 4]).
        let update: [UInt8] = cat([
            [0x02],                  // tag = Update
            u32(3),                  // id
            u16(1),                  // change_count
            u16(2), encValue(.int(9)),
            u16(1),                  // removal_count
            u16(4),
        ])

        let body = cat([
            u16(1),     // patch_count
            u16(0),     // handler_count
            u16(0),     // string_count
            update,
        ])

        let frame = cat([
            u32(FrameDeserializer.magic),
            [0x01], u32(7), [0x00], // version, seq, flags=delta
            body,
        ])

        let decoded = try FrameDeserializer.decode(frame)
        XCTAssertNil(decoded.root)
        XCTAssertEqual(decoded.patches.count, 1)
        guard case let .update(id, changes, removals) = decoded.patches[0] else {
            XCTFail("expected Update patch"); return
        }
        XCTAssertEqual(id, 3)
        XCTAssertEqual(changes, [Prop(index: 2, value: .int(9))])
        XCTAssertEqual(removals, [4])
    }

    /// A Splice child encodes keyed items (Appendix D §D.4).
    func testSpliceChild() throws {
        let child: [UInt8] = cat([
            [0x02],                       // tag = Splice
            u16(2),                       // item_count
            u64(100), u32(5),             // (key=100, node=5)
            u64(200), u32(6),             // (key=200, node=6)
        ])
        let node: [UInt8] = cat([
            u32(1), [0x01], u32(0),       // id, kind=Primitive, component_id=0
            u16(0),                       // prop_count
            u16(1), child,                // child_count=1, the splice
            u16(0),                       // handler_count
            u32(0), u32(0), u32(0),       // span
        ])
        let body = cat([u16(0), u16(0), u16(0), node])
        let frame = cat([
            u32(FrameDeserializer.magic),
            [0x01], u32(0), [0x01],       // full_tree
            body,
        ])
        let decoded = try FrameDeserializer.decode(frame)
        let root = try XCTUnwrap(decoded.root)
        guard case let .splice(itemCount, items) = root.children[0] else {
            XCTFail("expected Splice child"); return
        }
        XCTAssertEqual(itemCount, 2)
        XCTAssertEqual(items[0].key, 100)
        XCTAssertEqual(items[0].node, 5)
        XCTAssertEqual(items[1].key, 200)
        XCTAssertEqual(items[1].node, 6)
    }

    /// A truncated frame must fail with a precise `WireError`.
    func testTruncatedFrameFails() {
        let frame = cat([u32(FrameDeserializer.magic), [0x01], u32(0), [0x09]]) // cut before body
        XCTAssertThrowsError(try FrameDeserializer.decode(frame)) { error in
            XCTAssertTrue(error is WireError)
        }
    }

    /// A frame without the magic prefix must be rejected.
    func testBadMagicFails() {
        let frame = cat([u32(0xDEADBEEF), [0x01], u32(0), [0x00], u16(0), u16(0), u16(0)])
        XCTAssertThrowsError(try FrameDeserializer.decode(frame)) { error in
            guard let we = error as? WireError,
                  case let .badMagic(_, value) = we else {
                XCTFail("expected badMagic"); return
            }
            XCTAssertEqual(value, 0xDEADBEEF)
        }
    }

    /// A list/record value round-trips through the encoder used by the tests.
    func testValueEncoding() throws {
        let v: VMValue = .list([.int(1), .float(2.5), .bool(true)])
        var r = ByteReader(encValue(v))
        let decoded = try FrameDeserializer.decodeValue(&r)
        XCTAssertEqual(decoded, v)

        let rec: VMValue = .record([(0, .str(3)), (1, .int(9))])
        var r2 = ByteReader(encValue(rec))
        XCTAssertEqual(try FrameDeserializer.decodeValue(&r2), rec)
    }
}
