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
use tokio::sync::broadcast::{self, Receiver as BroadcastReceiver, Sender as BroadcastSender};
use tokio::task::JoinHandle;

use crate::config::ServerConfig;
use crate::debug_bridge::DevToolsRouter;
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
    /// Pairs host `AwaitSuspend` reports with capability completions and emits
    /// `Resume` frames (roadmap Phase 2).
    pub(crate) async_bridge: Mutex<crate::AsyncBridge>,
    /// Routes host telemetry (over the patch channel) to subscribed DevTools
    /// clients. Shared between the host accept loop and `serve_devtools` so a
    /// single router fans events out to every connected DevTools app.
    pub(crate) devtools_router: std::sync::Arc<parking_lot::Mutex<DevToolsRouter>>,
    clients: Mutex<BroadcastSender<Vec<u8>>>,
    shutdown: AtomicBool,
    /// Optional pairing token every host handshake must present (Appendix D
    /// §D.12.1, `flux dev --token`). `None` = accept any host (default open
    /// localhost loop).
    auth_token: Option<String>,
}

impl Shared {
    fn new(
        pipeline: Pipeline,
        devtools_router: std::sync::Arc<parking_lot::Mutex<DevToolsRouter>>,
        auth_token: Option<String>,
    ) -> Self {
        let (tx, _rx) = broadcast::channel(1024);
        Self {
            pipeline: Mutex::new(pipeline),
            async_bridge: Mutex::new(crate::AsyncBridge::new()),
            devtools_router,
            clients: Mutex::new(tx),
            shutdown: AtomicBool::new(false),
            auth_token,
        }
    }

    /// Subscribes a new host to broadcast frames. Only called after a
    /// successful handshake (audit C7).
    fn register(&self) -> BroadcastReceiver<Vec<u8>> {
        self.clients.lock().subscribe()
    }

