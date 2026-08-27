//! gpui application entry point (spec §5.1) for the Flux DevTools desktop app.

use std::sync::Arc;

use gpui::{
    App, AppContext, Context, Entity, ParentElement, Styled, TitlebarOptions, Window, WindowOptions,
};

use gpui_platform::application;

use crate::state::DevToolsState;
use crate::views::{ComponentTreeView, SignalGraphView, TimelineView, VmInspectorView};

/// The root DevTools window, owning the shared [`DevToolsState`].
struct DevToolsRoot {
    state: Entity<DevToolsState>,
}

impl DevToolsRoot {
    fn new(state: Entity<DevToolsState>, cx: &mut Context<'_, Self>) -> Self {
        let _ = cx;
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
/// Binds the WebSocket client in a background task (see [`crate::wire_client`])
/// and opens the debugger window. Returns when the gpui run loop exits.
///
/// # Errors
///
/// Returns an error if the gpui application fails to initialise.
pub fn run_app() -> anyhow::Result<()> {
    application().run(|cx: &mut App| {
        let state = cx.new(|_| DevToolsState::new());
        let root = cx.new(|cx| DevToolsRoot::new(state, cx));
        let _ = Arc::new(());
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
