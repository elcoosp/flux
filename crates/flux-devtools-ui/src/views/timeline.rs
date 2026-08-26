//! Timeline scrubber view (spec §5.3, §6): the time-travel slider.

use gpui::{Context, Entity, IntoElement, ParentElement, Render, Styled, View, Window};

use crate::state::DevToolsState;

/// Renders the time-travel timeline: a scrubber over the retained history.
pub struct TimelineView {
    state: Entity<DevToolsState>,
    /// Currently scrubbed index (the live edge when `None`).
    scrub_index: Option<usize>,
}

impl TimelineView {
    /// Creates the view bound to the shared state at the live edge.
    pub fn new(state: Entity<DevToolsState>) -> Self {
        Self {
            state,
            scrub_index: None,
        }
    }

    /// Number of retained timeline events.
    fn timeline_len(&self, cx: &Context<Self>) -> usize {
        self.state.read(cx).timeline_len()
    }

    /// Renders the view as a standalone pane.
    pub fn render_pane(&self, cx: &Context<Self>) -> impl IntoElement {
        let len = self.timeline_len(cx);
        let at = self.scrub_index.unwrap_or(len.saturating_sub(1));
        gpui::div()
            .flex()
            .flex_col()
            .p_4()
            .child(gpui::div().child("Timeline".to_string()))
            .child(gpui::div().child(format!("event {at} / {len}")))
    }
}

impl Render for TimelineView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_pane(cx)
    }
}
