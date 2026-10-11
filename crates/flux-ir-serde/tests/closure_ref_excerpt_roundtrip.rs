//! FLUX-F57 regression: a `ClosureRef` carrying a `SourceExcerpt` must
//! survive a `Delta` frame round-trip. The encoder used to write a u16
//! length prefix while the decoder read the version-dispatched (u32 at
//! v3) prefix, so the decode failed with a spurious "truncated frame".

use flux_ir_serde::Frame;
use flux_syntax::{ClosureRef, FileId, HandlerId, Patch, SourceExcerpt, Span};

fn mk_excerpt() -> SourceExcerpt {
    SourceExcerpt {
        file_id: FileId::from(1u32),
        byte_start: 10,
        byte_end: 20,
        line: 3,
        col: 5,
        snippet: "let x = 42;".to_string(),
    }
}

fn mk_closure(excerpt: Option<SourceExcerpt>) -> ClosureRef {
    ClosureRef {
        hash: 0x1234_5678_9abc_def0,
        bytecode_offset: 0,
        bytecode_len: 0,
        captured_signals: vec![],
        span: Span::new(1, 10, 20),
        excerpt,
    }
}

#[test]
fn closure_ref_with_excerpt_roundtrips() {
    let closure_ref = mk_closure(Some(mk_excerpt()));
    let frame_bytes = Frame::delta(
        0,
        0,
        &[Patch::Handler {
            id: HandlerId::from(1u32),
            closure: closure_ref.clone(),
        }],
        &[],
        &[],
        &[],
    )
    .try_to_bytes()
    .expect("encode frame");

    let decoded = Frame::from_delta_bytes(&frame_bytes)
        .expect("frame with a handler excerpt must decode");

    let mut found: Option<ClosureRef> = None;
    for p in &decoded.patches {
        if let Patch::Handler { closure, .. } = p {
            found = Some(closure.clone());
        }
    }
    let found = found.expect("handler patch present");
    assert_eq!(
        found.excerpt, closure_ref.excerpt,
        "the source excerpt must survive the round-trip verbatim"
    );
}

#[test]
fn closure_ref_without_excerpt_still_roundtrips() {
    let closure_ref = mk_closure(None);
    let frame_bytes = Frame::delta(
        0,
        0,
        &[Patch::Handler {
            id: HandlerId::from(2u32),
            closure: closure_ref.clone(),
        }],
        &[],
        &[],
        &[],
    )
    .try_to_bytes()
    .expect("encode frame");

    let decoded = Frame::from_delta_bytes(&frame_bytes).expect("decode");
    let mut found: Option<ClosureRef> = None;
    for p in &decoded.patches {
        if let Patch::Handler { closure, .. } = p {
            found = Some(closure.clone());
        }
    }
    let found = found.expect("handler patch present");
    assert_eq!(found.excerpt, None);
}

#[test]
fn multi_byte_snippet_roundtrips() {
    // A snippet whose first bytes would have masked the bug's symptom.
    let mut excerpt = mk_excerpt();
    excerpt.snippet = "élève = 42; // note".to_string();
    let closure_ref = mk_closure(Some(excerpt.clone()));
    let frame_bytes = Frame::delta(
        0,
        0,
        &[Patch::Handler {
            id: HandlerId::from(3u32),
            closure: closure_ref,
        }],
        &[],
        &[],
        &[],
    )
    .try_to_bytes()
    .expect("encode");

    let decoded = Frame::from_delta_bytes(&frame_bytes).expect("decode");
    let handler = decoded
        .patches
        .iter()
        .find_map(|p| match p {
            Patch::Handler { closure, .. } => Some(closure.clone()),
            _ => None,
        })
        .expect("handler patch");
    assert_eq!(handler.excerpt, Some(excerpt));
}
