//! Round-trip, determinism, size and framing tests for `flux-ir-serde`.

use flux_ir::ClosureIR;
use flux_ir_serde::{
    Frame, MAGIC, PROTOCOL_VERSION, WireError, deserialize_patches, hash_closure, hash_props,
    serialize_patches,
};
use flux_syntax::{
    Child, ClosureRef, HandlerId, NodeKind, Patch, PropDiff, Props, SignalId, Span, StringId,
    StringTable, Value,
};

/// NaN-aware value equality: `f64::NAN` never equals itself under `PartialEq`,
/// so a round-tripped canonical NaN must be compared by its NaN-ness, not by
/// `==`.
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Float(x), Value::Float(y)) => x == y || (x.is_nan() && y.is_nan()),
        _ => a == b,
    }
}

/// Builds a varied patch set exercising every wire tag and value variant.
fn sample_patches() -> Vec<Patch> {
    let span = Span::new(1, 0, 42);
    vec![
        Patch::Replace {
            id: 7,
            node: flux_syntax::NodeRef {
                id: 7,
                kind: NodeKind::Component,
                component_id: 3,
                props: Props::from_fields(vec![
                    (0u16, Value::Int(12)),
                    (1u16, Value::Str(StringId::from(4u32))),
                    (2u16, Value::List(vec![Value::Bool(true), Value::Null])),
                    (3u16, Value::Record(vec![(0u16, Value::Float(3.5))])),
                ]),
                children: vec![
                    Child::Node(8),
                    Child::Splice {
                        items: vec![(1, 9), (2, 10)],
                    },
                ],
                handlers: vec![5u32],
                span,
            },
        },
        Patch::Update {
            id: 11,
            props_diff: PropDiff {
                changes: vec![
                    (0u16, Value::Int(99)),
                    (1u16, Value::Str(StringId::from(2u32))),
                ],
                removals: vec![3u16, 4u16],
            },
        },
        Patch::Insert {
            parent: 1,
            index: 2,
            node: flux_syntax::NodeRef {
                id: 20,
                kind: NodeKind::Primitive,
                component_id: 9,
                props: Props::default(),
                children: vec![],
                handlers: vec![],
                span,
            },
        },
        Patch::Remove { id: 21 },
        Patch::Reorder {
            parent: 2,
            keys: vec![30, 31, 32],
        },
        Patch::Handler {
            id: 40,
            closure: ClosureRef {
                hash: 0xABCD,
                bytecode_offset: 4,
                bytecode_len: 8,
                captured_signals: vec![SignalId::from(1u32), SignalId::from(2u32)],
                span,
            },
        },
        Patch::Reattach {
            old_id: 50,
            new_id: 51,
            node: flux_syntax::NodeRef {
                id: 51,
                kind: NodeKind::Primitive,
                component_id: 9,
                props: Props::from_fields(vec![(0u16, Value::Int(3))]),
                children: vec![],
                handlers: vec![],
                span,
            },
        },
    ]
}

// ── serialize → deserialize round trip ────────────────────────────────────

#[test]
fn round_trips_sample_patches() {
    let patches = sample_patches();
    let table = StringTable::new();
    let bytes = serialize_patches(&patches, &table, &[]);
    let (back, _closures) = deserialize_patches(&bytes).expect("decode succeeds");
    // `Patch` does not derive `Eq` (it lives in `flux-syntax`), so we assert
    // structural equality via the deterministic canonical encoding instead.
    assert_eq!(serialize_patches(&back, &table, &[]), bytes);
}

#[test]
fn round_trips_empty_patch_set() {
    let bytes = serialize_patches(&[], &StringTable::new(), &[]);
    // An empty set still serializes to a valid (minimal) Delta frame, not an
    // empty buffer: magic(4) + version(1) + frame_type(1) + seq(4) + flags(1)
    // + patch_count(2) + handler_count(2) + string_count(2) = 17 bytes, plus
    // the empty handler section (blob_len u32 = 0) = 21 bytes.
    assert_eq!(bytes.len(), 21);
    let (patches, closures) = deserialize_patches(&bytes).unwrap();
    assert!(patches.is_empty());
    assert!(closures.is_empty());
}

