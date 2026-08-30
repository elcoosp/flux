//! Generates the committed FLUX-085 wire-fuzz **seed corpus** of *real* frames.
//!
//! Every seed is produced through the production `Frame::*` round-trip API
//! (exactly what the dev server emits), so the bytes match real wire shapes
//! rather than hand-crafted approximations. `cargo fuzz` automatically seeds
//! from `fuzz/corpus/<target>`; this tool writes each frame there under a
//! `real_<name>` filename so the harness starts every run from known-good +
//! known-boundary shapes, not just random bytes. (The filename is irrelevant
//! to libFuzzer — it feeds the file *contents* as seeds — but the `real_` prefix
//! keeps the curated set reviewable and distinct from the auto-generated
//! random corpus.)
//!
//! Run: `cargo +nightly run --example gen_corpus` (from the `fuzz/` crate).

use flux_ir_serde::{
    FRAME_DELTA, FRAME_HEARTBEAT, FRAME_INIT, Frame, MAGIC, PROTOCOL_VERSION,
    STRING_ID_CANONICAL_CEILING,
};
use flux_syntax::{
    Child, ClosureRef, HandlerId, NodeKind, Patch, PropDiff, Props, SignalId, SourceExcerpt, Span,
    StringId, StringTable, Value,
};
use std::io::Write;
use std::path::Path;

