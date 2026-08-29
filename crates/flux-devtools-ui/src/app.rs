//! gpui application entry point (spec §5.1) for the Flux DevTools desktop app.

use std::sync::Arc;

use anyhow::Context as _;
use gpui::{
    App, AppContext, Context, ParentElement, Styled, TitlebarOptions, Window, WindowOptions,
};

use gpui_platform::application;

use crate::state::DevToolsState;
use crate::views::{
    ComponentTreeView, LogViewerView, SignalGraphView, TimelineView, VmInspectorView,
};
use crate::wire_client::{DEFAULT_DEVTOOLS_PORT, connect, run_ingest_loop};

/// The root DevTools window, owning the shared [`DevToolsState`] and the four
/// debugger panes. The panes are created **once** in [`DevToolsRoot::new`] and
/// stored as entities; `render` only references them (gpui views must not be
/// re-created on every frame).
struct DevToolsRoot {
    state: Arc<DevToolsState>,
    last_len: usize,
    vm: gpui::Entity<VmInspectorView>,
    signals: gpui::Entity<SignalGraphView>,
    tree: gpui::Entity<ComponentTreeView>,
    timeline: gpui::Entity<TimelineView>,
    logs: gpui::Entity<LogViewerView>,
}

impl DevToolsRoot {
    fn new(state: Arc<DevToolsState>, cx: &mut Context<'_, Self>) -> Self {
        // The ingest loop runs on a background tokio runtime (see `run_app`) and
        // cannot call into gpui directly. The root view's `render` re-arms a
        // per-frame paint while new telemetry is arriving, which re-reads the
        // shared state and repaints every pane. This avoids any cross-thread
        // `AsyncApp` capture (which this pinned gpui version's spawn trait
        // rejects) and keeps the views live without polling.
        Self {
            state: state.clone(),
            last_len: 0,
            vm: cx.new(|_| VmInspectorView::new(state.clone())),
            signals: cx.new(|_| SignalGraphView::new(state.clone())),
            tree: cx.new(|_| ComponentTreeView::new(state.clone())),
            timeline: cx.new(|_| TimelineView::new(state.clone())),
            logs: cx.new(|_| LogViewerView::new(state.clone())),
        }
    }
}

impl gpui::Render for DevToolsRoot {
    fn render(
        &mut self,
        window: &mut Window,
        _cx: &mut gpui::Context<'_, Self>,
    ) -> impl gpui::IntoElement {
        // Re-arm a repaint so freshly ingested telemetry is reflected. We only keep
        // the animation-frame loop alive while the timeline is still growing, then
        // let it settle — this keeps the debugger live during interaction without
        // spinning at 60fps when idle.
        let len = self.state.timeline_len();
        if len != self.last_len {
            self.last_len = len;
            window.request_animation_frame();
        }

        gpui::div()
            .flex()
            .flex_row()
            .size_full()
            .bg(gpui::white())
            .child(self.vm.clone())
            .child(self.signals.clone())
            .child(self.tree.clone())
            .child(self.timeline.clone())
            .child(self.logs.clone())
    }
}

/// Launches the DevTools application.
///
/// Connects to the dev server's DevTools WebSocket endpoint in a background
/// task (see [`crate::wire_client`]) and opens the debugger window. The ingest
/// loop feeds telemetry into the shared [`DevToolsState`], which the views read
/// on every frame. Returns when the gpui run loop exits.
///
/// The WebSocket I/O runs on its own `tokio` runtime: `tokio-tungstenite`
/// requires a tokio reactor, and gpui's own async executor is not a tokio
/// runtime, so spawning the connect/ingest loop on `cx.spawn` panics with
/// "no reactor running".
///
/// # Errors
///
/// Returns an error if the gpui application or the tokio runtime fails to
/// initialise. A failed WebSocket connection is logged and tolerated — the
/// window still opens and shows whatever telemetry it can receive (AGENTS.md:
/// never crash in prod).
pub fn run_app() -> anyhow::Result<()> {
    // The shared state is an `Arc` so the async ingest loop and the gpui views
    // both hold a cheap clone (the inner `RwLock`s make reads cheap).
    let state = Arc::new(DevToolsState::new());

    // Dedicated tokio runtime for the WebSocket I/O.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("building DevTools tokio runtime")?;
    let ingest_state = state.clone();
    rt.spawn(async move {
        match connect(&format!("127.0.0.1:{DEFAULT_DEVTOOLS_PORT}")).await {
            Ok(stream) => {
                if let Err(e) = run_ingest_loop(stream, ingest_state).await {
                    tracing::warn!(%e, "DevTools ingest loop ended");
                }
            }
            Err(e) => tracing::warn!(%e, "DevTools failed to connect to dev server"),
        }
    });

    let ui_state = state.clone();
    application().run(move |cx: &mut App| {
        let root = cx.new(|cx| DevToolsRoot::new(ui_state.clone(), cx));
        match cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("Flux DevTools".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |_, _| root,
        ) {
            Ok(_handle) => {
                // Become a foreground app so the window is actually visible
                // (a binary launched from the terminal defaults to accessory
                // role and would otherwise show no window).
                cx.activate(true);
            }
            Err(e) => tracing::error!(%e, "failed to open DevTools window"),
        }
    });

    rt.shutdown_timeout(std::time::Duration::from_secs(1));
    Ok(())
}
