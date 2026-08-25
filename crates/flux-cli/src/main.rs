//! `flux` binary entry point (FLUX-022, spec §14.3).
//!
//! Parses the command line, installs a `tracing` subscriber driven by
//! `RUST_LOG`, and runs the selected subcommand on the Tokio runtime.

#![forbid(unsafe_code)]

use clap::Parser;
use flux_cli::Cli;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(flux_cli::run(cli.command))
}
