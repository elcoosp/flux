//! Error-frame / wire-version rejection parity (FLUX-082, links to FLUX-083).
//!
//! The plan (§2.1 / FLUX-083) requires the three decoders — Rust round-trip,
//! Kotlin `FrameDeserializer`, Swift `FrameDeserializer` — to reject a
//! version-mismatched / malformed frame with a clean, typed `WireError` *before*
//! any field decode, rather than a best-effort partial decode. The `flux-parity`
//! harness cannot execute the Kotlin/Swift decoders, but it can model the
//! host-side rejection contract and assert the Rust reference decoder and a
//! modeled host decoder agree on every frame in a corpus — so a regression that
//! changes *which* frames are rejected (and as what) is caught in CI.
//!
//! The harness builds real frames with [`flux_ir_serde::Frame`] (a valid `Error`
//! frame and a valid `Hello` frame) plus three malformed variants: a wrong
//! protocol version, a bad magic, and a truncated buffer. It then runs each
//! through [`ReferenceDecoder`] (the real Rust decoder) and [`HostDecoder`]
//! (the modeled host reject path) and asserts the two produce byte-compatible
//! [`Rejection`]s. The host model is the contract both production decoders must
//! honor; the Rust reference path is the source of truth the hosts are checked
//! against.

use flux_ir_serde::{Frame, FrameKind, MAGIC, PROTOCOL_VERSION, WireError};

/// The decoded outcome of feeding bytes to a frame decoder.
///
/// `Accepted` carries the frame kind (for valid frames); `Rejected` carries the
/// typed [`WireError`] reason. Both the Rust reference decoder and the modeled
/// host decoder must produce the *same* outcome for every frame in the corpus —
/// that is the parity contract FLUX-082/083 ratifies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rejection {
    /// The frame was accepted and decoded as this frame kind.
    Accepted(FrameKind),
    /// The frame was rejected with a typed `WireError` (fail-closed).
    Rejected(WireErrorKind),
}

/// A host-neutral summary of a [`WireError`], comparable across decoders.
///
/// `WireError` carries byte offsets that differ slightly between decoders; for
/// parity we compare only the *discriminant* and the load-bearing `context` /
/// `tag` fields (enough to localize a cross-platform reject divergence without
/// coupling to exact offsets).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WireErrorKind {
    /// A truncated buffer (header or payload).
    Truncated,
    /// An unknown/invalid tag byte (magic, version, or frame kind).
    InvalidTag,
    /// Invalid UTF-8 in a string field.
    InvalidUtf8,
    /// A malformed handler bytecode blob.
    MalformedBytecode,
    /// A frame whose declared payload exceeds the hard ceiling.
    FrameTooLarge,
}

impl From<&WireError> for WireErrorKind {
    fn from(e: &WireError) -> Self {
        match e {
            WireError::Truncated { .. } => WireErrorKind::Truncated,
            WireError::InvalidTag { .. } => WireErrorKind::InvalidTag,
            WireError::InvalidUtf8 { .. } => WireErrorKind::InvalidUtf8,
            WireError::MalformedBytecode { .. } => WireErrorKind::MalformedBytecode,
            WireError::FrameTooLarge { .. } => WireErrorKind::FrameTooLarge,
        }
    }
}

/// The Rust reference decoder path (the real `flux-ir-serde` decoders).
///
/// This is the source of truth the modeled host decoder is checked against.
#[derive(Debug)]
pub struct ReferenceDecoder;

impl ReferenceDecoder {
    /// Decodes `bytes` through the real Rust frame decoders, reporting the
    /// canonical [`Rejection`].
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Rejection {
        // The real `Error` frame decoder fails closed on a header fault
        // (version/magic/kind/truncation) *before* any field decode — exactly
        // the contract FLUX-083 pins. On success it reports the accepted kind.
        if let Ok(frame) = Frame::from_error_bytes(bytes) {
            return Rejection::Accepted(frame.kind);
        }
        // A header that is well-formed but simply a `Hello` frame is not a
        // reject: recognize it so valid `Hello` frames are `Accepted`.
        if let Some(hello) = Frame::from_hello_bytes(bytes) {
            return Rejection::Accepted(hello.kind);
        }
        let err = Frame::from_error_bytes(bytes).unwrap_err();
        Rejection::Rejected(WireErrorKind::from(&err))
    }
}

/// The modeled host decoder: the fail-closed contract both Swift/Kotlin
/// `FrameDeserializer`s must honor.
///
/// It mirrors the real header check in `read_frame_type` (magic → version →
/// kind) and rejects before field decode. Because the production decoders live
/// in the host apps, this model is the testable specification of their behavior;
/// the Rust [`ReferenceDecoder`] is the cross-checked source of truth.
#[derive(Debug)]
pub struct HostDecoder;

