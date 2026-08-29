//! `flux-devtools` binary — launches the Flux DevTools desktop window (gpui).
//!
//! Connects to the dev server's `:7333` DevTools WebSocket endpoint and renders
//! the time-travel debugger (VM inspector, signal graph, component tree,
//! timeline scrubber). Requires the nightly workspace toolchain (the pinned
//! gpui revision uses `std::hint::cold_path`).

use flux_devtools_ui::run_app;

fn main() -> anyhow::Result<()> {
    // Surface connection/ingest diagnostics on the terminal. `try_init` is a
    // no-op if another subscriber is already installed.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();
    run_app()
}
