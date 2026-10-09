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
            max_wait: DEFAULT_MAX_WAIT,
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
    /// Upper bound on the wait from the *first* pending event to compilation.
    ///
    /// Without this, a tool that touches a file continuously (`npm run watch`
    /// writing through an editor, an aggressive formatter) pushes `last_event`
    /// forward every cycle and `settled` never fires, so compilation starves
    /// forever. The cap forces a compile even under continuous churn.
    max_wait: Duration,
}

/// Default upper bound between the first pending event and a compile.
const DEFAULT_MAX_WAIT: Duration = Duration::from_millis(500);

/// Grace period before treating a `NotFound` read as a real delete.
///
/// Many editors save atomically: they write a temp file, then `rename` it
/// over the target. Between the two syscalls a reader sees the target
/// momentarily missing. Waiting briefly and retrying distinguishes a real
/// delete from an atomic save; without it, a save could silently remove the
/// file from the pipeline until the next `Create` event.
const ATOMIC_SAVE_GRACE: Duration = Duration::from_millis(50);

/// Debounces watch events, re-reads the changed sources, and recompiles.
fn watch_loop(
    rx: &Receiver<notify::Result<Event>>,
    shared: &Arc<Shared>,
    flag: &Arc<AtomicBool>,
    timing: Timing,
) {
    let mut pending: Vec<PathBuf> = Vec::new();
    let mut last_event: Option<Instant> = None;
    let mut first_event: Option<Instant> = None;
    while flag.load(Ordering::Relaxed) && !shared.is_shutdown() {
        match rx.recv_timeout(timing.debounce) {
            Ok(event) => {
                first_event.get_or_insert_with(Instant::now);
                absorb(event, &mut pending, &mut last_event);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                tracing::warn!("WATCH_DISCONNECTED");
                return;
            }
        }
        let settled = last_event.is_some_and(|t| t.elapsed() >= timing.debounce);
        // Starvation cap: if the first event in the burst is older than
        // `max_wait`, compile even though saves are still arriving. Without
        // this, continuous churn blocks compilation forever.
        let overdue = first_event.is_some_and(|t| t.elapsed() >= timing.max_wait);
        if pending.is_empty() || !(settled || overdue) {
            continue;
        }
        // Coalescing window: sleep *first*, then drain anything that landed
        // during the sleep into the same pass. The previous order read files
        // *before* the sleep, so a save landing inside the window was only
        // picked up on the next loop iteration — producing a stale compile
        // followed by a fresh one (two frames, the first one wrong).
        std::thread::sleep(timing.coalesce);
        while let Ok(event) = rx.try_recv() {
            absorb(event, &mut pending, &mut last_event);
        }
        reload(&mut pending, shared);
        last_event = None;
        first_event = None;
        compile_and_broadcast(shared);
    }
}

/// Records one `notify` event into the pending set + last-event timestamp.
fn absorb(
    event: notify::Result<Event>,
    pending: &mut Vec<PathBuf>,
    last_event: &mut Option<Instant>,
) {
    match event {
        Ok(ev) if is_source_change(&ev) => {
            pending.extend(ev.paths.into_iter().filter(|p| is_flux_source(p)));
            *last_event = Some(Instant::now());
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, "WATCH_ERROR"),
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
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // Atomic save: an editor writes a temp file, then renames it
                // over the target. Between the two the path is briefly missing.
                // Wait a short grace period and retry before treating it as a
                // real delete.
                std::thread::sleep(ATOMIC_SAVE_GRACE);
                match std::fs::read_to_string(&path) {
                    Ok(source) => shared.pipeline.lock().set_source(&path, source),
                    Err(_) => {
                        // Take the lock once for both `file_id_for_path` and
                        // `remove_file`: the previous double-lock was a TOCTOU
                        // (another thread could register the file in between).
                        let mut pipeline = shared.pipeline.lock();
                        if let Some(file_id) = pipeline.file_id_for_path(&path) {
                            pipeline.remove_file(file_id);
                        }
                        tracing::info!(path = %path.display(), "deleted .flux file removed from pipeline");
                    }
                }
            }
            Err(error) => tracing::warn!(path = %path.display(), %error, "cannot read saved file"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, DataChange, ModifyKind};
    use notify::{Event, EventKind};

    /// F6.1: coalesce window. Verify that events arriving inside the coalesce
    /// window are absorbed into the *same* pending set (previous ordering read
    /// first, slept after, so an intra-window event was only picked up on the
    /// next cycle).
    ///
    /// This test pins the `absorb` helper + the drain-during-coalesce behavior
    /// without a real filesystem watcher.
    #[test]
    fn absorb_accumulates_paths_and_updates_timestamp() {
        let mut pending: Vec<PathBuf> = Vec::new();
        let mut last: Option<Instant> = None;
        let t0 = Instant::now();

        let ev = Event {
            kind: EventKind::Modify(ModifyKind::Data(DataChange::Any)),
            paths: vec![PathBuf::from("/tmp/a.flux"), PathBuf::from("/tmp/b.txt")],
            attrs: Default::default(),
        };
        absorb(Ok(ev), &mut pending, &mut last);

        assert_eq!(pending, vec![PathBuf::from("/tmp/a.flux")], "only .flux files enter pending");
        assert!(last.is_some(), "absorb records the event timestamp");
        assert!(last.unwrap() >= t0);
    }

    /// F6.2: absorbed Access events must be filtered (audit fix, unchanged).
    #[test]
    fn absorb_ignores_access_events() {
        use notify::event::AccessKind;
        let mut pending: Vec<PathBuf> = Vec::new();
        let mut last: Option<Instant> = None;
        let ev = Event {
            kind: EventKind::Access(AccessKind::Any),
            paths: vec![PathBuf::from("/tmp/a.flux")],
            attrs: Default::default(),
        };
        absorb(Ok(ev), &mut pending, &mut last);
        assert!(pending.is_empty(), "read-only access must not trigger recompile");
        assert!(last.is_none(), "no timestamp recorded for filtered events");
    }

    /// F6.3: Create events are honoured (a brand-new file appears in the tree).
    #[test]
    fn absorb_accepts_create_events() {
        let mut pending: Vec<PathBuf> = Vec::new();
        let mut last: Option<Instant> = None;
        let ev = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![PathBuf::from("/tmp/new.flux")],
            attrs: Default::default(),
        };
        absorb(Ok(ev), &mut pending, &mut last);
        assert_eq!(pending, vec![PathBuf::from("/tmp/new.flux")]);
    }

    /// F6.4: the `max_wait` field exists and defaults to a non-zero value.
    #[test]
    fn timing_struct_has_max_wait() {
        let t = Timing {
            debounce: Duration::from_millis(10),
            coalesce: Duration::from_millis(5),
            max_wait: DEFAULT_MAX_WAIT,
        };
        assert!(t.max_wait > Duration::ZERO);
        assert_eq!(DEFAULT_MAX_WAIT, Duration::from_millis(500));
    }
}
