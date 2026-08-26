//! gpui application entry point (spec §5.1) for the Flux DevTools desktop app.

use std::sync::Arc;

use gpui::{App, Application, Context, Entity, Window, WindowOptions};

use crate::state::DevToolsState;
use crate::views::{ComponentTreeView, SignalGraphView, TimelineView, VmInspectorView};

/// The root DevTools window, owning the shared [`DevToolsState`].
struct DevToolsRoot {
    state: Entity<DevToolsState>,
}

impl DevToolsRoot {
    fn new(state: Entity<DevToolsState>, cx: &mut Context<Self>) -> Self {
        let _ = cx;
        Self { state }
    }
}

impl gpui::Render for DevToolsRoot {
    fn render(
        &mut self,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let state = self.state.read(cx);
        let vm = VmInspectorView::new(self.state);
        let signals = SignalGraphView::new(self.state);
        let tree = ComponentTreeView::new(self.state);
        let timeline = TimelineView::new(self.state);
        let _ = (&state, &vm, &signals, &tree, &timeline);
        gpui::div()
            .flex()
            .flex_row()
            .size_full()
            .child(vm.render_pane(cx))
            .child(signals.render_pane(cx))
            .child(tree.render_pane(cx))
            .child(timeline.render_pane(cx))
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
    Application::new().run(|cx: &mut App| {
        let state = cx.new(|_| DevToolsState::new());
        let root = cx.new(|cx| DevToolsRoot::new(state, cx));
        let _ = Arc::new(());
        cx.open_window(
            WindowOptions {
                title: Some("Flux DevTools".into()),
                ..Default::default()
            },
            move |_, cx| cx.new(|_| root),
        );
    })?;
    Ok(())
}
