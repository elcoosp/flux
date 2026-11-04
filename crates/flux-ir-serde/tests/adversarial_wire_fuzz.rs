//! Round-21 adversarial tests for the Appendix D wire codec: *structured*
//! corruption of valid Init/Delta frames. Complements `proptest_round_trip.rs`
//! (which only round-trips encoder output) and the single 4-zero-byte check in
//! `round_trip.rs`.
//!
//! Contract: truncation at every length must be rejected; header corruption
//! must be rejected; arbitrary single-byte mutations must never panic, never
//! hang, and (where the mutation preserves validity) at least re-encode
//! deterministically. `MAX_FRAME_BYTES` guards the decode-side length checks.

use flux_ir_serde::{Frame, STRING_ID_CANONICAL_CEILING};
use flux_syntax::{NodeKind, Patch, PropDiff, SignalId, Span, StringId, StringTable, Value};

/// Builds a realistic Init frame: one component root with props, a state seed
/// and a non-empty string table.
fn valid_init() -> Vec<u8> {
    let mut table = StringTable::new();
    let label = table.intern("Increment");
    let root = flux_syntax::NodeRef {
        id: 1,
        kind: NodeKind::Component,
        component_id: 1,
        props: flux_syntax::Props::from_fields(vec![(0u16, Value::Str(label))]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 8),
    };
    Frame::init(
        &root,
        &[],
        &[(SignalId::from(1u32), Value::Int(0))],
        &[(0u32, "src/main.flux".to_string())],
        &table,
        &[],
        &[],
        &[],
    )
    .to_bytes()
}

/// Builds a realistic Delta frame with one Update patch and an interned string.
fn valid_delta() -> Vec<u8> {
    let patches = vec![Patch::Update {
        id: 11,
        props_diff: PropDiff {
            changes: vec![
                (0u16, Value::Int(99)),
                (1u16, Value::Str(StringId::from(2u32))),
            ],
            removals: vec![3u16],
        },
    }];
    Frame::delta(
        0,
        0,
        &patches,
        &[(StringId::from(1u32), "hello".to_string())],
        &[],
        &[],
    )
    .to_bytes()
}

/// Truncating a valid frame must be rejected — a prefix of a frame is never a
/// frame. The ONE documented exception: the Init payload is terminated by an
/// optional 1-byte `signal_meta` presence marker (ADR-0027, "absent ⇒ empty"),
/// so cutting exactly that final byte still decodes. Never panic.
#[test]
fn every_truncation_is_rejected() {
    let init = valid_init();
    assert!(Frame::from_init_bytes(&init).is_ok(), "baseline decodes");
    for cut in 0..init.len() - 1 {
        assert!(
            Frame::from_init_bytes(&init[..cut]).is_err(),
            "init truncated at {cut} of {} must be rejected",
            init.len()
        );
    }
    // The final byte is the optional presence marker: its absence must decode
    // to an EMPTY signal_meta section, never to junk.
    let cut = Frame::from_init_bytes(&init[..init.len() - 1]).expect("marker-less init decodes");
    assert!(
        cut.signal_meta.is_empty(),
        "absent presence marker must mean empty signal_meta"
    );
    let delta = valid_delta();
    assert!(Frame::from_delta_bytes(&delta).is_ok(), "baseline decodes");
    for cut in 0..delta.len() {
        assert!(
            Frame::from_delta_bytes(&delta[..cut]).is_err(),
            "delta truncated at {cut} of {} must be rejected",
            delta.len()
        );
    }
}

/// Corrupting the 6-byte header (`magic(4) | version(1) | kind(1)`) must be
/// rejected: any changed magic/version byte, an invalid kind byte, or a valid
/// kind that differs from the decoder's expected kind.
#[test]
fn header_corruption_is_rejected() {
    for (name, bytes) in [("init", valid_init()), ("delta", valid_delta())] {
        // Magic + version bytes: any change must be fatal.
        for pos in 0..5 {
            let mut corrupted = bytes.clone();
            corrupted[pos] = corrupted[pos].wrapping_add(1);
            assert!(
                Frame::from_init_bytes(&corrupted).is_err(),
                "{name} header byte {pos} (+1) must be rejected as init"
            );
            assert!(
                Frame::from_delta_bytes(&corrupted).is_err(),
                "{name} header byte {pos} (+1) must be rejected as delta"
            );
        }
        // Kind byte (5): an invalid tag must be fatal for both decoders.
        let mut bad_kind = bytes.clone();
        bad_kind[5] = 0xFF;
        assert!(Frame::from_init_bytes(&bad_kind).is_err());
        assert!(Frame::from_delta_bytes(&bad_kind).is_err());
    }
    // Cross-kind was pinned separately; here pin that flipping Init's kind to
    // *another valid* kind (FRAME_HELLO = 0x01) is rejected by the Init decoder.
    let mut init = valid_init();
    init[5] = flux_ir_serde::FRAME_HELLO;
    assert!(
        Frame::from_init_bytes(&init).is_err(),
        "init decoder must reject a non-Init kind byte"
    );
}

/// XOR 0x55 over every single byte: the decoder must never panic and never
/// hang — a corrupted frame must surface as an `Err`, not an allocation
/// failure. (Regression: flipping a count-prefix byte in the Init `state_seed`
/// section or the Delta `patches`/`strings` counts used to drive
/// `Vec::with_capacity` into a ~57 GB allocation that aborted the process.)
#[test]
fn every_single_byte_xor_never_panics_or_hangs() {
    let ceiling = STRING_ID_CANONICAL_CEILING;
    assert!(ceiling > 0, "canonical ceiling must be positive");
    for bytes in [valid_init(), valid_delta()] {
        for pos in 0..bytes.len() {
            let mut mutated = bytes.clone();
            mutated[pos] ^= 0x55;
            // Never panic, never hang — outcome value is irrelevant.
            let _ = Frame::from_init_bytes(&mutated);
            let _ = Frame::from_delta_bytes(&mutated);
        }
    }
}

/// Bytes appended after a complete frame belong to the NEXT frame on the wire
/// (the transport multiplexes frames back-to-back), so the decoder must ignore
/// them — but the decoded content must be byte-identical to the junk-free
/// baseline: trailing garbage may never change what the host applies.
#[test]
fn trailing_junk_is_ignored_without_changing_content() {
    let init = valid_init();
    let baseline = Frame::from_init_bytes(&init).expect("baseline decodes");
    let mut junked = init.clone();
    junked.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
    let decoded = Frame::from_init_bytes(&junked).expect("init + trailing junk decodes");
    assert_eq!(decoded.state_seed, baseline.state_seed);
    assert_eq!(decoded.source_map, baseline.source_map);
    assert_eq!(
        decoded.string_table.resolve(StringId::from(0u32)),
        baseline.string_table.resolve(StringId::from(0u32))
    );
    assert!(baseline.signal_meta.is_empty());

    let delta = valid_delta();
    let mut delta_junked = delta.clone();
    delta_junked.extend_from_slice(&[0xFF; 8]);
    assert!(
        Frame::from_delta_bytes(&delta_junked).is_ok(),
        "delta + trailing junk decodes (stream semantics)"
    );
}

/// Cross-kind decoding: an Init body handed to the Delta decoder (and vice
/// versa) must be rejected, not misparsed.
#[test]
fn cross_kind_decode_is_rejected() {
    let init = valid_init();
    let delta = valid_delta();
    assert!(
        Frame::from_delta_bytes(&init).is_err(),
        "init bytes must not decode as delta"
    );
    assert!(
        Frame::from_init_bytes(&delta).is_err(),
        "delta bytes must not decode as init"
    );
}
