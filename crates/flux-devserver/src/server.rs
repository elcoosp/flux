//! The WebSocket patch channel, HTTP asset server, and file watcher (FLUX-019).
//!
//! [`DevServer::start`] binds all three and returns a [`RunningServer`] handle
//! carrying the bound addresses and a shutdown switch.
//!
//! The WebSocket side is fully asynchronous: [`tokio_tungstenite::accept_async`]
//! upgrades each accepted socket and one `tokio::spawn`ed task drives it, so a
//! slow or silent client can never stall the accept loop or another session.
//! Compile work — the only CPU-bound step — is pushed onto Tokio's blocking pool
//! with `spawn_blocking`, keeping the I/O reactor free (brittleness issue 6).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::Mutex;
use tokio::net::TcpListener;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::task::JoinHandle;

use crate::config::ServerConfig;
use crate::dispatch::NodeSignalDeps;
use crate::error::DevServerError;
use crate::pipeline::Pipeline;
use crate::watch::{Watcher, collect_flux_sources};
use flux_ir_serde::DebugCommand;

mod session;

/// Shared server state: the compile pipeline plus the connected clients' queues.
#[derive(Debug)]
pub(crate) struct Shared {
    pub(crate) pipeline: Mutex<Pipeline>,
    clients: Mutex<Vec<UnboundedSender<Vec<u8>>>>,
    shutdown: AtomicBool,
}

impl Shared {
    fn new(pipeline: Pipeline) -> Self {
        Self {
            pipeline: Mutex::new(pipeline),
            clients: Mutex::new(Vec::new()),
            shutdown: AtomicBool::new(false),
        }
    }

    fn register(&self) -> UnboundedReceiver<Vec<u8>> {
        let (tx, rx) = unbounded_channel();
        self.clients.lock().push(tx);
        rx
    }

    /// Fans `frame` out to every connected client, dropping closed queues.
    pub(crate) fn broadcast(&self, frame: Vec<u8>) {
        let mut clients = self.clients.lock();
        clients.retain(|tx| tx.send(frame.clone()).is_ok());
        tracing::debug!(
            clients = clients.len(),
            bytes = frame.len(),
            "broadcast frame"
        );
    }

    pub(crate) fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Relaxed)
    }
}

/// Entry point for launching the dev server.
#[derive(Debug)]
pub struct DevServer;

impl DevServer {
    /// Binds the WebSocket patch channel, the HTTP asset server and the file
    /// watcher, performs the initial compile, and returns the running handle.
    ///
    /// # Errors
    ///
    /// Returns [`DevServerError::Bind`] when either listener cannot bind, and
    /// [`DevServerError::Watch`] when the project root cannot be watched.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn run() -> Result<(), flux_devserver::DevServerError> {
    /// use flux_devserver::{DevServer, ServerConfig};
    ///
    /// let server = DevServer::start(ServerConfig::new(".")).await?;
    /// println!("patch channel on {}", server.ws_addr());
    /// server.shutdown();
    /// # Ok(())
    /// # }
    /// ```
    pub async fn start(config: ServerConfig) -> Result<RunningServer, DevServerError> {
        let (ws_listener, ws_addr) = bind_ws(config.ws_addr()).await?;
        let (http_listener, http_addr) = bind_http(config.http_addr()).await?;

        let mut pipeline = Pipeline::new(config.root(), config.profile());
        for (path, source) in collect_flux_sources(config.root()) {
            pipeline.set_source(&path, source);
        }
        let shared = Arc::new(Shared::new(pipeline));
        initial_compile(&shared);

        let watcher = Watcher::spawn(&config, Arc::clone(&shared))?;
        let accept_task = spawn_accept_loop(ws_listener, Arc::clone(&shared));
        let http_task = crate::assets::spawn(http_listener, config.root().to_path_buf());

        // DevTools WebSocket endpoint (`:7333`, spec §4.1): enriches host
        // telemetry with source spans and relays `DebugCommand`s. The host
        // command channel is drained here; forwarding onto the live host
        // session is owned by the host-session agent (§4.3).
        let devtools_addr = std::net::SocketAddr::from((
            [127, 0, 0, 1],
            crate::debug_bridge::DEFAULT_DEVTOOLS_PORT,
        ));
        let (host_command_tx, mut host_command_rx) =
            tokio::sync::mpsc::unbounded_channel::<DebugCommand>();
        let source_map = shared.pipeline.lock().devtools_source_map();
        let devtools_drain = tokio::spawn(async move {
            while host_command_rx.recv().await.is_some() {
                tracing::debug!("devtools command received (host forwarding pending)");
            }
        });
        let devtools_task = tokio::spawn(async move {
            if let Err(e) =
                crate::debug_bridge::serve_devtools(devtools_addr, source_map, host_command_tx)
                    .await
            {
                tracing::warn!(error = %e, "devtools endpoint stopped");
            }
        });

        tracing::info!(%ws_addr, %http_addr, %devtools_addr, root = %config.root().display(), "flux dev server started");
        Ok(RunningServer {
            shared,
            ws_addr,
            http_addr,
            devtools_addr,
            root: config.root().to_path_buf(),
            tasks: vec![accept_task, http_task, devtools_drain, devtools_task],
            watcher,
        })
    }
}

