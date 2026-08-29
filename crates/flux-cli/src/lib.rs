//! `flux` — the Flux command-line interface (FLUX-022, spec §14.3).
//!
//! The binary exposes four subcommands:
//!
//! * `flux init <name>` — scaffold a new Flux project.
//! * `flux dev [--root <path>]` — start the hot-reload dev server.
//! * `flux build --platform ios|android [--root <path>]` — codegen the project.
//! * `flux doc` — emit a JSON schema of the stdlib API.
//!
//! All errors are reported through [`anyhow`] at the binary boundary; library
//! code returns `Result<_, anyhow::Error>` only and never panics.

#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]

mod build;
mod dev;
mod doc;
mod init;
mod lsp;
mod sources;

/// Re-exported for the integration tests, which validate the stdlib schema
/// JSON directly without going through the `Command` dispatch.
pub use doc::build_schema;

/// Re-exported so the integration tests exercise the `flux lsp` core directly
/// (the subcommand prints what this returns).
pub use lsp::collect_lsp;

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// The Flux command-line interface.
#[derive(Debug, Parser)]
#[command(
    name = "flux",
    version,
    about = "Write-once native UI for iOS and Android",
    long_about = "flux is the developer CLI for the Flux UI language: scaffold projects, \
                  run the hot-reload dev server, codegen native apps, and inspect the stdlib."
)]
pub struct Cli {
    /// The subcommand to run.
    #[command(subcommand)]
    pub command: Command,
}

/// The available `flux` subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Scaffold a new Flux project at `<name>/`.
    Init {
        /// Project name; also the directory that is created.
        name: String,
    },

    /// Start the hot-reload dev server (WebSocket on `:7331`).
    Dev {
        /// Project root to watch and serve.
        #[arg(long, default_value = ".")]
        root: PathBuf,

        /// WebSocket bind host. Defaults to `127.0.0.1`. Use `0.0.0.0` to
        /// expose the server on the local network so physical devices and
        /// simulators can reach it via the host machine's LAN IP.
        #[arg(long, default_value = "127.0.0.1")]
        ws_host: String,

        /// WebSocket bind port (the patch channel).
        #[arg(long, default_value_t = flux_devserver::DEFAULT_WS_PORT)]
        ws_port: u16,

        /// HTTP asset-server bind port.
        #[arg(long, default_value_t = flux_devserver::DEFAULT_HTTP_PORT)]
        http_port: u16,
    },

    /// Codegen the project for a native platform.
    Build {
        /// Target platform.
        platform: Platform,

        /// Project root to build.
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },

    /// Emit parse + type-check diagnostics for a `.flux` source file as JSON (FLUX-025).
    Lsp {
        /// The `.flux` source file to analyze.
        file: PathBuf,

        /// Also run the type-checker (default). When false, only parse
        /// diagnostics are emitted (the fast path for large files).
        #[arg(long, default_value_t = true)]
        types: bool,
    },

    /// Emit a JSON schema of the stdlib API to stdout.
    Doc,
}

/// A native build target.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Platform {
    /// Apple platforms (SwiftUI / SwiftPM).
    Ios,
    /// Android (Jetpack Compose / Gradle).
    Android,
}

impl Platform {
    /// The subdirectory under `platforms/` that generated sources are written to.
    #[must_use]
    pub fn generated_dir_name(self) -> &'static str {
        match self {
            Platform::Ios => "ios",
            Platform::Android => "android",
        }
    }

    /// The file extension for generated sources on this platform.
    #[must_use]
    pub fn source_extension(self) -> &'static str {
        match self {
            Platform::Ios => "swift",
            Platform::Android => "kt",
        }
    }
}

/// Parses the CLI and dispatches to the selected subcommand.
///
/// # Errors
///
/// Propagates any error from the subcommand (a malformed project, a port that
/// cannot be bound, a write failure, …). Errors are rendered by the binary.
pub async fn run(command: Command) -> anyhow::Result<()> {
    match command {
        Command::Init { name } => init::run(&name),
        Command::Dev {
            root,
            ws_host,
            ws_port,
            http_port,
        } => dev::run(&root, &ws_host, ws_port, http_port).await,
        Command::Build { platform, root } => build::run(platform, &root),
        Command::Lsp { file, types } => lsp::run(&file, types),
        Command::Doc => doc::run(),
    }
}
