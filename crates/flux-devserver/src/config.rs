//! Dev-server configuration (FLUX-019).

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Default WebSocket port for the patch channel (spec §D, FLUX-019).
pub const DEFAULT_WS_PORT: u16 = 7331;
/// Default HTTP port for the asset server.
pub const DEFAULT_HTTP_PORT: u16 = 7332;
/// File-watch debounce window: edits landing inside it compile once.
pub const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(50);
/// Frame-coalescing window: frames produced inside it ship as one batch.
pub const DEFAULT_COALESCE: Duration = Duration::from_millis(16);

/// How the dev server binds, watches, and reports.
///
/// Build it with [`ServerConfig::new`] and the `with_*` methods; every field has
/// a spec default so `ServerConfig::new(root)` is a complete configuration.
///
/// # Examples
///
/// ```rust
/// use flux_devserver::ServerConfig;
///
/// let config = ServerConfig::new(".").with_profile(true);
/// assert_eq!(config.ws_addr().port(), flux_devserver::DEFAULT_WS_PORT);
/// assert!(config.profile());
/// ```
#[derive(Clone, Debug)]
pub struct ServerConfig {
    root: PathBuf,
    ws_addr: SocketAddr,
    http_addr: SocketAddr,
    debounce: Duration,
    coalesce: Duration,
    profile: bool,
}

impl ServerConfig {
    /// Creates a configuration rooted at `root` with the spec default ports,
    /// a 50 ms watch debounce and a 16 ms coalescing window.
    #[must_use]
    pub fn new(root: impl AsRef<Path>) -> Self {
        // Resolve `root` to an absolute, canonical path. The file watcher
        // (FSEvents/kqueue) reports events with absolute paths, while a
        // relative `root` would make `collect_flux_sources` key sources by
        // relative paths. Mismatched keys would cause `set_source` (called on
        // every save) to update a *different* `FileId` than the initial
        // compile used, so recompiles would see the stale source and emit no
        // `Delta` — silently breaking hot reload (FLUX-019 regression).
        let root = root.as_ref().to_path_buf();
        let root = std::fs::canonicalize(&root).unwrap_or_else(|_| {
            std::env::current_dir()
                .map(|cwd| cwd.join(&root))
                .unwrap_or(root.clone())
        });
        Self {
            root,
            ws_addr: SocketAddr::from(([127, 0, 0, 1], DEFAULT_WS_PORT)),
            http_addr: SocketAddr::from(([127, 0, 0, 1], DEFAULT_HTTP_PORT)),
            debounce: DEFAULT_DEBOUNCE,
            coalesce: DEFAULT_COALESCE,
            profile: false,
        }
    }

    /// Overrides the WebSocket bind address (port `0` picks a free port).
    #[must_use]
    pub fn with_ws_addr(mut self, addr: SocketAddr) -> Self {
        self.ws_addr = addr;
        self
    }

    /// Overrides the WebSocket bind host, keeping the current port.
    ///
    /// `host` is parsed as an [`std::net::IpAddr`]; `0.0.0.0` binds all
    /// interfaces so the server is reachable from other machines on the LAN
    /// (e.g. a physical Android device tethered over USB).
    #[must_use]
    pub fn with_ws_host(mut self, host: &str) -> Self {
        match host.parse::<std::net::IpAddr>() {
            Ok(ip) => {
                let port = self.ws_addr.port();
                self.ws_addr = SocketAddr::new(ip, port);
            }
            Err(_) => tracing::warn!(
                host = host,
                "ignoring unparseable ws_host; keeping current bind address"
            ),
        }
        self
    }

    /// Overrides the WebSocket bind port, keeping the current host.
    #[must_use]
    pub fn with_ws_port(mut self, port: u16) -> Self {
        let ip = self.ws_addr.ip();
        self.ws_addr = SocketAddr::new(ip, port);
        self
    }

    /// Overrides the HTTP asset-server bind address (port `0` picks a free port).
    #[must_use]
    pub fn with_http_addr(mut self, addr: SocketAddr) -> Self {
        self.http_addr = addr;
        self
    }

    /// Overrides the HTTP asset-server bind port, keeping the current host.
    #[must_use]
    pub fn with_http_port(mut self, port: u16) -> Self {
        let ip = self.http_addr.ip();
        self.http_addr = SocketAddr::new(ip, port);
        self
    }

    /// Overrides the file-watch debounce window.
    #[must_use]
    pub fn with_debounce(mut self, debounce: Duration) -> Self {
        self.debounce = debounce;
        self
    }

    /// Overrides the frame-coalescing window.
    #[must_use]
    pub fn with_coalesce(mut self, coalesce: Duration) -> Self {
        self.coalesce = coalesce;
        self
    }

    /// Enables per-phase timing logs (the `--profile` flag).
    #[must_use]
    pub fn with_profile(mut self, profile: bool) -> Self {
        self.profile = profile;
        self
    }

    /// The watched project root, also served by the HTTP asset server.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The WebSocket bind address.
    #[must_use]
    pub fn ws_addr(&self) -> SocketAddr {
        self.ws_addr
    }

    /// The HTTP asset-server bind address.
    #[must_use]
    pub fn http_addr(&self) -> SocketAddr {
        self.http_addr
    }

    /// The file-watch debounce window.
    #[must_use]
    pub fn debounce(&self) -> Duration {
        self.debounce
    }

    /// The frame-coalescing window.
    #[must_use]
    pub fn coalesce(&self) -> Duration {
        self.coalesce
    }

    /// Whether per-phase timings are logged.
    #[must_use]
    pub fn profile(&self) -> bool {
        self.profile
    }
}
