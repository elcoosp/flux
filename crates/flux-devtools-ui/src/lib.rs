//! Flux DevTools desktop application (gpui) — spec §5.
//!
//! This crate is the desktop debugger for the Flux dev server. It connects to
//! the dev server's `:7333` DevTools WebSocket endpoint, ingests an enriched
//! telemetry stream, and renders a time-travel debugger (VM inspector, signal
//! graph, component tree, timeline scrubber).
//!
//! Per ADR-0041, `gpui` is the only UI dependency. The time-travel core
//! ([`time_travel`]) and the wire client ([`wire_client`]) are deliberately
//! free of `gpui` so the scrub/replay algorithm and decode/dispatch logic can
//! be unit-tested without a display — and on the workspace's stable toolchain,
//! because the pinned `gpui` build needs a newer compiler (see `Cargo.toml`).
//! The gpui UI itself is gated behind the `gpui-ui` feature.

#![forbid(unsafe_code)]
#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]
#![allow(unreachable_pub)]

pub mod state;
pub mod time_travel;
pub mod wire_client;

#[cfg(feature = "gpui-ui")]
mod app;
#[cfg(feature = "gpui-ui")]
mod views;

pub use state::{DevToolsState, VmState};
pub use wire_client::{DEFAULT_DEVTOOLS_PORT, connect, encode_command, ingest_message};

#[cfg(feature = "gpui-ui")]
pub use app::run_app;
