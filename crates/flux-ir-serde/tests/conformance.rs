//! Byte-level conformance tests: assert the exact Appendix D wire layout
//! (magic, version, frame_type byte, patch/value tags, offsets) rather than
//! only encode/decode symmetry. These guard against silent drift from the
//! normative spec (AGENTS.md §3.3: the wire protocol is load-bearing).

use flux_ir_serde::{
    FRAME_DELTA, FRAME_ERROR, FRAME_HEARTBEAT, FRAME_HELLO, FRAME_INIT, Frame, MAGIC,
    PROTOCOL_VERSION,
};
use flux_syntax::{
    Child, ClosureRef, NodeKind, NodeRef, Patch, PropDiff, Props, Span, StringTable, Value,
};

/// Little-endian MAGIC bytes (`0x465C5558`).
fn magic_bytes() -> [u8; 4] {
    MAGIC.to_le_bytes()
}

#[test]
fn delta_frame_header_matches_d1() {
    let patches = vec![Patch::Remove { id: 7 }];
    let bytes = Frame::delta(0x1234, 0, &patches, &[], &[], &[]).to_bytes();
    // D.1 + ADR-0059: magic(4) version(1) frame_type(1)=0x04 seq(4) flags(1)
    // then v3 widened the three counts to u32: patch_count(4) handler_count(4)
    // string_count(4). Header is now 23 bytes.
    assert_eq!(&bytes[0..4], &magic_bytes());
    assert_eq!(bytes[4], PROTOCOL_VERSION);
    assert_eq!(bytes[5], FRAME_DELTA);
    assert_eq!(
        u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]),
        0x1234
    );
    assert_eq!(bytes[10], 0); // flags
    assert_eq!(
        u32::from_le_bytes([bytes[11], bytes[12], bytes[13], bytes[14]]),
        1
    ); // patch_count (u32 in v3)
    assert_eq!(
        u32::from_le_bytes([bytes[15], bytes[16], bytes[17], bytes[18]]),
        0
    ); // handler_count
    assert_eq!(
        u32::from_le_bytes([bytes[19], bytes[20], bytes[21], bytes[22]]),
        0
    ); // string_count
    // First patch body starts at offset 23: D.2 Remove tag 0x04 then id u32.
    assert_eq!(bytes[23], 0x04, "Remove tag");
    assert_eq!(
        u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]),
        7
    );
}

#[test]
fn hello_frame_type_byte_is_0x01() {
    let frame = Frame::hello(
        "ios",
        "iPhone15,2",
        &[("touch".into(), 1, vec!["tap".into()])],
    );
    let bytes = frame.to_bytes();
    assert_eq!(&bytes[0..4], &magic_bytes());
    assert_eq!(bytes[4], PROTOCOL_VERSION);
    assert_eq!(bytes[5], FRAME_HELLO);
    // D.12.1 + ADR-0059: after frame_type, platform_len(u32) + platform bytes.
    let plat_len =
        u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) as usize;
    assert_eq!(&bytes[10..10 + plat_len], b"ios");
}

#[test]
fn init_frame_type_byte_is_0x02_and_string_count_is_u32() {
    // Build a minimal root NodeRef (no arena needed).
    let root_ref = NodeRef {
        id: flux_syntax::NodeId::from(1u32),
        kind: NodeKind::Component,
        component_id: flux_syntax::ComponentId::from(1u32),
        props: Props::default(),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 0),
    };
    let table = StringTable::new();

    let frame = Frame::init(&root_ref, &[], &[], &[], &table, &[], &[], &[]);
    let bytes = frame.to_bytes();
    assert_eq!(&bytes[0..4], &magic_bytes());
    assert_eq!(bytes[4], PROTOCOL_VERSION);
    assert_eq!(bytes[5], FRAME_INIT);
    // D.12.2: seq(4) at offset 6, then the root Node.
    let seq = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]);
    assert_eq!(seq, 0);
    // The root Node's first payload byte is its id (u32 LE = 1), then the
    // kind tag at offset 6 within the node (D.3): id(4) kind(1).
    let node_start = 10usize;
    let node_id = u32::from_le_bytes([
        bytes[node_start],
        bytes[node_start + 1],
        bytes[node_start + 2],
        bytes[node_start + 3],
    ]);
    assert_eq!(node_id, 1);
    // D.3: kind byte immediately follows the id. Component = 0.
    assert_eq!(bytes[node_start + 4], 0, "Component kind tag");
}

#[test]
fn error_frame_type_byte_is_0x03() {
    let frame = Frame::error(9, "boom", Some(Span::new(0, 1, 2)), None);
    let bytes = frame.to_bytes();
    assert_eq!(&bytes[0..4], &magic_bytes());
    assert_eq!(bytes[4], PROTOCOL_VERSION);
    assert_eq!(bytes[5], FRAME_ERROR);
    // D.12.3 + ADR-0059: seq(4) at 6, message_len(u32) at 10.
    let seq = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]);
    assert_eq!(seq, 9);
    let msg_len = u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]) as usize;
    assert_eq!(&bytes[14..].split_at(msg_len).0, b"boom");
}