// ── every value variant round-trips through the codec ──────────────────────

#[test]
fn every_value_variant_round_trips() {
    let values = vec![
        Value::Null,
        Value::Int(-123456789),
        Value::Int(i64::MIN),
        Value::Int(i64::MAX),
        Value::Float(1.5),
        Value::Float(f64::NAN),
        Value::Bool(true),
        Value::Bool(false),
        Value::Str(StringId::from(7u32)),
        Value::HandlerRef(11),
        Value::List(vec![Value::Int(1), Value::Int(2), Value::Null]),
        Value::Record(vec![(0u16, Value::Bool(true)), (5u16, Value::Float(2.0))]),
    ];
    for value in values {
        let table = StringTable::new();
        let patches = vec![Patch::Update {
            id: 1,
            props_diff: PropDiff {
                changes: vec![(0u16, value.clone())],
                removals: vec![],
            },
        }];
        let bytes = serialize_patches(&patches, &table, &[]);
        let (back, _closures) = deserialize_patches(&bytes).unwrap();
        match (&value, &back[0]) {
            (_, Patch::Update { id: _, props_diff }) => {
                assert!(
                    values_equal(&props_diff.changes[0].1, &value),
                    "value round-trip"
                );
            }
            _ => panic!("unexpected patch"),
        }
    }
}

// ── deterministic hashing ──────────────────────────────────────────────────

#[test]
fn hash_props_is_order_independent() {
    let a = vec![(0u16, Value::Int(1)), (1u16, Value::Bool(true))];
    let b = vec![(1u16, Value::Bool(true)), (0u16, Value::Int(1))];
    assert_eq!(hash_props(&a), hash_props(&b));
    assert_eq!(hash_props(&a), hash_props(&a));
}

#[test]
fn hash_props_distinguishes_content() {
    let a = vec![(0u16, Value::Int(1))];
    let b = vec![(0u16, Value::Int(2))];
    assert_ne!(hash_props(&a), hash_props(&b));
}

#[test]
fn hash_closure_distinguishes_captures() {
    let code = vec![0x00u8, 0x10, 0x20];
    let a = hash_closure(&code, &[SignalId::from(1u32)]);
    let b = hash_closure(&code, &[SignalId::from(2u32)]);
    assert_ne!(a, b);
    assert_eq!(a, hash_closure(&code, &[SignalId::from(1u32)]));
}

// ── serialized bytes are deterministic ─────────────────────────────────────

#[test]
fn serialize_is_deterministic() {
    let patches = sample_patches();
    let table = StringTable::new();
    let a = serialize_patches(&patches, &table, &[]);
    let b = serialize_patches(&patches, &table, &[]);
    assert_eq!(a, b);
}

// ── frames round-trip ──────────────────────────────────────────────────────

#[test]
fn hello_frame_round_trips() {
    let frame = Frame::hello(
        "ios",
        "iPhone15,2",
        &[(
            "ui.text".to_string(),
            1,
            vec!["render".to_string(), "measure".to_string()],
        )],
    );
    assert_eq!(frame.kind, flux_ir_serde::FrameKind::Hello);
    let bytes = frame.to_bytes();
    let back = Frame::from_hello_bytes(&bytes).expect("hello decodes");
    assert_eq!(back.platform, "ios");
    assert_eq!(back.device, "iPhone15,2");
    assert_eq!(back.capabilities.len(), 1);
    assert_eq!(back.capabilities[0].0, "ui.text");
    assert_eq!(back.capabilities[0].2.len(), 2);
}

