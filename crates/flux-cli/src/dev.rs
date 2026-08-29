//! `flux dev` — start the hot-reload dev server (FLUX-022, spec §14.3).
//!
//! Binds [`DevServer::start`] on the spec default WS port (`:7331`), prints the
//! *actually bound* `ws://` address, and keeps the server alive until the task
//! is cancelled (Ctrl-C, or the process exiting).

use std::path::Path;

use flux_devserver::{DevServer, ServerConfig};

use anyhow::Context;

/// Starts the dev server rooted at `root`, blocking until cancelled.
///
/// # Errors
///
/// Returns [`DevServerError::Bind`] (as `anyhow::Error`) when the WS or HTTP
/// listener cannot bind, or [`DevServerError::Watch`] when `root` cannot be
/// watched.
pub(crate) async fn run(
    root: &Path,
    ws_host: &str,
    ws_port: u16,
    http_port: u16,
    token: Option<String>,
) -> anyhow::Result<()> {
    let mut config = ServerConfig::new(root)
        .with_ws_host(ws_host)
        .with_ws_port(ws_port)
        .with_http_port(http_port);
    if let Some(token) = token {
        config = config.with_auth_token(token);
    }
    let server = DevServer::start(config)
        .await
        .context("starting the flux dev server")?;

    let ws = server.ws_addr();
    println!("Listening on ws://{ws}");
    tracing::info!(ws = %ws, http = %server.http_addr(), "flux dev server running");

    // `RunningServer` shuts the listeners down on drop; the tokio runtime keeps
    // the accept and watch tasks alive until the process is cancelled.
    let _ = server;
    std::future::pending::<()>().await;
    Ok(())
}
