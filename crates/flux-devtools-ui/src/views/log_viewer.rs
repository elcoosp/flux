//! Structured log viewer (spec §5.3, FLUX-060): renders the retained log buffer.

use std::sync::Arc;

use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window};

use crate::state::DevToolsState;
use crate::time_travel::LogBuffer;

/// Renders the structured log stream as a scrollable list of `L target: message`
/// lines, newest at the bottom.
pub struct LogViewerView {
    state: Arc<DevToolsState>,
}

impl LogViewerView {
    /// Creates the view bound to the shared state.
    pub fn new(state: Arc<DevToolsState>) -> Self {
        Self { state }
    }

    /// The current retained log buffer.
    fn logs(&self) -> LogBuffer {
        self.state.logs.read().clone()
    }

    /// Renders the view as a standalone pane.
    pub fn render_pane(&self, _cx: &Context<'_, Self>) -> impl IntoElement {
        let logs = self.logs();
        gpui::div()
            .flex()
            .flex_col()
            .p_4()
            .child(gpui::div().child("Logs".to_string()))
            .children(
                logs.snapshot()
                    .into_iter()
                    .map(|entry| gpui::div().child(entry.render())),
            )
    }
}

impl Render for LogViewerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        self.render_pane(cx)
    }
}
