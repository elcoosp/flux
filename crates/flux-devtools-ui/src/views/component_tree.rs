//! Component tree view (spec §5.3): the shadow tree node layout frames.

use gpui::{Context, Entity, IntoElement, ParentElement, Render, Styled, View, Window};

use flux_ir_serde::Rect;
use flux_syntax::NodeId;

use crate::state::DevToolsState;
use crate::time_travel::ReconstructedState;

/// Renders the live component (shadow) tree as node layout frames.
pub struct ComponentTreeView {
    state: Entity<DevToolsState>,
}

impl ComponentTreeView {
    /// Creates the view bound to the shared state.
    pub fn new(state: Entity<DevToolsState>) -> Self {
        Self { state }
    }

    /// The current reconstructed view frames.
    fn live(&self, cx: &Context<Self>) -> ReconstructedState {
        self.state.read(cx).live.read().clone()
    }

    /// Renders the view as a standalone pane.
    pub fn render_pane(&self, cx: &Context<Self>) -> impl IntoElement {
        let live = self.live(cx);
        gpui::div()
            .flex()
            .flex_col()
            .p_4()
            .child(gpui::div().child("Component Tree".to_string()))
            .children(live.view_frames.iter().map(|(id, frame): &(NodeId, Rect)| {
                gpui::div()
                    .flex()
                    .justify_between()
                    .child(gpui::div().child(format!("node#{id}")))
                    .child(gpui::div().child(format!(
                        "{}×{} @ ({},{})",
                        frame.width, frame.height, frame.x, frame.y
                    )))
            }))
    }
}

impl Render for ComponentTreeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_pane(cx)
    }
}