    /// Fans `frame` out to every connected client. A lagging client (buffer
    /// full) is silently dropped — its session task will observe the
    /// `RecvError::Lagged` and close the connection (audit H15).
    pub(crate) fn broadcast(&self, frame: Vec<u8>) {
        // Audit H15: bounded broadcast channel; lagging clients (buffer full on
        // 1024 items) are evicted by their session task which observes
        // RecvError::Lagged and closes the socket. A send fails only when no
        // subscribers remain, which is a normal idle state, not an error.
        if self.clients.lock().send(frame).is_err() {
            tracing::trace!("broadcast: no active subscribers");
        }
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
        // Build the shared DevTools router up front (from the freshly assembled
        // pipeline's source map) so both the host patch-channel accept loop and
        // the `:7333` DevTools endpoint fan telemetry out through one router.
        // This lets the iOS host send telemetry over the existing `:7331`
        // WebSocket (the only port the Simulator forwards) instead of opening a
        // separate device→:7333 socket that the Simulator cannot reach.
        let (host_command_tx, mut host_command_rx) =
            tokio::sync::mpsc::channel::<DebugCommand>(1024);
        let source_map = pipeline.devtools_source_map();
        let devtools_router = std::sync::Arc::new(parking_lot::Mutex::new(DevToolsRouter::new(
            source_map,
            host_command_tx.clone(),
        )));
        let shared = Arc::new(Shared::new(
            pipeline,
            devtools_router,
            config.auth_token().map(str::to_owned),
        ));
        initial_compile(&shared);

        let watcher = Watcher::spawn(&config, Arc::clone(&shared))?;
        let accept_task = spawn_accept_loop(ws_listener, Arc::clone(&shared));
        let http_task = crate::assets::spawn(http_listener, config.root().to_path_buf());

        // DevTools WebSocket endpoint (spec §4.1): enriches host telemetry with
        // source spans and relays `DebugCommand`s. Binds to the config-driven
        // address (default 127.0.0.1:7333, NOT 0.0.0.0) so a `--ws-host 0.0.0.0`
        // LAN exposure does not also expose DevTools without an explicit opt-in.
        let devtools_addr = config.devtools_addr();
        let devtools_drain = tokio::spawn(async move {
            while host_command_rx.recv().await.is_some() {
                // DevTools commands are currently observed but not forwarded to
                // the live host session (forwarding is owned by the host-session
                // agent, §4.3). Drain so the channel never blocks the sender.
            }
        });
        let devtools_task = tokio::spawn({
            let router = std::sync::Arc::clone(&shared.devtools_router);
            let token = config.auth_token().map(str::to_owned);
            async move {
                if let Err(e) =
                    crate::debug_bridge::serve_devtools(devtools_addr, router, token).await
                {
                    tracing::warn!(error = %e, "devtools endpoint stopped");
                }
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
    let (perf_json, refreshed_map) = {
        let mut pipeline = shared.pipeline.lock();
        let mut refreshed_map = None;
        match pipeline.compile() {
            Ok(_) => {
                // Round-21 fix: the router was built from the pipeline BEFORE
                // this first compile, so its SourceMap was empty forever and
                // every DevTools telemetry enrichment resolved to "no source
                // mapping". Capture the map from the freshly compiled tree and
                // install it on the router below.
                refreshed_map = Some(pipeline.devtools_source_map());
                tracing::info!("initial compile succeeded");
            }
            Err(diagnostic) => {
                tracing::warn!(%diagnostic, "initial compile failed; serving no tree until fixed");
            }
        }
        // Capture the render-perf records from the initial compile so DevTools
        // shows the flamegraph even before the first file edit (FLUX-059).
        let perf_json = pipeline
            .perf_records()
            .into_iter()
            .filter_map(|r| r.to_json().ok())
            .collect::<Vec<String>>();
        (perf_json, refreshed_map)
    };
    if let Some(map) = refreshed_map {
        shared.devtools_router.lock().set_source_map(map);
    }
    for json in &perf_json {
        shared
            .devtools_router
            .lock()
            .route_telemetry(&flux_ir_serde::TelemetryEvent::perf_record(json.clone()));
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use flux_ir_serde::Frame;

    /// A minimal well-formed component source whose tree carries one handler
    /// closure, so the compiled SourceMap has both node spans and a bytecode
    /// blob to resolve.
    const SOURCE: &str = "compo Hello\n  state count: Int = 0\n  Button(text: \"tap\", onClick: fn() { count = count + 1 })\n";

    fn scratch_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "flux-devserver-sourcemap-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        std::fs::write(dir.join("main.flux"), SOURCE).expect("write source");
        dir
    }

    /// A `Shared` whose pipeline already holds one source, paired with a router
    /// built the way `DevServer::start` builds it: from the PRE-compile
    /// pipeline, i.e. with an empty SourceMap.
    fn shared_with_router(root: &Path) -> (Arc<Shared>, Arc<parking_lot::Mutex<DevToolsRouter>>) {
        let (tx, _rx) = tokio::sync::mpsc::channel::<DebugCommand>(16);
        let router = Arc::new(parking_lot::Mutex::new(DevToolsRouter::new(
            crate::debug_bridge::SourceMap::default(),
            tx,
        )));
        let mut pipeline = Pipeline::new(root, false);
        pipeline.set_source(&root.join("main.flux"), SOURCE.to_owned());
        let shared = Arc::new(Shared::new(pipeline, Arc::clone(&router), None));
        (shared, router)
    }

    /// Subscribes to the host broadcast channel so a test can capture the
    /// frame a compile ships (same-module access to the private sender).
    fn subscribe(shared: &Shared) -> tokio::sync::broadcast::Receiver<Vec<u8>> {
        shared.clients.lock().subscribe()
    }

    #[test]
    fn initial_compile_installs_devtools_source_map() {
        // Round-21 fix: the router's SourceMap used to be captured before the
        // first compile and stayed empty forever, so every DevTools telemetry
        // enrichment resolved to "no source mapping". The initial compile must
        // install a map that covers the compiled tree.
        let root = scratch_root("initial");
        let (shared, router) = shared_with_router(&root);
        // Sanity: the router starts with the pre-compile (empty) map.
        assert!(
            router
                .lock()
                .source_map()
                .span_for_bytecode_offset(0)
                .is_none()
        );
        initial_compile(&shared);
        // The tree's first closure bytecode starts at blob offset 0; the
        // refreshed map must resolve it to the handler's source span.
        assert!(
            router
                .lock()
                .source_map()
                .span_for_bytecode_offset(0)
                .is_some(),
            "initial_compile must install a source map covering the compiled tree"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn compile_and_broadcast_refreshes_devtools_source_map() {
        // Round-21 fix: every successful recompile must rebuild the router's
        // SourceMap from the new last-good tree, so spans track the edited
        // sources instead of the startup snapshot (or nothing at all).
        let root = scratch_root("recompile");
        let (shared, router) = shared_with_router(&root);
        let mut rx = subscribe(&shared);
        assert!(
            crate::watch::compile_and_broadcast(&shared),
            "the first compile ships an Init frame"
        );
        // Grab the shipped Init and probe the refreshed map with a real
        // merged-tree node id from the frame itself.
        let init = Frame::from_init_bytes(&rx.try_recv().expect("Init frame broadcast"))
            .expect("shipped frame decodes as Init");
        assert!(
            router
                .lock()
                .source_map()
                .span_for_node_id(init.root.id)
                .is_some(),
            "compile_and_broadcast must refresh the router's source map"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
