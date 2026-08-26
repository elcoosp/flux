//! Wire-protocol (Appendix D) serialization for the Flux reactive tree.
//!
//! `flux-ir-serde` turns an [`IRArena`] diff into the binary frames shipped
//! over the WebSocket dev channel. Frames use a fixed little-endian layout
//! (Appendix D §D.1) with BLAKE3 content addressing for props, closures, and
//! nodes (§D.14). The production deserializers are Swift/Kotlin; this crate
//! ships a round-trip decoder for tests only ([`wire::WireError`] carries the
//! decode failures).
//!
//! # High-level API
//!
//! * [`serialize_patches`] / [`deserialize_patches`] — raw Appendix D §D.2
//!   patch streams.
//! * [`Frame::hello`], [`Frame::init`], [`Frame::delta`], [`Frame::error`],
//!   [`Frame::heartbeat`] — frame construction with the protocol version.
//! * [`hash_props`], [`hash_closure`] — BLAKE3 content addresses.
//!
//! [`IRArena`]: flux_ir::IRArena

#![forbid(unsafe_code)]
#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]
// The crate's public API (`Frame`, `*Frame`, `FrameKind`, `MAGIC`, `FLAG_*`, …)
// is consumed by downstream crates (devserver/codegen), so `unreachable_pub`
// is a false positive here; the items are intentionally exposed.
#![allow(unreachable_pub)]

mod encode;
mod frame;
mod telemetry;
mod wire;

pub use encode::{deserialize_patches, hash_closure, hash_props, serialize_patches};
pub use frame::{
    DeltaFrame, ErrorFrame, Frame, FrameKind, HeartbeatFrame, HelloFrame, InitFrame,
    InternStringFrame, MAGIC, PROTOCOL_VERSION, STRING_ID_CANONICAL_CEILING, StringInternedFrame,
};
pub use frame::{
    FLAG_FULL_TREE, FLAG_HAS_SRC_MAP_DELTA, FLAG_HAS_STATE_DELTA, FLAG_HAS_STRING_DELTA,
    FLAG_HEARTBEAT, FLAG_NODE_HAS_SIGNAL_DEPS, FRAME_DELTA, FRAME_ERROR, FRAME_HEARTBEAT,
    FRAME_HELLO, FRAME_INIT, FRAME_INTERN_STRING, FRAME_STRING_INTERNED,
};
pub use telemetry::{
    DebugCommand, DebugCommandFrame, EnrichedTelemetryEvent, EnrichedTelemetryFrame,
    FRAME_DEBUG_COMMAND, FRAME_TELEMETRY, Rect, Registers, TelemetryEvent, TelemetryFrame,
    enrich_telemetry, enrich_with_span,
};
pub use wire::NodeSignalMeta;
pub use wire::WireError;
