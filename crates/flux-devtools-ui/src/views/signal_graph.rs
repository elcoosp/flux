//! Signal graph view (spec §5.3): the reactive signal cell values.

use gpui::{Context, Entity, IntoElement, ParentElement, Render, Styled, Window};

use crate::state::DevToolsState;
use crate::time_travel::ReconstructedState;

/// Renders the live signal graph as a table of `(signal_id, value)` pairs.
pub struct SignalGraphView {
    state: Entity<DevToolsState>,
}

impl SignalGraphView {
    /// Creates the view bound to the shared state.
    pub fn new(state: Entity<DevToolsState>) -> Self {
        Self { state }
    }

    /// The current reconstructed signal state.
    fn live(&self, cx: &Context<'_, Self>) -> ReconstructedState {
        self.state.read(cx).live.read().clone()
    }

    /// Renders the view as a standalone pane.
    pub fn render_pane(&self, cx: &Context<'_, Self>) -> impl IntoElement {
        let live = self.live(cx);
        gpui::div()
            .flex()
            .flex_col()
            .p_4()
            .child(gpui::div().child("Signals".to_string()))
            .children(live.signals.iter().map(|(id, value)| {
                gpui::div()
                    .flex()
                    .justify_between()
                    .child(gpui::div().child(format!("sig#{id}")))
                    .child(gpui::div().child(format!("{value:?}")))
            }))
    }
}

impl Render for SignalGraphView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        self.render_pane(cx)
    }
}
