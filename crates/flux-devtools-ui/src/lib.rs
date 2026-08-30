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
//! be unit-tested without a display.
//! The gpui UI is always compiled on the nightly workspace toolchain (the pinned
//! `gpui` build needs `std::hint::cold_path`, a nightly feature).

#![forbid(unsafe_code)]
#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]
#![allow(unreachable_pub)]

pub mod state;
pub mod time_travel;
pub mod wire_client;

mod app;
mod perf_record;
mod row;
mod views;

pub use state::{DevToolsState, VmState};
pub use wire_client::{DEFAULT_DEVTOOLS_PORT, connect, encode_command, ingest_message};

pub use app::run_app;
