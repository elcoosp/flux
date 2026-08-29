//! gpui application entry point (spec §5.1) for the Flux DevTools desktop app.

use std::sync::Arc;

use anyhow::Context as _;
use gpui::{
    AnyView, App, AppContext, Context, Entity, FontWeight, IntoElement, ParentElement, Render,
    Styled, Window, px,
};
use gpui_component::{
    Root, Theme, TitleBar, badge::Badge, group_box::GroupBox, separator::Separator,
    status_bar::StatusBar,
};

use gpui_platform::application;

use crate::state::{DevToolsState, HostInfo};
use crate::views::{ComponentTreeView, SignalGraphView, TimelineView, VmInspectorView};
use crate::wire_client::{DEFAULT_DEVTOOLS_PORT, connect, run_ingest_loop};

/// The root DevTools window, owning the shared [`DevToolsState`] and the four
/// debugger panes. The panes are created **once** in [`DevToolsRoot::new`] and
/// stored as entities; `render` only references them (gpui views must not be
/// re-created on every frame).
///
/// The window's outermost view is a gpui-component [`Root`] (required by
/// gpui-component for its overlay/dialog layers), and this struct is the content
/// rendered inside it.
struct DevToolsRoot {
    state: Arc<DevToolsState>,
    last_len: usize,
    last_host: Option<HostInfo>,
    vm: Entity<VmInspectorView>,
    signals: Entity<SignalGraphView>,
    tree: Entity<ComponentTreeView>,
    timeline: Entity<TimelineView>,
}

impl DevToolsRoot {
    fn new(state: Arc<DevToolsState>, cx: &mut Context<'_, Self>) -> Self {
        // The ingest loop runs on a background tokio runtime (see `run_app`) and
        // cannot call into gpui directly. The root view's `render` re-arms a
        // per-frame paint while new telemetry is arriving (or the host identity
        // changes), which re-reads the shared state and repaints every pane. This
        // avoids any cross-thread `AsyncApp` capture (which this pinned gpui
        // version's spawn trait rejects) and keeps the views live without polling.
        Self {
            state: state.clone(),
            last_len: 0,
            last_host: None,
            vm: cx.new(|_| VmInspectorView::new(state.clone())),
            signals: cx.new(|_| SignalGraphView::new(state.clone())),
            tree: cx.new(|_| ComponentTreeView::new(state.clone())),
            timeline: cx.new(|_| TimelineView::new(state.clone())),
        }
    }

    /// The current host identity, formatted for display.
    fn host_label(&self) -> Option<String> {
        self.state.host_info().map(|h| h.label())
    }
}

impl Render for DevToolsRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        // Keep the window live while a host is connected: re-arm an animation
        // frame every render so freshly ingested telemetry (VM steps, signals,
        // layout frames) is reflected immediately — including when the DevTools
        // window is in the background while you interact with the host app. macOS
        // would otherwise defer presentation until the window is refocused.
        let len = self.state.timeline_len();
        let host = self.state.host_info();
        self.last_len = len;
        self.last_host = host.clone();
        if host.is_some() {
            window.request_animation_frame();
        }

        let colors = cx.global::<Theme>();
        let host_label = self.host_label();
        let is_connected = host_label.is_some();
        let host_badge = host.as_ref().map(|host| {
            gpui::div()
                .flex()
                .flex_row()
                .gap(px(4.))
                .items_center()
                .child(
                    gpui::div()
                        .w(px(10.))
                        .h(px(10.))
                        .rounded(px(999.))
                        .bg(colors.primary),
                )
                .child(
                    gpui::div()
                        .text_xs()
                        .text_color(colors.foreground)
                        .child(host.label()),
                )
        });

        gpui::div()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors.background)
            .text_color(colors.foreground)
            // ── Top bar: gpui-component TitleBar (auto-indents past macOS traffic
            //    lights) with app title + live host identity ──
            .child(
                TitleBar::new().child(
                    gpui::div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .flex_1()
                        .child(
                            gpui::div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    gpui::div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .child("Flux DevTools"),
                                )
                                .child(Badge::new().child(if is_connected {
                                    "connected"
                                } else {
                                    "no host"
                                })),
                        )
                        .child(host_badge.unwrap_or_else(|| {
                            gpui::div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .pr(px(12.))
                                .child(
                                    gpui::div()
                                        .text_xs()
                                        .text_color(colors.muted_foreground)
                                        .child("awaiting host…"),
                                )
                        })),
                ),
            )
            // ── Body: four gpui-component GroupBox panes separated by dividers ──
            .child(
                gpui::div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(px(0.))
                    .child(pane("VM Inspector", self.vm.clone(), cx))
                    .child(Separator::vertical())
                    .child(pane("Signals", self.signals.clone(), cx))
                    .child(Separator::vertical())
                    .child(pane("Component Tree", self.tree.clone(), cx))
                    .child(Separator::vertical())
                    .child(pane("Timeline", self.timeline.clone(), cx)),
            )
            // ── Bottom status bar ──
            .child(
                StatusBar::new()
                    .left(gpui::div().text_xs().child(format!(
                        "host: {}",
                        host_label.unwrap_or_else(|| "—".into())
                    )))
                    .right(gpui::div().text_xs().child(format!("events: {len}"))),
            )
    }
}

/// Wraps a pane view in a gpui-component [`GroupBox`] (titled, bordered surface)
/// whose body holds the view, so every panel reads as a distinct, polished
/// surface instead of bare text.
fn pane<E: Render + 'static>(title: &'static str, view: Entity<E>, _cx: &App) -> impl IntoElement {
    GroupBox::new()
        .title(title)
        .flex_1()
        .min_w(px(0.))
        .child(gpui::div().flex_col().min_h(px(0.)).child(view))
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
        // gpui-component must be initialised before any of its components are
        // built (sets up theme + overlay globals).
        gpui_component::init(cx);

        let root = cx.new(|cx| DevToolsRoot::new(ui_state.clone(), cx));
        match cx.open_window(TitleBar::window_options(), move |window, cx| {
            cx.new(|cx| Root::new(AnyView::from(root.clone()), window, cx))
        }) {
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