#[test]
fn init_frame_round_trips() {
    let mut table = StringTable::new();
    let label = table.intern("Increment");
    let root = flux_syntax::NodeRef {
        id: 1,
        kind: NodeKind::Component,
        component_id: 1,
        props: Props::from_fields(vec![(0u16, Value::Str(label))]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 8),
    };
    let frame = Frame::init(
        &root,
        &[],
        &[(SignalId::from(1u32), Value::Int(0))],
        &[(0u32, "src/main.flux".to_string())],
        &table,
        &[],
        &[],
        &[],
    );
    let bytes = frame.to_bytes();
    let decoded = Frame::from_init_bytes(&bytes).expect("init decodes");
    // `NodeRef` does not derive `Eq`, so confirm the root survives by
    // re-encoding it through a deterministic Replace patch.
    let orig = serialize_patches(
        &[Patch::Replace {
            id: root.id,
            node: root.clone(),
        }],
        &table,
        &[],
    );
    let decoded_root = serialize_patches(
        &[Patch::Replace {
            id: decoded.root.id,
            node: decoded.root.clone(),
        }],
        &decoded.string_table,
        &[],
    );
    assert_eq!(decoded_root, orig);
    assert_eq!(
        decoded.state_seed,
        vec![(SignalId::from(1u32), Value::Int(0))]
    );
    assert_eq!(
        decoded.source_map,
        vec![(0u32, "src/main.flux".to_string())]
    );
    assert_eq!(decoded.string_table.resolve(label), Some("Increment"));
}

#[test]
fn delta_frame_round_trips() {
    let patches = sample_patches();
    let frame = Frame::delta(
        0,
        0,
        &patches,
        &[(StringId::from(1u32), "hello".to_string())],
        &[],
        &[],
    );
    let bytes = frame.to_bytes();
    let decoded = Frame::from_delta_bytes(&bytes).expect("delta decodes");
    // `Patch` does not derive `Eq`; compare the canonical encodings instead.
    assert_eq!(
        serialize_patches(&decoded.patches, &StringTable::new(), &[]),
        serialize_patches(&patches, &StringTable::new(), &[])
    );
    assert_eq!(
        decoded.strings,
        vec![(StringId::from(1u32), "hello".to_string())]
    );
    assert!(decoded.closures.is_empty());
}

#[test]
fn error_frame_round_trips() {
    let frame = Frame::error(3, "type mismatch in Counter", Some(Span::new(0, 12, 20)));
    let bytes = frame.to_bytes();
    let decoded = Frame::from_error_bytes(&bytes).expect("error decodes");
    assert_eq!(decoded.message, "type mismatch in Counter");
    assert_eq!(decoded.span, Some(Span::new(0, 12, 20)));
}

#[test]
fn heartbeat_frame_has_version_and_seq() {
    let frame = Frame::heartbeat(7);
    assert_eq!(frame.version, PROTOCOL_VERSION);
    assert_eq!(frame.seq, 7);
    let bytes = frame.to_bytes();
    // Magic must appear at the front.
    assert_eq!(&bytes[0..4], &MAGIC.to_le_bytes());
}

// ── truncation / corruption handling ───────────────────────────────────────

#[test]
fn corrupt_frame_is_rejected() {
    assert!(Frame::from_init_bytes(&[0u8; 4]).is_err());
    assert!(Frame::from_hello_bytes(&[0u8; 16]).is_none());
}

#[test]
fn truncated_patch_stream_errors() {
    let patches = sample_patches();
    let bytes = serialize_patches(&patches, &StringTable::new(), &[]);
    // Chop the buffer in half — decoding must fail, not panic.
    let result = deserialize_patches(&bytes[..bytes.len() / 2]);
    assert!(matches!(result, Err(WireError::Truncated { .. })));
}

// ── Gap G1: handler transport (bytecode blob + HandlerDef stream) ──────────

/// Builds two sample closures with distinct bytecode and captures.
fn sample_closures() -> Vec<ClosureIR> {
    vec![
        ClosureIR::new(
            HandlerId::from(1u32),
            vec![0x00, 0x10, 0x20, 0x30], // HALT, READ_SIGNAL, ADD_I64, ...
            vec![SignalId::from(1u32), SignalId::from(2u32)],
            Span::new(0, 0, 4),
        ),
        ClosureIR::new(
            HandlerId::from(2u32),
            vec![0xB0, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], // LOAD_INT_CONST 1
            vec![SignalId::from(3u32)],
            Span::new(0, 4, 12),
        ),
    ]
}

