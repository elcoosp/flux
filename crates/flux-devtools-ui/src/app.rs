//! gpui application entry point (spec §5.1) for the Flux DevTools desktop app.

use std::sync::Arc;

use anyhow::Context as _;
use gpui::prelude::*;
use gpui::{
    AnyView, App, AppContext, Context, ElementId, Entity, FontWeight, IntoElement, ParentElement,
    Render, StyleRefinement, Styled, Window, px,
};
use gpui_component::{
    Root, Theme, TitleBar, Icon, IconName, badge::Badge, group_box::GroupBox,
    resizable::{h_resizable, resizable_panel, v_resizable},
    scroll::ScrollableElement, status_bar::StatusBar,
};

use gpui_platform::application;

use crate::state::{DevToolsState, HostInfo};
use crate::views::{
    ComponentTreeView, LogViewerView, NetworkInspectorView, SignalGraphView, TimelineView,
    VmInspectorView,
};
use crate::wire_client::{DEFAULT_DEVTOOLS_PORT, connect, run_ingest_loop};

/// A single DevTools pane: a themed, titled, scrollable surface built on
/// gpui-component's [`GroupBox`] (outline variant → bordered + radius via the
/// active theme), with an `IconName` next to a bold title and a vertically
/// scrollable body.
///
/// **Why a macro (not a function):** gpui-component's `Scrollable` keys its
/// `ScrollHandle` by `caller_id()` (the source location of the
/// `overflow_y_scrollbar()` call). If all six panes called a shared `pane()`
/// function, they would share ONE scroll handle — scrolling the VM pane would
/// move the Component Tree pane, and vice-versa. A `macro_rules!` expands each
/// invocation at a DISTINCT source location, so each pane gets its own handle.
///
/// The body is given `flex_1()` inside a GroupBox whose content is forced to
/// grow (`content_style.flex_grow = 1`), so the scroll viewport has a real
/// height and a visible vertical scrollbar renders.
macro_rules! devtools_pane {
    ($title:expr, $icon:expr, $view:expr, $colors:expr) => {{
        let title: &'static str = $title;
        let mut content_style = StyleRefinement::default();
        content_style.flex_grow = Some(1.0);
        GroupBox::new()
            .flex_1()
            .min_w(px(0.))
            .border_1()
            .border_color($colors.border)
            .content_style(content_style)
            .title(
                gpui::div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .child(Icon::new($icon).size_4().text_color($colors.foreground))
                    .child(
                        gpui::div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color($colors.foreground)
                            .child(title.to_string()),
                    ),
            )
            .child(
                gpui::div()
                    .id(ElementId::from(title))
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scrollbar()
                    .child($view),
            )
            .into_any_element()
    }};
}

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
    logs: Entity<LogViewerView>,
    net: Entity<NetworkInspectorView>,
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
            timeline: cx.new(|cx| TimelineView::new(state.clone(), cx)),
            logs: cx.new(|_| LogViewerView::new(state.clone())),
            net: cx.new(|_| NetworkInspectorView::new(state.clone())),
        }
    }

    /// The current host identity, formatted for display.
    fn host_label(&self) -> Option<String> {
        self.state.host_info().map(|h| h.label())
    }
}

impl Render for DevToolsRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        // Re-arm a paint only when something actually changed. Continuously
        // requesting an animation frame (every render) forced a 60fps full
        // re-render of every pane, which reset scroll positions to the top each
        // frame. Guarding on data/host change keeps the window live as telemetry
        // arrives without that churn. (macOS still repaints on focus/resize.)
        let len = self.state.timeline_len();
        let host = self.state.host_info();
        let changed = len != self.last_len || host != self.last_host;
        self.last_len = len;
        self.last_host = host.clone();
        if changed {
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
            // ── Body: six resizable panes. Two columns (left: Component Tree /
            //    Logs / Network; right: VM / Signals / Timeline) separated by a
            //    draggable vertical splitter, each column a vertical resizable
            //    stack of three panes. Users can now resize to prioritise what
            //    matters instead of a fixed 3×2 grid. ──
            .child(
                h_resizable("devtools-workspace")
                    .child(
                        resizable_panel()
                            .size(px(380.))
                            .child(
                                v_resizable("devtools-left")
                                    .child(devtools_pane!("Component Tree", IconName::Inspector, self.tree.clone(), colors))
                                    .child(devtools_pane!("Logs", IconName::File, self.logs.clone(), colors))
                                    .child(devtools_pane!("Network", IconName::Network, self.net.clone(), colors)),
                            ),
                    )
                    .child(
                        resizable_panel()
                            .child(
                                v_resizable("devtools-right")
                                    .child(devtools_pane!("VM Inspector", IconName::Cpu, self.vm.clone(), colors))
                                    .child(devtools_pane!("Signals", IconName::Bot, self.signals.clone(), colors))
                                    .child(devtools_pane!("Timeline", IconName::ChartPie, self.timeline.clone(), colors)),
                            ),
                    ),
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
        // Reconnect forever: the dev server (and its host) come and go as the
        // user edits / restarts `flux dev`. A single failed handshake or a
        // dropped socket must not kill telemetry permanently — retry with a
        // short backoff so the DevTools window reconnects on its own.
        let addr = format!("127.0.0.1:{DEFAULT_DEVTOOLS_PORT}");
        loop {
            match connect(&addr).await {
                Ok(stream) => {
                    if let Err(e) = run_ingest_loop(stream, ingest_state.clone()).await {
                        eprintln!("DevTools ingest loop ended: {e}");
                    }
                    // Socket closed: brief pause before reconnecting.
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
                Err(e) => {
                    eprintln!("DevTools failed to connect to dev server ({e}); retrying");
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
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
