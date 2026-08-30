//! Timeline / flamegraph view (spec §5.3, §6): the time-travel scrubber plus the
//! render-perf flamegraph (FLUX-059 / PRD-J).
//!
//! The scrubber counter (events / scrubbed index) is retained for time-travel,
//! and the flamegraph renders the `MetricRecord` stream the dev server emits as
//! `PerfRecord` telemetry events. The records are consumed verbatim from
//! `flux-perf-harness` (no new wire field), so the bars reflect the same §3.10
//! budgets CI gates against.

use std::sync::Arc;

use gpui::{AnyElement, Context, IntoElement, Render, Window};

use flux_perf_harness::MetricRecord;

use crate::perf_record::{flame_rows, render_pane_rows};
use crate::row::{into_any, kv_row, rows_column};
use crate::state::DevToolsState;

/// Renders the time-travel timeline and the render-perf flamegraph.
pub struct TimelineView {
    state: Arc<DevToolsState>,
    /// Currently scrubbed index (the live edge when `None`).
    scrub_index: Option<usize>,
}

impl TimelineView {
    /// Creates the view bound to the shared state at the live edge.
    pub fn new(state: Arc<DevToolsState>) -> Self {
        Self {
            state,
            scrub_index: None,
        }
    }

    /// Number of retained timeline events.
    fn timeline_len(&self) -> usize {
        self.state.timeline_len()
    }

    /// Renders the view as a standalone pane.
    pub fn render_pane(&self, _cx: &Context<'_, Self>) -> impl IntoElement {
        let len = self.timeline_len();
        let at = self.scrub_index.unwrap_or(len.saturating_sub(1));

        // Scrubber header (time-travel), then the flamegraph of perf records.
        let mut rows: Vec<AnyElement> = vec![
            into_any(kv_row("events", len.to_string())),
            into_any(kv_row("scrubbed", format!("event {at}"))),
        ];

        let records: Vec<MetricRecord> = self.state.perf_records();
        rows.extend(render_pane_rows(&records));

        rows_column(rows)
    }
}

impl Render for TimelineView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        self.render_pane(cx)
    }
}

/// Builds the flamegraph lanes from the state's retained records (exposed for
/// unit tests that render without a display).
#[must_use]
pub fn lanes_for(state: &DevToolsState) -> Vec<crate::perf_record::FlameRow> {
    flame_rows(&state.perf_records())
}
