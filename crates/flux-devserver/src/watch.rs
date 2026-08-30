//! File watching with debounce and frame coalescing (FLUX-019).
//!
//! `notify` events for `.flux` files are debounced (50 ms by default) so an
//! editor's write burst compiles once; the resulting frames are coalesced over a
//! 16 ms window before they go out on the wire.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecursiveMode, Watcher as _};

use crate::config::ServerConfig;
use crate::error::DevServerError;
use crate::pipeline::Compiled;
use crate::server::Shared;

/// The `.flux` source extension.
const FLUX_EXT: &str = "flux";

/// A running file watcher. Dropping it (or calling [`Watcher::stop`]) ends the
/// watch thread.
#[derive(Debug)]
pub(crate) struct Watcher {
    running: Arc<AtomicBool>,
}

impl Watcher {
    /// Starts watching `config.root()` recursively.
    ///
    /// # Errors
    ///
    /// Returns [`DevServerError::Watch`] when the root cannot be watched.
    pub(crate) fn spawn(
        config: &ServerConfig,
        shared: Arc<Shared>,
    ) -> Result<Self, DevServerError> {
        let root = config.root().to_path_buf();
        let watch_error = |e: notify::Error| DevServerError::Watch {
            root: root.display().to_string(),
            message: e.to_string(),
        };
        let (tx, rx) = channel::<notify::Result<Event>>();
        let mut watcher = notify::recommended_watcher(tx).map_err(watch_error)?;
        watcher
            .watch(&root, RecursiveMode::Recursive)
            .map_err(watch_error)?;

        let running = Arc::new(AtomicBool::new(true));
        let flag = Arc::clone(&running);
        let timing = Timing {
            debounce: config.debounce(),
            coalesce: config.coalesce(),
        };
        std::thread::spawn(move || {
            // `watcher` is moved in so the watch stays alive for the loop.
            let _watcher = watcher;
            watch_loop(&rx, &shared, &flag, timing);
        });
        Ok(Self { running })
    }

    /// Stops the watch thread.
    pub(crate) fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

/// The debounce and coalescing windows the watch loop runs with.
#[derive(Clone, Copy, Debug)]
struct Timing {
    /// Saves landing inside this window compile once.
    debounce: Duration,
    /// Frames produced inside this window ship as one batch.
    coalesce: Duration,
}

/// Debounces watch events, re-reads the changed sources, and recompiles.
fn watch_loop(
    rx: &Receiver<notify::Result<Event>>,
    shared: &Arc<Shared>,
    flag: &Arc<AtomicBool>,
    timing: Timing,
) {
    let mut pending: Vec<PathBuf> = Vec::new();
    let mut last_event: Option<Instant> = None;
    while flag.load(Ordering::Relaxed) && !shared.is_shutdown() {
        match rx.recv_timeout(timing.debounce) {
            Ok(Ok(event)) => {
                if is_source_change(&event) {
                    pending.extend(event.paths.into_iter().filter(|p| is_flux_source(p)));
                    last_event = Some(Instant::now());
                }
            }
            Ok(Err(error)) => tracing::warn!(%error, "WATCH_ERROR"),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                tracing::warn!("WATCH_DISCONNECTED");
                return;
            }
        }
        let settled = last_event.is_some_and(|t| t.elapsed() >= timing.debounce);
        if pending.is_empty() || !settled {
            continue;
        }
        reload(&mut pending, shared);
        last_event = None;
        // Coalescing window: a second save landing inside it is picked up by
        // the same compile pass.
        std::thread::sleep(timing.coalesce);
        compile_and_broadcast(shared);
    }
}

/// Re-reads every pending path into the pipeline's source snapshot, draining
/// `pending`.
fn reload(pending: &mut Vec<PathBuf>, shared: &Arc<Shared>) {
    pending.sort();
    pending.dedup();
    for path in pending.drain(..) {
        match std::fs::read_to_string(&path) {
            Ok(source) => shared.pipeline.lock().set_source(&path, source),
            Err(error) => {
                tracing::warn!(path = %path.display(), %error, "cannot read saved file");
            }
        }
    }
}

/// Recompiles and ships the resulting frame. Returns whether a frame was sent.
///
/// On a compile failure an `Error` frame is shipped and the previous good tree
/// is retained — no `Delta` is produced (spec §D.12.3).
pub(crate) fn compile_and_broadcast(shared: &Arc<Shared>) -> bool {
    let (outcome, perf_json) = {
        let mut pipeline = shared.pipeline.lock();
        let outcome = match pipeline.compile() {
            Ok(compiled) => Ok(compiled),
            Err(diagnostic) => {
                tracing::warn!(%diagnostic, "compile failed; retaining previous tree");
                Err(pipeline.error_frame(&diagnostic))
            }
        };
        // Capture the render-perf records from the compile that just ran so they can
        // be broadcast to DevTools as `PerfRecord` telemetry (FLUX-059), independent
        // of whether the frame itself shipped.
        let perf_json: Vec<String> = pipeline
            .perf_records()
            .into_iter()
            .filter_map(|r| r.to_json().ok())
            .collect();
        (outcome, perf_json)
    };
    let sent = match outcome {
        Ok(Compiled::Init(frame)) => {
            shared.broadcast(frame);
            true
        }
        Ok(Compiled::Delta(frame)) => {
            shared.broadcast(frame);
            true
        }
        Err(frame) => {
            shared.broadcast(frame);
            true
        }
        Ok(Compiled::Unchanged) => false,
    };
    // Broadcast the render-perf records to every subscribed DevTools client. These
    // are best-effort: a disconnected client is pruned by `route_telemetry`.
    for json in &perf_json {
        shared
            .devtools_router
            .lock()
            .route_telemetry(&flux_ir_serde::TelemetryEvent::perf_record(json.clone()));
    }
    sent
}

/// Whether `event` is a content-affecting change (create / modify / remove).
fn is_source_change(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    )
}

/// Whether `path` is a `.flux` source file.
fn is_flux_source(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some(FLUX_EXT)
}

/// Recursively collects every `.flux` source under `root`, sorted by path so the
/// assigned [`flux_syntax::FileId`]s are deterministic across restarts.
pub(crate) fn collect_flux_sources(root: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    collect_into(root, &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn collect_into(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_into(&path, out);
        } else if is_flux_source(&path) {
            match std::fs::read_to_string(&path) {
                Ok(source) => out.push((path, source)),
                Err(error) => {
                    tracing::warn!(path = %path.display(), %error, "cannot read source file");
                }
            }
        }
    }
}
