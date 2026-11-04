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
    ///
    /// Sets the shared flag the loop polls; the thread exits within one
    /// `debounce` window (it may be blocked on `recv_timeout`).
    pub(crate) fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

impl Drop for Watcher {
    /// Ensures the watch thread is signalled to stop when a `Watcher` is
    /// dropped without an explicit [`Watcher::stop`] call — the contract the
    /// type's own doc-comment promised but previously did not deliver.
    ///
    /// `RunningServer::drop` already calls `stop()`, so this is defensive:
    /// it covers a standalone `Watcher` (a test or a future caller) and keeps
    /// the doc-comment honest.
    fn drop(&mut self) {
        self.stop();
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
                if error.kind() == std::io::ErrorKind::NotFound {
                    if let Some(file_id) = shared.pipeline.lock().file_id_for_path(&path) {
                        shared.pipeline.lock().remove_file(file_id);
                    }
                    tracing::info!(path = %path.display(), "deleted .flux file removed from pipeline");
                } else {
                    tracing::warn!(path = %path.display(), %error, "cannot read saved file");
                }
            }
        }
    }
}

/// Recompiles and ships the resulting frame. Returns whether a frame was sent.
///
/// On a compile failure an `Error` frame is shipped and the previous good tree
/// is retained — no `Delta` is produced (spec §D.12.3).
///
/// Audit H16: the frame is broadcast while the pipeline lock is still held so
/// that a Hello arriving between compile and broadcast cannot observe a stale
/// sequence (Init(new) then Delta(old→new)). The broadcast sender is
/// non-blocking (`broadcast::Sender::send` is synchronous and cheap), so it is
/// safe to call while holding the `parking_lot::Mutex`.
pub(crate) fn compile_and_broadcast(shared: &Arc<Shared>) -> bool {
    let mut pipeline = shared.pipeline.lock();
    let outcome = match pipeline.compile() {
        Ok(compiled) => Ok(compiled),
        Err(diagnostic) => {
            tracing::warn!(%diagnostic, "compile failed; retaining previous tree");
            Err(pipeline.error_frame(&diagnostic))
        }
    };
    let perf_json: Vec<String> = pipeline
        .perf_records()
        .into_iter()
        .filter_map(|r| r.to_json().ok())
        .collect();
    // Round-21 fix: capture the refreshed DevTools SourceMap while the
    // pipeline lock is still held. The router was built once at startup from
    // the *pre-compile* pipeline, so without this its map stayed empty forever
    // and every enriched telemetry event resolved to "no source mapping".
    let refreshed_map = outcome.is_ok().then(|| pipeline.devtools_source_map());
    // Audit H16: broadcast the frame while the pipeline lock is still held
    // so a concurrent Hello cannot observe Init(new) then Delta(old→new).
    let sent = match &outcome {
        Ok(Compiled::Init(frame)) => {
            shared.broadcast(frame.clone());
            true
        }
        Ok(Compiled::Delta(frame)) => {
            shared.broadcast(frame.clone());
            true
        }
        Err(frame) => {
            shared.broadcast(frame.clone());
            true
        }
        Ok(Compiled::Unchanged) => false,
    };
    drop(pipeline);
    if let Some(map) = refreshed_map {
        shared.devtools_router.lock().set_source_map(map);
    }
    // Broadcast the render-perf records to every subscribed DevTools client.
    // These are best-effort: a disconnected client is pruned by
    // `route_telemetry`.
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
    // Audit fix: `Access` events (open/close/read) come through as
    // `EventKind::Access(_)` on inotify — they fire whenever anything opens
    // or reads the file (`ls`, LSP polls, editor temp reads). A read-only
    // access must not trigger a recompile, so we filter Access out and only
    // accept content-affecting kinds.
    match event.kind {
        EventKind::Access(_) => false,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => true,
        _ => false,
    }
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

/// Directories that never contain user `.flux` sources and are walked past
/// regardless of a project's layout. Ported from `flux-cli/src/sources.rs`.
const SKIP_DIRS: &[&str] = &[".git", "target", "node_modules", "platforms", ".build"];

fn collect_into(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
    // Round-21 fix: guard identical to `flux-cli/src/sources.rs`. The walk
    // previously followed symlinked directories, so a directory symlinked to
    // one of its ancestors (or to itself) recursed until the process stack
    // limit and aborted the dev server at startup; walking `.git` / `target` /
    // `node_modules` / `platforms` also burned startup time and could surface
    // unrelated `.flux` files from VCS internals, build output or dependency
    // trees as project sources.
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // `path.is_symlink()` checks the entry itself; use it to prevent
        // following a directory symlink into a loop.
        if path.is_symlink() {
            continue;
        }
        if path.is_dir() {
            let skip = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| SKIP_DIRS.contains(&n));
            if skip {
                continue;
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a fresh scratch project directory with two real sources.
    fn scratch_root(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("flux-watch-walk-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        std::fs::write(dir.join("main.flux"), "compo Hello\n  Text(\"hi\")\n")
            .expect("write source");
        std::fs::create_dir_all(dir.join("nested")).expect("nested dir");
        std::fs::write(
            dir.join("nested/inner.flux"),
            "compo Inner\n  Text(\"in\")\n",
        )
        .expect("write inner source");
        dir
    }

    #[test]
    fn symlink_loop_does_not_hang_or_crash_the_source_walk() {
        // Round-21 fix: `collect_into` followed symlinked directories, so a
        // directory symlink pointing at an ancestor (or at itself) recursed
        // toward the stack limit at startup, and a symlink pointing at a real
        // sibling made the same source file collected twice (once per path) —
        // the devserver then lowered it under two FileIds. The walk must skip
        // symlinked entries entirely and terminate promptly.
        let root = scratch_root("loop");
        // Loop shapes: a directory symlinked to its own ancestor and to itself.
        std::os::unix::fs::symlink(&root, root.join("loop")).expect("ancestor symlink loop");
        std::os::unix::fs::symlink(root.join("nested"), root.join("nested/self"))
            .expect("self symlink loop");
        // A symlink to a REAL directory holding a .flux file: pre-fix this
        // collected `nested/inner.flux` a second time as `mirror/inner.flux`.
        std::os::unix::fs::symlink(root.join("nested"), root.join("mirror"))
            .expect("dir symlink to real dir");
        // A symlinked .flux FILE: pre-fix this collected `main.flux` twice.
        std::os::unix::fs::symlink(root.join("main.flux"), root.join("alias.flux"))
            .expect("file symlink");

        let started = std::time::Instant::now();
        let sources = collect_flux_sources(&root);
        // Recursion guard (timeout-ish): a fix-less walk on a platform without
        // a path-length backstop never gets here; a prompt return proves the
        // walk did not chase the loops unboundedly.
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "source walk must terminate promptly with symlink loops present"
        );
        let paths: Vec<PathBuf> = sources.iter().map(|(p, _)| p.clone()).collect();
        assert_eq!(
            paths,
            vec![root.join("main.flux"), root.join("nested/inner.flux")],
            "real sources are collected exactly once; symlinked entries are skipped"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn walk_skips_vcs_build_and_dependency_directories() {
        // Round-21 fix: the walk descended into `.git` / `target` /
        // `node_modules` / `platforms`, burning startup time and surfacing
        // unrelated `.flux` files from build output or dependencies.
        let root = scratch_root("skip");
        for dir in [".git", "target", "node_modules", "platforms"] {
            std::fs::create_dir_all(root.join(dir)).expect("noise dir");
            std::fs::write(
                root.join(dir).join("stray.flux"),
                format!("compo Stray{dir}\n  Text(\"s\")\n"),
            )
            .expect("write stray source");
        }
        let sources = collect_flux_sources(&root);
        let paths: Vec<PathBuf> = sources.iter().map(|(p, _)| p.clone()).collect();
        assert_eq!(
            paths,
            vec![root.join("main.flux"), root.join("nested/inner.flux")],
            "only user sources are collected; SKIP_DIRS entries are walked past"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