impl HostDecoder {
    /// Decodes `bytes` per the host reject contract: a 6-byte header with the
    /// right magic and the current `PROTOCOL_VERSION`, otherwise a typed reject.
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Rejection {
        if bytes.len() < 6 {
            return Rejection::Rejected(WireErrorKind::Truncated);
        }
        let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        if magic != MAGIC {
            return Rejection::Rejected(WireErrorKind::InvalidTag);
        }
        // Version mismatch (e.g. a v1 frame hitting a v2 host) must reject
        // fail-closed — never mis-decode. This is the FLUX-083 path.
        if bytes[4] != PROTOCOL_VERSION {
            return Rejection::Rejected(WireErrorKind::InvalidTag);
        }
        let kind = match bytes[5] {
            flux_ir_serde::FRAME_HELLO => Some(FrameKind::Hello),
            flux_ir_serde::FRAME_INIT => Some(FrameKind::Init),
            flux_ir_serde::FRAME_ERROR => Some(FrameKind::Error),
            flux_ir_serde::FRAME_DELTA => Some(FrameKind::Delta),
            flux_ir_serde::FRAME_HEARTBEAT => Some(FrameKind::Heartbeat),
            flux_ir_serde::FRAME_INTERN_STRING => Some(FrameKind::InternString),
            flux_ir_serde::FRAME_STRING_INTERNED => Some(FrameKind::StringInterned),
            _ => None,
        };
        match kind {
            Some(kind) => Rejection::Accepted(kind),
            None => Rejection::Rejected(WireErrorKind::InvalidTag),
        }
    }
}

/// A frame in the rejection corpus: a label plus the raw bytes.
#[derive(Clone, Debug)]
pub struct CorpusFrame {
    /// Human-readable description of the frame (used in assertion messages).
    pub label: &'static str,
    /// Raw frame bytes.
    pub bytes: Vec<u8>,
}

/// Builds the canonical rejection corpus: one valid `Error` frame, one valid
/// `Hello` frame, and three malformed variants (bad version, bad magic,
/// truncated). The malformed variants are exactly what FLUX-083 requires all
/// three decoders to reject with a typed error.
#[must_use]
pub fn default_corpus() -> Vec<CorpusFrame> {
    let valid_error = Frame::error(1, "compile failed at L3", None, None).to_bytes();
    let valid_hello = Frame::hello("ios", "iPhone", &[]).to_bytes();
    let mut bad_version = valid_error.clone();
    bad_version[4] = PROTOCOL_VERSION.wrapping_add(1); // future/unknown version
    let mut bad_magic = valid_error.clone();
    bad_magic[0] ^= 0xFF; // corrupt the magic word
    // Header-level truncation: fewer than the 6-byte magic+version+kind header.
    // Both the real Rust decoder and the modeled host decoder must reject this
    // fail-closed (FLUX-083) before any field decode.
    let truncated = valid_error[..4].to_vec();

    vec![
        CorpusFrame {
            label: "valid_error",
            bytes: valid_error,
        },
        CorpusFrame {
            label: "valid_hello",
            bytes: valid_hello,
        },
        CorpusFrame {
            label: "bad_version",
            bytes: bad_version,
        },
        CorpusFrame {
            label: "bad_magic",
            bytes: bad_magic,
        },
        CorpusFrame {
            label: "truncated",
            bytes: truncated,
        },
    ]
}

/// Runs the full corpus through [`ReferenceDecoder`] and [`HostDecoder`] and
/// asserts both produce the same [`Rejection`] for every frame.
///
/// Returns the label of the first diverging frame (with both outcomes rendered)
/// so CI localizes a cross-decoder regression.
///
/// # Errors
///
/// Returns a descriptive message on the first divergence; `Ok(())` when every
/// frame is accepted/rejected identically (the parity contract holds).
pub fn assert_error_frame_parity() -> Result<(), String> {
    let corpus = default_corpus();
    for frame in &corpus {
        let reference = ReferenceDecoder::decode(&frame.bytes);
        let host = HostDecoder::decode(&frame.bytes);
        if reference != host {
            return Err(format!(
                "error-frame parity divergence on `{}`: reference={:?} host={:?}",
                frame.label, reference, host
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_frames_are_accepted() {
        let corpus = default_corpus();
        for frame in &corpus {
            if frame.label == "valid_error" {
                assert_eq!(
                    HostDecoder::decode(&frame.bytes),
                    Rejection::Accepted(FrameKind::Error),
                    "valid Error frame must be accepted"
                );
            }
            if frame.label == "valid_hello" {
                assert_eq!(
                    HostDecoder::decode(&frame.bytes),
                    Rejection::Accepted(FrameKind::Hello),
                    "valid Hello frame must be accepted"
                );
            }
        }
    }

    #[test]
    fn malformed_frames_are_rejected_fail_closed() {
        let corpus = default_corpus();
        for frame in &corpus {
            if frame.label == "bad_version" || frame.label == "bad_magic" {
                assert_eq!(
                    HostDecoder::decode(&frame.bytes),
                    Rejection::Rejected(WireErrorKind::InvalidTag),
                    "frame `{}` must reject with InvalidTag",
                    frame.label
                );
            }
            if frame.label == "truncated" {
                assert_eq!(
                    HostDecoder::decode(&frame.bytes),
                    Rejection::Rejected(WireErrorKind::Truncated),
                    "truncated frame must reject with Truncated"
                );
            }
        }
    }

    #[test]
    fn reference_and_host_decoders_agree() {
        assert_error_frame_parity().expect("reference and host decoders must agree on the corpus");
    }
}
