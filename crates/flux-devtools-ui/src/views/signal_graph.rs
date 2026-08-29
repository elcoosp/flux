//! Signal graph view (spec §5.3): the reactive signal cell values.

use std::sync::Arc;

use gpui::{AnyElement, Context, IntoElement, Render, Window};

use crate::row::{empty_row, into_any, kv_row, rows_column};
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
        let mut rows: Vec<AnyElement> = Vec::new();
        if live.signals.is_empty() && live.signal_edges.is_empty() {
            rows.push(into_any(empty_row("no signals yet")));
        }
        for (id, value) in live.signals.iter() {
            rows.push(into_any(kv_row(format!("sig#{id}"), format!("{value:?}"))));
        }
        // Dependency edges: which effects re-run when each signal changes
        // (PRD-P user story 2 — "what reads" a signal).
        for (id, readers) in live.signal_edges.iter() {
            let readers: Vec<String> = readers.iter().map(|e| format!("fx#{e}")).collect();
            let readers = if readers.is_empty() {
                "∅".to_string()
            } else {
                readers.join(", ")
            };
            rows.push(into_any(kv_row(format!("sig#{id} →"), readers)));
        }
        rows_column(rows)
    }
}

impl Render for SignalGraphView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        self.render_pane(cx)
    }
}
