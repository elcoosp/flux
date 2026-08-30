//! Low-level Appendix D binary codec (FLUX-013).
//!
//! This module owns the byte-level encoding/decoding for every wire type in
//! Appendix D §D.2–§D.11, split into single-responsibility submodules by
//! wire type:
//!
//! - [`core`] — [`WireError`], [`MAX_FRAME_BYTES`], [`validate_bytecode`], and
//!   the allocation-free [`Writer`]/[`Reader`] primitives.
//! - [`value`] / [`child`] / [`props`] / [`span`] / [`node`] / [`prop_diff`] /
//!   [`closure_ref`] / [`handler_section`] / [`string_entry`] / [`delta`] /
//!   [`patch`] / [`signal_meta`] — one per wire type.
//!
//! All integers are little-endian and every layout mirrors the appendix
//! byte-for-byte so the Swift/Kotlin production deserializers — which read the
//! same spec — stay in lock-step.
//!
//! The [`Writer`] and [`Reader`] are the only allocation-free primitives; the
//! typed encode/decode functions build on them. The decoder is used by the
//! round-trip tests and by the [`crate::frame`] decoders; it is *not* a
//! production path (the host apps ship their own).

pub mod child;
pub mod closure_ref;
pub mod core;
pub mod cursor;
pub mod delta;
pub mod handler_section;
pub mod node;
pub mod patch;
pub mod prop_diff;
pub mod props;
pub mod signal_meta;
pub mod span;
pub mod string_entry;
pub mod value;

// --- Public API (consumed by downstream crates + re-exported in lib.rs) ------

pub use core::{MAX_FRAME_BYTES, WireError, validate_bytecode};
pub use signal_meta::NodeSignalMeta;
pub use value::{decode_value_blob, encode_value_blob};

// --- Internal API (consumed by frame.rs / encode.rs / resume.rs / telemetry.rs) ---

pub(crate) use cursor::{Reader, Writer};
pub(crate) use handler_section::{
    decode_bytecode_blob, decode_handler_def, encode_bytecode_blob, encode_handler_def,
};
pub(crate) use node::{decode_node, encode_node};
pub(crate) use patch::{decode_patch, encode_patch};
pub(crate) use signal_meta::{decode_signal_meta_section, encode_signal_meta_section};
pub(crate) use span::{decode_span, decode_str, encode_span};
pub(crate) use string_entry::{decode_string_entry, encode_string_entry};
pub(crate) use value::{decode_value, encode_value};
