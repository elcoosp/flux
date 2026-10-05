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
        bytes[4],
        flux_ir_serde::PROTOCOL_VERSION + 1,
        "unsupported-version.bin must carry an unsupported version byte \
         (current + 1)"
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

// =============================================================================
// v3 fixture tests (ADR-0059)
// =============================================================================

#[test]
fn init_v3_fixture_decodes() {
    let path = fixture_path("init_v3.bin");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("fixture {} missing: {}", path.display(), e));
    assert_eq!(
        bytes[4],
        flux_ir_serde::PROTOCOL_VERSION,
        "init_v3.bin must carry the current protocol version"
    );
    let frame =
        Frame::from_init_bytes(&bytes).unwrap_or_else(|e| panic!("init_v3.bin must decode: {}", e));
    assert!(
        !frame.root.props.fields().is_empty() || !frame.root.children.is_empty(),
        "init_v3 should have a root with props and/or children"
    );
}

#[test]
fn init_v3_fixture_round_trips() {
    let path = fixture_path("init_v3.bin");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("fixture {} missing: {}", path.display(), e));
    let frame =
        Frame::from_init_bytes(&bytes).unwrap_or_else(|e| panic!("init_v3.bin must decode: {}", e));
    assert_eq!(
        frame.to_bytes(),
        bytes,
        "init_v3.bin must round-trip: decode → to_bytes must reproduce the committed bytes"
    );
}

#[test]
fn delta_v3_fixture_decodes() {
    let path = fixture_path("delta_v3.bin");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("fixture {} missing: {}", path.display(), e));
    assert_eq!(
        bytes[4],
        flux_ir_serde::PROTOCOL_VERSION,
        "delta_v3.bin must carry the current protocol version"
    );
    let frame = Frame::from_delta_bytes(&bytes)
        .unwrap_or_else(|e| panic!("delta_v3.bin must decode: {}", e));
    assert!(
        !frame.patches.is_empty(),
        "delta_v3 should contain at least one patch"
    );
}

#[test]
fn delta_v3_fixture_round_trips() {
    let path = fixture_path("delta_v3.bin");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("fixture {} missing: {}", path.display(), e));
    let frame = Frame::from_delta_bytes(&bytes)
        .unwrap_or_else(|e| panic!("delta_v3.bin must decode: {}", e));
    assert_eq!(
        frame.to_bytes(),
        bytes,
        "delta_v3.bin must round-trip: decode → to_bytes must reproduce the committed bytes"
    );
}

/// The v2 fixture and its v3 counterpart share structural content. ADR-0059
/// widened 16 length prefixes from u16 to u32, so v3 bytes must be strictly
/// longer. This pins the widening at the fixture level; a silent reversion to
/// u16 in the encoder would collapse the two lengths and fail here.
#[test]
fn v3_fixture_is_wider_than_v2() {
    let init_v2 = std::fs::read(fixture_path("init_v2.bin")).expect("init_v2.bin");
    let init_v3 = std::fs::read(fixture_path("init_v3.bin")).expect("init_v3.bin");
    assert!(
        init_v3.len() > init_v2.len(),
        "v3 Init must be wider than v2: {} vs {}",
        init_v3.len(),
        init_v2.len()
    );

    let delta_v2 = std::fs::read(fixture_path("delta_v2.bin")).expect("delta_v2.bin");
    let delta_v3 = std::fs::read(fixture_path("delta_v3.bin")).expect("delta_v3.bin");
    assert!(
        delta_v3.len() > delta_v2.len(),
        "v3 Delta must be wider than v2: {} vs {}",
        delta_v3.len(),
        delta_v2.len()
    );
}

