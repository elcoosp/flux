//! Component tree view (spec §5.3): the shadow tree node layout frames.

use std::sync::Arc;

use gpui::{AnyElement, Context, IntoElement, Render, Window};

use flux_ir_serde::Rect;
use flux_syntax::NodeId;

use crate::row::{empty_row, into_any, kv_row, rows_column};
use crate::state::DevToolsState;
use crate::time_travel::ReconstructedState;

/// Renders the live component (shadow) tree as node layout frames.
pub struct ComponentTreeView {
    state: Arc<DevToolsState>,
}

impl ComponentTreeView {
    /// Creates the view bound to the shared state.
    pub fn new(state: Arc<DevToolsState>) -> Self {
        Self { state }
    }

    /// The current reconstructed view frames.
    fn live(&self) -> ReconstructedState {
        self.state.live.read().clone()
    }

    /// Renders the view as a standalone pane.
    pub fn render_pane(&self, _cx: &Context<'_, Self>) -> impl IntoElement {
        let live = self.live();
        let mut rows: Vec<AnyElement> = Vec::new();
        if live.view_frames.is_empty() {
            // Surface the empty state instead of rendering nothing — the host
            // may not be streaming layout frames yet (or the protocol frame is
            // not wired through). This is a UI/data diagnostic, not a crash.
            rows.push(into_any(empty_row("no layout frames received yet")));
        }
        for (id, frame) in live.view_frames.iter() {
            rows.push(into_any(kv_row(
                format!("node#{id}"),
                format!(
                    "{}×{} @ ({}, {})",
                    frame.width, frame.height, frame.x, frame.y
                ),
            )));
        }
        rows_column(rows)
    }
}

impl Render for ComponentTreeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        self.render_pane(cx)
    }
}