/// Writes `bytes` to `dir/real_<name>` (the curated, reviewable seed filename),
/// skipping if an identical seed already exists.
fn emit(dir: &Path, name: &str, bytes: &[u8]) {
    let path = dir.join(format!("real_{name}"));
    if path.exists() {
        return;
    }
    let mut f = std::fs::File::create(&path).expect("create seed");
    f.write_all(bytes).expect("write seed");
    eprintln!("  seed {:>10} bytes  real_{}", bytes.len(), name);
}

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("corpus")
        .join("decode_frame");
    std::fs::create_dir_all(&dir).expect("create corpus dir");
    eprintln!("writing curated wire seeds to {}", dir.display());

    // ── Hello (handshake, no token + with token) ────────────────────────────
    let hello = Frame::hello(
        "ios",
        "iPhone17,2",
        &[
            ("storage".into(), 1, vec!["sync".into()]),
            ("http".into(), 1, vec!["async".into()]),
            ("persist".into(), 1, vec!["sync".into()]),
        ],
    )
    .to_bytes();
    emit(&dir, "hello_ios", &hello);

    let hello_token = Frame::hello_with_token(
        "android",
        "Pixel9",
        &[("storage".into(), 1, vec!["sync".into()])],
        "dev-pairing-token-0123",
    )
    .to_bytes();
    emit(&dir, "hello_android_token", &hello_token);

    // ── Init (full tree, nested component + primitive, string-table + caps) ──
    let mut table = StringTable::new();
    let label = table.intern("Increment");
    let title = table.intern("Counter");
    let root = flux_syntax::NodeRef {
        id: 1,
        kind: NodeKind::Component,
        component_id: 1,
        props: Props::from_fields(vec![
            (0u16, Value::Str(label)),
            (1u16, Value::Int(0)),
            (
                2u16,
                Value::List(vec![Value::Bool(true), Value::Null, Value::Float(1.5)]),
            ),
            (
                3u16,
                Value::Record(vec![(0u16, Value::Str(title)), (1u16, Value::Int(7))]),
            ),
        ]),
        children: vec![
            Child::Node(2),
            Child::Splice {
                items: vec![(0, 3), (1, 4)],
            },
        ],
        handlers: vec![5u32],
        span: Span::new(0, 0, 42),
    };
    let child = flux_syntax::NodeRef {
        id: 2,
        kind: NodeKind::Primitive,
        component_id: 9,
        props: Props::from_fields(vec![(0u16, Value::Str(label))]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 10, 20),
    };
    let init = Frame::init(
        &root,
        &[child],
        &[
            (SignalId::from(1u32), Value::Int(0)),
            (SignalId::from(2u32), Value::Float(2.5)),
            (SignalId::from(3u32), Value::Str(title)),
        ],
        &[(0u32, "src/main.flux".to_string())],
        &table,
        &[(flux_syntax::ComponentId::from(1u32), "Counter".to_string())],
        &[],
        &[],
    )
    .to_bytes();
    emit(&dir, "init_counter_tree", &init);

    // ── Init with a handler closure (Gap G1 bytecode transport) ──────────────
    let span = Span::new(0, 0, 8);
    let closure = flux_ir::ClosureIR {
        id: HandlerId::from(1u32),
        // Minimal but *valid* (validate_bytecode-accepting) bytecode: a single
        // `RET` opcode (0x00) is a complete, well-formed program.
        bytecode: vec![0x00],
        captured_signals: vec![SignalId::from(1u32)],
        span,
        excerpt: None,
        param_types: vec![],
        return_type: flux_syntax::TypeId::from(0u32),
    };
    let init_closure = Frame::init(&root, &[], &[], &[], &table, &[], &[closure], &[]).to_bytes();
    emit(&dir, "init_with_closure", &init_closure);

    // ── Delta: every patch tag + value variant + a string delta ─────────────
    let patches = vec![
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
                excerpt: None,
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
    ];
    let delta = Frame::delta(
        0x1234,
        0,
        &patches,
        &[(StringId::from(1u32), "hello".to_string())],
        &[],
        &[],
    )
    .to_bytes();
    emit(&dir, "delta_full_patchset", &delta);

    // ── Delta with signal_meta flag set (ADR-0027 trailing section) ──────────
    let meta = vec![flux_ir_serde::NodeSignalMeta {
        node_id: 1,
        deps: vec![SignalId::from(2u32), SignalId::from(3u32)],
        thunk: None,
        layout: vec![5u16, 6u16],
        item_slot: None,
    }];
    let delta_meta = Frame::delta(
        1,
        flux_ir_serde::FLAG_NODE_HAS_SIGNAL_DEPS,
        &[],
        &[],
        &[],
        &meta,
    )
    .to_bytes();
    emit(&dir, "delta_signal_meta", &delta_meta);

    // ── Error frame (with span + ADR-0057 excerpt) ───────────────────────────
    let excerpt = SourceExcerpt::from_span(Span::new(0, 12, 20), "count = count + 1\nwrong line\n")
        .expect("excerpt computes from source");
    let error = Frame::error(
        3,
        "type mismatch in Counter: expected Int, got String",
        Some(Span::new(0, 12, 20)),
        Some(excerpt),
    )
    .to_bytes();
    emit(&dir, "error_with_excerpt", &error);

    // ── Heartbeat ────────────────────────────────────────────────────────────
    let hb = Frame::heartbeat(42).to_bytes();
    emit(&dir, "heartbeat", &hb);

    // ── InternString request + StringInterned response (brittleness 4a) ──────
    let intern = Frame::intern_string(b"Button").to_bytes();
    emit(&dir, "intern_string_button", &intern);
    let mut rt = StringTable::new();
    let interned = Frame::intern_string(b"Button").intern_into(&mut rt);
    emit(&dir, "string_interned_button", &interned.to_bytes());
    // A canonical id must be below the ceiling (the invariant the harness guards).
    assert!(
        interned.id < STRING_ID_CANONICAL_CEILING,
        "interned id must be canonical"
    );

    // ── Boundary: max u16 prop indices (0xFFFF) ──────────────────────────────
    let boundary_root = flux_syntax::NodeRef {
        id: u32::MAX,
        kind: NodeKind::Component,
        component_id: u32::MAX,
        props: Props::from_fields(vec![
            (u16::MAX, Value::Int(i64::MIN)),
            (u16::MAX - 1, Value::Float(f64::MAX)),
        ]),
        children: vec![],
        handlers: vec![],
        span: Span::new(u32::MAX, u32::MAX, u32::MAX),
    };
    let init_boundary = Frame::init(
        &boundary_root,
        &[],
        &[(SignalId::from(u32::MAX), Value::Int(i64::MAX))],
        &[],
        &StringTable::new(),
        &[],
        &[],
        &[],
    )
    .to_bytes();
    emit(&dir, "init_max_indices", &init_boundary);

    // ── Boundary: content-addressed id from FLUX-074 (BLAKE3 of a value) ────
    let hashed = flux_ir_serde::hash_props(&[(0u16, Value::Int(7))]);
    // The content address is deterministic and stable; a frame is still a valid
    // seed regardless of which id space resolves it.
    let _ = hashed;
    let init_addr =
        Frame::init(&root, &[], &[], &[], &StringTable::new(), &[], &[], &[]).to_bytes();
    emit(&dir, "init_content_addressed", &init_addr);

    // ── Boundary: FLUX-083 version-mismatch frame (must fail closed) ─────────
    // magic(4) | version(1)=1 (unsupported) | type(1)=Init | + minimal payload.
    let mut bad = Vec::new();
    bad.extend_from_slice(&MAGIC.to_le_bytes());
    bad.push(1); // version 1, rejected by a v2 host
    bad.push(FRAME_INIT);
    bad.extend_from_slice(&0u32.to_le_bytes()); // seq
    emit(&dir, "version_mismatch_v1_init", &bad);

    // version 99 (wildly unsupported) on a Heartbeat-shaped frame.
    let mut bad_hb = Vec::new();
    bad_hb.extend_from_slice(&MAGIC.to_le_bytes());
    bad_hb.push(99);
    bad_hb.push(FRAME_HEARTBEAT);
    bad_hb.extend_from_slice(&0u32.to_le_bytes());
    emit(&dir, "version_mismatch_v99_heartbeat", &bad_hb);

    // ── Boundary: wrong magic (must be rejected, never mis-decoded) ──────────
    let mut wrong_magic = Vec::new();
    wrong_magic.extend_from_slice(b"XFLU"); // not the FLUX magic
    wrong_magic.push(PROTOCOL_VERSION);
    wrong_magic.push(FRAME_DELTA);
    wrong_magic.extend_from_slice(&[0u8; 8]);
    emit(&dir, "wrong_magic_delta", &wrong_magic);

    // ── Boundary: truncated frame (shorter than the 6-byte header) ──────────
    emit(&dir, "truncated_3bytes", &MAGIC.to_le_bytes()[..3]);

    // ── Boundary: unknown frame type byte (must be rejected) ─────────────────
    let mut unknown_type = Vec::new();
    unknown_type.extend_from_slice(&MAGIC.to_le_bytes());
    unknown_type.push(PROTOCOL_VERSION);
    unknown_type.push(0x42); // reserved / unknown type
    emit(&dir, "unknown_frame_type", &unknown_type);

    eprintln!("done.");
}
