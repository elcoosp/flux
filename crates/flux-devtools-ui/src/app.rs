//! gpui application entry point (spec §5.1) for the Flux DevTools desktop app.

use std::sync::Arc;

use gpui::{
    App, AppContext, Context, ParentElement, Styled, TitlebarOptions, Window, WindowOptions,
};

use gpui_platform::application;

use crate::state::DevToolsState;
use crate::views::{ComponentTreeView, SignalGraphView, TimelineView, VmInspectorView};
use crate::wire_client::{DEFAULT_DEVTOOLS_PORT, connect, run_ingest_loop};

/// The root DevTools window, owning the shared [`DevToolsState`].
struct DevToolsRoot {
    state: Arc<DevToolsState>,
}

impl DevToolsRoot {
    fn new(state: Arc<DevToolsState>, _cx: &mut Context<'_, Self>) -> Self {
        Self { state }
    }
}

impl gpui::Render for DevToolsRoot {
    fn render(
        &mut self,
        _window: &mut Window,
        cx: &mut gpui::Context<'_, Self>,
    ) -> impl gpui::IntoElement {
        let vm = cx.new(|_| VmInspectorView::new(self.state.clone()));
        let signals = cx.new(|_| SignalGraphView::new(self.state.clone()));
        let tree = cx.new(|_| ComponentTreeView::new(self.state.clone()));
        let timeline = cx.new(|_| TimelineView::new(self.state.clone()));
        gpui::div()
            .flex()
            .flex_row()
            .size_full()
            .child(vm)
            .child(signals)
            .child(tree)
            .child(timeline)
    }
}

/// Launches the DevTools application.
///
/// Connects to the dev server's DevTools WebSocket endpoint in a background
/// task (see [`crate::wire_client`]) and opens the debugger window. The ingest
/// loop feeds telemetry into the shared [`DevToolsState`], which the views read
/// on every frame. Returns when the gpui run loop exits.
///
/// # Errors
///
/// Returns an error if the gpui application fails to initialise. A failed
/// WebSocket connection is logged and tolerated — the window still opens and
/// shows whatever telemetry it can receive (AGENTS.md: never crash in prod).
pub fn run_app() -> anyhow::Result<()> {
    application().run(|cx: &mut App| {
        // The shared state is an `Arc` so the async ingest loop and the gpui
        // views both hold a cheap clone (the inner `RwLock`s make reads cheap).
        let state = Arc::new(DevToolsState::new());
        let root = cx.new(|cx| DevToolsRoot::new(state.clone(), cx));

        // Drive the telemetry ingest loop on a background async task, feeding the
        // shared state as frames arrive from the dev server.
        let ingest_state = state.clone();
        cx.spawn(async move |_cx| {
            match connect(&format!("127.0.0.1:{DEFAULT_DEVTOOLS_PORT}")).await {
                Ok(stream) => {
                    if let Err(e) = run_ingest_loop(stream, ingest_state).await {
                        tracing::warn!(%e, "DevTools ingest loop ended");
                    }
                }
                Err(e) => tracing::warn!(%e, "DevTools failed to connect to dev server"),
            }
        })
        .detach();

        let _ = cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("Flux DevTools".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |_, _| root,
        );
    });
    Ok(())
}