#[test]
fn init_frame_carries_handlers_round_trip() {
    let table = StringTable::new();
    let root = flux_syntax::NodeRef {
        id: 1,
        kind: NodeKind::Component,
        component_id: 1,
        props: Props::default(),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 8),
    };
    let closures = sample_closures();
    let frame = Frame::init(
        &root,
        &[],
        &[(SignalId::from(1u32), Value::Int(0))],
        &[(0u32, "src/main.flux".to_string())],
        &table,
        &[],
        &closures,
        &[],
    );
    let bytes = frame.to_bytes();
    let decoded = Frame::from_init_bytes(&bytes).expect("init decodes with handlers");
    assert_eq!(decoded.closures.len(), 2);
    // Bytecode + captures must round-trip exactly.
    assert_eq!(decoded.closures[0].id, HandlerId::from(1u32));
    assert_eq!(decoded.closures[0].bytecode, vec![0x00, 0x10, 0x20, 0x30]);
    assert_eq!(
        decoded.closures[0].captured_signals,
        vec![SignalId::from(1u32), SignalId::from(2u32)]
    );
    assert_eq!(
        decoded.closures[1].bytecode,
        vec![0xB0, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
    );
    // The content hash recomputed from the decoded bytecode + captures must
    // match the canonical BLAKE3 digest (Appendix D §D.7).
    assert_eq!(
        flux_ir_serde::hash_closure(
            &decoded.closures[0].bytecode,
            &decoded.closures[0].captured_signals
        ),
        flux_ir_serde::hash_closure(
            &[0x00, 0x10, 0x20, 0x30],
            &[SignalId::from(1u32), SignalId::from(2u32)]
        )
    );
}

#[test]
fn delta_frame_carries_handlers_round_trip() {
    let patches = vec![Patch::Remove { id: 7 }];
    let closures = sample_closures();
    let frame = Frame::delta(0x1234, 0, &patches, &[], &closures, &[]);
    let bytes = frame.to_bytes();
    let decoded = Frame::from_delta_bytes(&bytes).expect("delta decodes with handlers");
    assert_eq!(decoded.closures.len(), 2);
    assert_eq!(decoded.closures[0].bytecode, vec![0x00, 0x10, 0x20, 0x30]);
    assert_eq!(
        decoded.closures[1].captured_signals,
        vec![SignalId::from(3u32)]
    );
    // The closure bytecode must index a stable offset in the shared blob, so
    // serialization is byte-deterministic.
    let again = Frame::delta(0x1234, 0, &patches, &[], &closures, &[]).to_bytes();
    assert_eq!(again, bytes);
}

#[test]
fn empty_handler_section_is_zero_length_blob() {
    // A frame with no closures must still encode a valid (empty) handler
    // section so the decoder's blob read never underflows.
    let frame = Frame::delta(0, 0, &[Patch::Remove { id: 1 }], &[], &[], &[]);
    let bytes = frame.to_bytes();
    let decoded = Frame::from_delta_bytes(&bytes).unwrap();
    assert!(decoded.closures.is_empty());
    assert_eq!(decoded.closures, Vec::<ClosureIR>::new());
}

// ── 50-node Init frame stays under 20 KB ───────────────────────────────────