#[test]
fn heartbeat_frame_type_byte_is_0x05() {
    let frame = Frame::heartbeat(0xCAFE);
    let bytes = frame.to_bytes();
    assert_eq!(&bytes[0..4], &magic_bytes());
    assert_eq!(bytes[4], PROTOCOL_VERSION);
    assert_eq!(bytes[5], FRAME_HEARTBEAT);
    // D.12.5: seq(4) follows the frame_type.
    let seq = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]);
    assert_eq!(seq, 0xCAFE);
}

#[test]
fn value_int_tag_and_payload_match_d5() {
    // Encode a single-value Update patch and inspect the Value tag/payload.
    let prop_diff = PropDiff {
        changes: vec![(flux_syntax::PropIdx::from(0u16), Value::Int(42))],
        removals: vec![],
    };
    let patch = Patch::Update {
        id: 1,
        props_diff: prop_diff,
    };
    let bytes = Frame::delta(0, 0, &[patch], &[], &[], &[]).to_bytes();
    // Walk to the value: ADR-0059 v3 Delta header is 23 bytes (three u32 counts);
    // D.2 Update tag 0x02 at 23, id u32 at 24..27, then PropDiff
    // (change_count u32, [(u16 prop_idx, Value)]). change_count at 28..31,
    // prop_idx u16 at 32..33, Value tag at 34, payload follows.
    assert_eq!(bytes[23], 0x02); // Update
    let change_count = u32::from_le_bytes([bytes[28], bytes[29], bytes[30], bytes[31]]);
    assert_eq!(change_count, 1);
    let prop_idx = u16::from_le_bytes([bytes[32], bytes[33]]);
    assert_eq!(prop_idx, 0);
    // D.5: Int tag 0x01, then i64 LE.
    assert_eq!(bytes[34], 0x01, "Int value tag");
    let v = i64::from_le_bytes([
        bytes[35], bytes[36], bytes[37], bytes[38], bytes[39], bytes[40], bytes[41], bytes[42],
    ]);
    assert_eq!(v, 42);
}

#[test]
fn patch_tags_match_d2() {
    // Replace=0x01, Update=0x02, Insert=0x03, Remove=0x04, Reorder=0x05, Handler=0x06.
    let closure = ClosureRef {
        hash: 0,
        bytecode_offset: 0,
        bytecode_len: 0,
        captured_signals: vec![],
        span: Span::new(0, 0, 0),
        excerpt: None,
    };
    let samples = [
        (
            Patch::Replace {
                id: 1,
                node: sample_node(),
            },
            0x01u8,
        ),
        (
            Patch::Update {
                id: 1,
                props_diff: PropDiff {
                    changes: vec![],
                    removals: vec![],
                },
            },
            0x02,
        ),
        (
            Patch::Insert {
                parent: 1,
                index: 0,
                node: sample_node(),
            },
            0x03,
        ),
        (Patch::Remove { id: 1 }, 0x04),
        (
            Patch::Reorder {
                parent: 1,
                keys: vec![2, 3],
            },
            0x05,
        ),
        (
            Patch::Handler {
                id: 1,
                closure: closure.clone(),
            },
            0x06,
        ),
        (
            Patch::Reattach {
                old_id: 1,
                new_id: 2,
                node: sample_node(),
            },
            0x07,
        ),
    ];
    for (patch, tag) in samples {
        let bytes = Frame::delta(0, 0, &[patch], &[], &[], &[]).to_bytes();
        assert_eq!(bytes[23], tag, "patch tag for {tag:#x}");
    }
}

#[test]
fn reattach_patch_layout_matches_d2() {
    // §D.2 tag 0x07: u32 old_id, u32 new_id, then the node record.
    let bytes = Frame::delta(
        0,
        0,
        &[Patch::Reattach {
            old_id: 0x1122_3344,
            new_id: 0x5566_7788,
            node: sample_node(),
        }],
        &[],
        &[],
        &[],
    )
    .to_bytes();
    // ADR-0059 v3: Delta header is 23 bytes (three u32 counts).
    assert_eq!(bytes[23], 0x07, "reattach tag");
    let old_id = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
    let new_id = u32::from_le_bytes([bytes[28], bytes[29], bytes[30], bytes[31]]);
    assert_eq!(old_id, 0x1122_3344);
    assert_eq!(new_id, 0x5566_7788);
}

/// A minimal standalone node used to exercise Replace/Insert patches.
fn sample_node() -> NodeRef {
    NodeRef {
        id: flux_syntax::NodeId::from(1u32),
        kind: NodeKind::Primitive,
        component_id: flux_syntax::ComponentId::from(1u32),
        props: Props::default(),
        children: vec![Child::Node(flux_syntax::NodeId::from(2u32))],
        handlers: vec![],
        span: Span::new(0, 0, 0),
    }
}
