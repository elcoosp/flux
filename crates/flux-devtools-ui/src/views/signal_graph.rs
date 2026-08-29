//! Signal graph view (spec §5.3): the reactive signal cell values.

use std::sync::Arc;

use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window};

use crate::state::DevToolsState;
use crate::time_travel::ReconstructedState;

/// Renders the live signal graph as a table of `(signal_id, value)` pairs.
pub struct SignalGraphView {
    state: Arc<DevToolsState>,
}

impl SignalGraphView {
    /// Creates the view bound to the shared state.
    pub fn new(state: Arc<DevToolsState>) -> Self {
        Self { state }
    }

    /// The current reconstructed signal state.
    fn live(&self) -> ReconstructedState {
        self.state.live.read().clone()
    }

    /// Renders the view as a standalone pane.
    pub fn render_pane(&self, _cx: &Context<'_, Self>) -> impl IntoElement {
        let live = self.live();
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
            // Dependency edges: which effects re-run when each signal changes
            // (PRD-P user story 2 — "what reads" a signal).
            .child(gpui::div().child("Edges".to_string()))
            .children(live.signal_edges.iter().map(|(id, readers)| {
                let readers: Vec<String> = readers.iter().map(|e| format!("fx#{e}")).collect();
                gpui::div().child(format!(
                    "sig#{id} → {}",
                    if readers.is_empty() {
                        "∅".to_string()
                    } else {
                        readers.join(", ")
                    }
                ))
            }))
    }
}

impl Render for SignalGraphView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        self.render_pane(cx)
    }
}
