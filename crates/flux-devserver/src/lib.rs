//! `flux-devserver` — the Flux hot-reload dev server (FLUX-019).
//!
//! The server watches a project root for `.flux` saves and, for each save, runs
//! the pipeline
//!
//! ```text
//! parse → type_check → lower → diff(old, new) → serialize → send
//! ```
//!
//! shipping the result as an Appendix D wire frame over WebSocket on `:7331`
//! ([`DEFAULT_WS_PORT`]) while serving assets over HTTP on `:7332`
//! ([`DEFAULT_HTTP_PORT`]).
//!
//! # Protocol
//!
//! A host connects and sends `Hello` (spec §D.12.1); the server replies with a
//! full-tree `Init` (§D.12.2) carrying the root node, the state seed, the source
//! map and the string table resolved from
//! [`IRArena::string_table`](flux_ir::IRArena::string_table). Every later save
//! ships a `Delta` (§D.1). A reconnecting host repeats the handshake and gets a
//! fresh `Init` (§D.13). Malformed source ships an `Error` frame (§D.12.3) and
//! the previous good tree is retained — no `Delta` is produced.
//!
//! # Examples
//!
//! ```rust,no_run
//! # async fn run() -> Result<(), flux_devserver::DevServerError> {
//! use flux_devserver::{DevServer, ServerConfig};
//!
//! let server = DevServer::start(ServerConfig::new("./my-app").with_profile(true)).await?;
//! println!("patch channel: {}", server.ws_addr());
//! println!("assets:        {}", server.http_addr());
//! server.shutdown();
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]

mod assets;
mod capability_manifest;
mod config;
mod debug_bridge;
mod dispatch;
mod error;
mod pipeline;
mod server;
mod watch;

pub use dispatch::{
    DependencyIndex, DispatchReport, FRAME_DISPATCH_REPORT, MinimalPatchError, NodeSignalDeps,
    emit_minimal_updates,
};

pub use config::{
    DEFAULT_COALESCE, DEFAULT_DEBOUNCE, DEFAULT_HTTP_PORT, DEFAULT_WS_PORT, ServerConfig,
};
pub use debug_bridge::{DEFAULT_DEVTOOLS_PORT, DevToolsRouter, SourceMap, enrich, serve_devtools};
pub use error::{DevServerError, Diagnostic};
pub use pipeline::{Compiled, PhaseTimings, Pipeline};
pub use server::{DevServer, RunningServer};
