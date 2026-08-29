//! `flux-lsp` binary — the spawnable Flux language server.
//!
//! Consumed by the FLUX-026 VS Code extension (and any LSP client) over stdio.
//! The server loop itself lives in the library crate's [`flux_lsp::run_stdio`];
//! this entry point only owns process-level concerns: logging and the tokio
//! runtime the `async-lsp` loop runs on.

use flux_lsp::run_stdio;

/// Entry point: install tracing and drive the stdio server loop to completion.
///
/// # Errors
/// Propagates any error from the `async-lsp` main loop (transport EOF,
/// malformed JSON-RPC, …). A non-zero exit signals the client that the server
/// terminated unexpectedly.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_target(false)
        .init();
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(run_stdio())?;
    Ok(())
}