/// Binds the asynchronous WebSocket listener, returning it and its resolved
/// address (a configured port of `0` resolves to the chosen port).
async fn bind_ws(addr: SocketAddr) -> Result<(TcpListener, SocketAddr), DevServerError> {
    let bind_error = |source| DevServerError::Bind {
        kind: "websocket",
        addr,
        source,
    };
    let listener = TcpListener::bind(addr).await.map_err(bind_error)?;
    let resolved = listener.local_addr().map_err(bind_error)?;
    Ok((listener, resolved))
}

/// Binds the HTTP asset listener, returning it and its resolved address.
async fn bind_http(
    addr: SocketAddr,
) -> Result<(tokio::net::TcpListener, SocketAddr), DevServerError> {
    let bind_error = |source| DevServerError::Bind {
        kind: "http",
        addr,
        source,
    };
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(bind_error)?;
    let resolved = listener.local_addr().map_err(bind_error)?;
    Ok((listener, resolved))
}

/// Compiles once at start-up so the first `Hello` can be answered immediately.
fn initial_compile(shared: &Arc<Shared>) {
    let mut pipeline = shared.pipeline.lock();
    match pipeline.compile() {
        Ok(_) => tracing::info!("initial compile succeeded"),
        Err(diagnostic) => {
            tracing::warn!(%diagnostic, "initial compile failed; serving no tree until fixed");
        }
    }
}

/// Spawns the asynchronous accept loop; each accepted socket is upgraded and
/// driven by its own `tokio::spawn`ed session task, so one slow client cannot
/// stall the loop or any other session (brittleness issue 6).
fn spawn_accept_loop(listener: TcpListener, shared: Arc<Shared>) -> JoinHandle<()> {
    tokio::spawn(async move {
        while !shared.is_shutdown() {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    let shared = Arc::clone(&shared);
                    tokio::spawn(async move {
                        if let Err(error) = session::serve_client(stream, shared).await {
                            tracing::debug!(%peer, %error, "client session ended");
                        }
                    });
                }
                Err(error) => {
                    // A per-connection accept failure (e.g. the peer vanished
                    // between the SYN and the accept) must not kill the loop.
                    tracing::warn!(%error, "accept failed");
                }
            }
        }
    })
}

/// A running dev server: bound addresses plus the shutdown switch.
///
/// Dropping the handle stops the server, as does [`RunningServer::shutdown`].
#[derive(Debug)]
pub struct RunningServer {
    shared: Arc<Shared>,
    ws_addr: SocketAddr,
    http_addr: SocketAddr,
    devtools_addr: SocketAddr,
    root: PathBuf,
    tasks: Vec<JoinHandle<()>>,
    watcher: Watcher,
}

impl RunningServer {
    /// The bound WebSocket patch-channel address.
    #[must_use]
    pub fn ws_addr(&self) -> SocketAddr {
        self.ws_addr
    }

    /// The bound HTTP asset-server address.
    #[must_use]
    pub fn http_addr(&self) -> SocketAddr {
        self.http_addr
    }

    /// The bound DevTools WebSocket address (`:7333`, spec §4.1).
    #[must_use]
    pub fn devtools_addr(&self) -> SocketAddr {
        self.devtools_addr
    }

    /// The watched project root.
    #[must_use]
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    /// Signals every task to stop and aborts the listeners.
    pub fn shutdown(self) {
        self.stop();
    }

    fn stop(&self) {
        self.shared.shutdown.store(true, Ordering::Relaxed);
        self.watcher.stop();
        for task in &self.tasks {
            task.abort();
        }
        tracing::info!("flux dev server stopped");
    }

    /// Ships `frame` to every connected host. Exposed for the CLI's manual
    /// rebuild command.
    pub fn broadcast(&self, frame: Vec<u8>) {
        self.shared.broadcast(frame);
    }

    /// Recompiles the project and ships the resulting frame, mirroring what a
    /// file save does. Returns whether a frame was shipped.
    pub fn rebuild(&self) -> bool {
        crate::watch::compile_and_broadcast(&self.shared)
    }

    /// Whether the pipeline currently holds a good tree.
    #[must_use]
    pub fn has_tree(&self) -> bool {
        self.shared.pipeline.lock().has_tree()
    }

    /// Injects the per-node `signal_deps` the server's minimal-patch index is
    /// built from (ADR-0027 Phase 2). Exposed for the file-watch path and for
    /// integration tests; in production FA-IRWIRE will populate this directly
    /// from the lowered IR.
    pub fn set_signal_deps(&self, deps: Option<Vec<NodeSignalDeps>>) {
        self.shared.pipeline.lock().set_signal_deps(deps);
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        self.stop();
    }
}
