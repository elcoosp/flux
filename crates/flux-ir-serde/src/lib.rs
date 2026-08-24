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
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]

mod encode;
mod frame;
mod wire;

pub use encode::{deserialize_patches, hash_closure, hash_props, serialize_patches};
pub use frame::{
    DeltaFrame, ErrorFrame, Frame, FrameKind, HeartbeatFrame, HelloFrame, InitFrame, MAGIC,
    PROTOCOL_VERSION,
};
pub use wire::WireError;
