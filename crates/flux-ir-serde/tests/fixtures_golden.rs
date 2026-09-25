//! T-503: wire fixture golden test.
//!
//! Decodes the committed `fixtures/wire/*.bin` fixtures and asserts they
//! can be decoded without error (init_v2 and delta_v2) and that
//! `unsupported-version.bin` is rejected fail-closed. Additionally, each
//! fixture must round-trip through a fresh encode (decode → `to_bytes`),
//! which catches accidental encoder drift (T-505).

use flux_ir_serde::Frame;

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/wire")
        .join(name)
}

#[test]
fn init_v2_fixture_decodes() {
    let path = fixture_path("init_v2.bin");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("fixture {} missing: {}", path.display(), e));
    let frame =
        Frame::from_init_bytes(&bytes).unwrap_or_else(|e| panic!("init_v2.bin must decode: {}", e));
    assert!(
        !frame.root.props.fields().is_empty() || !frame.root.children.is_empty(),
        "init_v2 should have a root with props and/or children"
    );
}

#[test]
fn delta_v2_fixture_decodes() {
    let path = fixture_path("delta_v2.bin");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("fixture {} missing: {}", path.display(), e));
    let frame = Frame::from_delta_bytes(&bytes)
        .unwrap_or_else(|e| panic!("delta_v2.bin must decode: {}", e));
    assert!(
        frame.patches.len() >= 1,
        "delta_v2 should contain at least one patch"
    );
}

#[test]
fn unsupported_version_fixture_is_rejected() {
    let path = fixture_path("unsupported-version.bin");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("fixture {} missing: {}", path.display(), e));
    assert_eq!(
        bytes[4], 3,
        "unsupported-version.bin must carry an unsupported version byte"
    );
    assert!(
        Frame::from_init_bytes(&bytes).is_err(),
        "unsupported-version.bin must be rejected fail-closed"
    );
}

/// T-505: decode → re-encode must reproduce the committed fixture bytes
/// exactly. This catches accidental encoder drift (e.g., field reordering
/// in `Frame::to_bytes` that diverges from the committed golden).
#[test]
fn init_v2_fixture_round_trips() {
    let path = fixture_path("init_v2.bin");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("fixture {} missing: {}", path.display(), e));
    let frame =
        Frame::from_init_bytes(&bytes).unwrap_or_else(|e| panic!("init_v2.bin must decode: {}", e));
    let reencoded = frame.to_bytes();
    assert_eq!(
        bytes.as_slice(),
        reencoded.as_slice(),
        "init_v2.bin must round-trip: decode → to_bytes must reproduce the committed bytes"
    );
}

/// T-505: delta fixture must also round-trip through encode.
#[test]
fn delta_v2_fixture_round_trips() {
    let path = fixture_path("delta_v2.bin");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("fixture {} missing: {}", path.display(), e));
    let frame = Frame::from_delta_bytes(&bytes)
        .unwrap_or_else(|e| panic!("delta_v2.bin must decode: {}", e));
    let reencoded = frame.to_bytes();
    assert_eq!(
        bytes.as_slice(),
        reencoded.as_slice(),
        "delta_v2.bin must round-trip: decode → to_bytes must reproduce the committed bytes"
    );
}