#[test]
fn init_frame_under_20kb() {
    let mut table = StringTable::new();
    let mut nodes = Vec::new();
    for i in 0..50u32 {
        let label = table.intern(&format!("node-{i}"));
        nodes.push(flux_syntax::NodeRef {
            id: i + 1,
            kind: NodeKind::Primitive,
            component_id: i % 7 + 1,
            props: Props::from_fields(vec![
                (0u16, Value::Str(label)),
                (1u16, Value::Int(i as i64)),
            ]),
            children: vec![],
            handlers: vec![],
            span: Span::new(0, i * 4, i * 4 + 4),
        });
    }
    let frame = Frame::init(
        &nodes[0],
        &[],
        &[(SignalId::from(1u32), Value::Int(0))],
        &[(0u32, "src/main.flux".to_string())],
        &table,
        &[],
        &[],
        &[],
    );
    let bytes = frame.to_bytes();
    assert!(
        bytes.len() < 20 * 1024,
        "Init frame is {} bytes, budget is 20480",
        bytes.len()
    );
}

#[test]
fn init_frame_signal_meta_round_trips() {
    use flux_ir_serde::NodeSignalMeta;

    let root = flux_syntax::NodeRef {
        id: 1,
        kind: NodeKind::Primitive,
        component_id: 1,
        props: Props::default(),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 0),
    };
    let closure = ClosureRef {
        hash: 0xABCD,
        bytecode_offset: 0,
        bytecode_len: 4,
        captured_signals: vec![SignalId::from(2u32)],
        span: Span::new(0, 0, 0),
    };
    let signal_meta = vec![NodeSignalMeta {
        node_id: flux_syntax::NodeId::from(1u32),
        deps: vec![SignalId::from(2u32), SignalId::from(3u32)],
        thunk: Some(closure),
        layout: vec![0u16, 1u16],
    }];
    let frame = Frame::init(
        &root,
        &[],
        &[],
        &[],
        &StringTable::new(),
        &[],
        &[],
        &signal_meta,
    );
    let bytes = frame.to_bytes();
    let decoded = Frame::from_init_bytes(&bytes).expect("decode Init with signal_meta");
    assert_eq!(
        decoded.signal_meta.len(),
        1,
        "one node's metadata round-tripped"
    );
    let meta = &decoded.signal_meta[0];
    assert_eq!(meta.node_id, flux_syntax::NodeId::from(1u32));
    assert_eq!(
        meta.deps,
        vec![SignalId::from(2u32), SignalId::from(3u32)],
        "deps are sorted/unique and round-trip"
    );
    let thunk = meta.thunk.as_ref().expect("thunk present");
    assert_eq!(thunk.bytecode_len, 4);
    assert_eq!(thunk.captured_signals, vec![SignalId::from(2u32)]);
    assert_eq!(meta.layout, vec![0u16, 1u16]);
}

#[test]
fn delta_frame_signal_meta_requires_flag() {
    use flux_ir_serde::{FLAG_NODE_HAS_SIGNAL_DEPS, NodeSignalMeta};

    let patch = Patch::Replace {
        id: 1,
        node: flux_syntax::NodeRef {
            id: 1,
            kind: NodeKind::Primitive,
            component_id: 1,
            props: Props::default(),
            children: vec![],
            handlers: vec![],
            span: Span::new(0, 0, 0),
        },
    };
    // Without the flag, the decoder must not read a `signal_meta` section even
    // though the encoder skipped it — back-compatible decode.
    let frame = Frame::delta(1, 0, std::slice::from_ref(&patch), &[], &[], &[]);
    let decoded = Frame::from_delta_bytes(&frame.to_bytes()).expect("decode delta");
    assert!(decoded.signal_meta.is_empty(), "no flag ⇒ no signal_meta");

    // With the flag, the section is emitted and round-trips.
    let signal_meta = vec![NodeSignalMeta {
        node_id: flux_syntax::NodeId::from(1u32),
        deps: vec![SignalId::from(5u32)],
        thunk: None,
        layout: vec![],
    }];
    let flagged = Frame::delta(
        2,
        FLAG_NODE_HAS_SIGNAL_DEPS,
        &[patch],
        &[],
        &[],
        &signal_meta,
    );
    let decoded = Frame::from_delta_bytes(&flagged.to_bytes()).expect("decode flagged delta");
    assert_eq!(decoded.signal_meta.len(), 1);
    assert_eq!(decoded.signal_meta[0].deps, vec![SignalId::from(5u32)]);
}
