//! Round-21 regressions for the wire codec:
//! 1. A `ClosureRef` carrying a `SourceExcerpt` desynced every v3 frame: the
//!    encoder wrote a fixed u16 length prefix while the decoder read a
//!    version-dependent (v3 = u32) one.
//! 2. A handler stream whose every closure has empty bytecode produced an
//!    UNDECODABLE frame (empty blob ⇒ hosts read no count, but the encoder
//!    wrote one). The encoder now rejects that combination fail-loud.

use flux_ir::ClosureIR;
use flux_ir_serde::{deserialize_patches, serialize_patches};
use flux_syntax::{
    ClosureRef, ComponentId, FileId, HandlerId, NodeKind, NodeRef, SourceExcerpt, Span, StringId,
    StringTable, TypeId,
};

fn excerpt() -> SourceExcerpt {
    SourceExcerpt {
        file_id: FileId::from(3u32),
        byte_start: 40,
        byte_end: 58,
        line: 5,
        col: 3,
        snippet: "count = count + 1".to_owned(),
    }
}

/// A `Patch::Handler` whose closure carries a non-empty v3 excerpt must
/// round-trip through the codec, and re-encoding must be byte-identical.
#[test]
fn handler_patch_with_excerpt_round_trips() {
    let closure = ClosureRef {
        hash: 0x1234,
        bytecode_offset: 0,
        bytecode_len: 0,
        captured_signals: vec![],
        span: Span::new(0, 0, 10),
        excerpt: Some(excerpt()),
    };
    let patches = vec![flux_syntax::Patch::Handler {
        id: HandlerId::from(9u32),
        closure,
    }];
    let bytes = serialize_patches(&patches, &StringTable::new(), &[]);
    let (decoded, _) = deserialize_patches(&bytes).expect("v3 frame with excerpt must decode");
    match &decoded[0] {
        flux_syntax::Patch::Handler { closure, .. } => {
            let ex = closure.excerpt.as_ref().expect("excerpt must survive");
            assert_eq!(ex.snippet, "count = count + 1");
            assert_eq!(ex.file_id, FileId::from(3u32));
            assert_eq!(ex.line, 5);
            assert_eq!(ex.col, 3);
        }
        other => panic!("expected Handler patch, got {other:?}"),
    }
    let reencoded = serialize_patches(&decoded, &StringTable::new(), &[]);
    assert_eq!(
        reencoded, bytes,
        "decode → re-encode must reproduce the bytes"
    );
}

/// An Init frame carrying excerpted closures decodes all the way through the
/// frame composition (the Init path reads more sections after the closures).
#[test]
fn init_frame_with_excerpted_closures_decodes() {
    let closure = ClosureIR {
        id: HandlerId::from(1u32),
        bytecode: vec![0x00],
        captured_signals: vec![],
        span: Span::new(0, 0, 4),
        excerpt: Some(excerpt()),
        param_types: vec![],
        return_type: TypeId::from(0u32),
    };
    let root = NodeRef {
        id: 1,
        kind: NodeKind::Primitive,
        component_id: ComponentId::from(0u32),
        props: Default::default(),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 1),
    };
    let frame = flux_ir_serde::Frame::init(
        &root,
        &[],
        &[],
        &[],
        &StringTable::new(),
        &[],
        &[closure],
        &[],
    );
    let bytes = frame.to_bytes();
    let decoded = flux_ir_serde::Frame::from_init_bytes(&bytes)
        .expect("Init with an excerpted closure must decode without desync");
    assert_eq!(decoded.closures.len(), 1);
    let ex = decoded.closures[0]
        .excerpt
        .as_ref()
        .expect("excerpt must survive the Init path");
    assert_eq!(ex.snippet, "count = count + 1");
}

/// The encoder must refuse (fail-loud) a closure stream that compiles to an
/// empty blob — under the D.12 contract an empty blob means "no count
/// follows", so emitting a count+defs stream after an empty blob is
/// undecodable by every host.
#[test]
fn all_empty_bytecode_handler_stream_is_rejected_at_encode() {
    let closures = vec![ClosureIR {
        id: HandlerId::from(1u32),
        bytecode: vec![], // empty body
        captured_signals: vec![],
        span: Span::new(0, 0, 0),
        excerpt: None,
        param_types: vec![],
        return_type: TypeId::from(0u32),
    }];
    let root = NodeRef {
        id: 1,
        kind: NodeKind::Primitive,
        component_id: ComponentId::from(0u32),
        props: Default::default(),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 1),
    };
    let result = flux_ir_serde::Frame::init(
        &root,
        &[],
        &[],
        &[],
        &StringTable::new(),
        &[],
        &closures,
        &[],
    )
    .try_to_bytes();
    assert!(
        result.is_err(),
        "an all-empty-bytecode handler stream must be rejected at encode time"
    );
}

// Keep the interned-string id type referenced (the string table round-trips
// through these frames with dense first-insertion ids).
#[allow(dead_code)]
fn _table() -> StringId {
    StringId::from(0u32)
}
