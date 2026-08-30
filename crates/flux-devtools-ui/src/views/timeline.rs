//! Timeline / flamegraph view (spec §5.3, §6): the time-travel scrubber plus the
//! render-perf flamegraph (FLUX-059 / PRD-J).
//!
//! The scrubber is a gpui-component [`Slider`] bound to [`DevToolsState::scrub_index`]:
//! dragging it reconstructs the VM state at that timeline index (real
//! time-travel, ADR-0042), so other panes can reflect a scrubbed point instead
//! of only the live edge. The flamegraph renders the `MetricRecord` stream the
//! dev server emits as `PerfRecord` telemetry events.

use std::sync::Arc;

use gpui::{
    AnyElement, Context, Entity, IntoElement, ParentElement, Render, Subscription, Window, div,
    prelude::*, px,
};
use gpui_component::slider::{Slider, SliderEvent, SliderState, SliderValue};

use flux_perf_harness::MetricRecord;

use crate::perf_record::render_pane_rows;
use crate::row::{into_any, kv_row, rows_column};
use crate::state::DevToolsState;

/// Renders the time-travel timeline and the render-perf flamegraph.
pub struct TimelineView {
    state: Arc<DevToolsState>,
    /// The slider's backing state entity (range 0..timeline_len).
    slider: Entity<SliderState>,
    /// Subscription to slider changes so we can write `scrub_index`.
    _sub: Subscription,
}

impl TimelineView {
    /// Creates the view bound to the shared state at the live edge.
    pub fn new(state: Arc<DevToolsState>, cx: &mut Context<'_, Self>) -> Self {
        let len = state.timeline_len();
        let slider = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max((len.max(1) - 1) as f32)
                .step(1.0)
                .default_value((len.max(1) - 1) as f32)
        });
        let sub = cx.subscribe(&slider, {
            let state = state.clone();
            move |_, _, event: &SliderEvent, cx| {
                let value = match event {
                    SliderEvent::Change(v) | SliderEvent::Release(v) => *v,
                };
                let idx = match value {
                    SliderValue::Single(f) => f as usize,
                    _ => 0,
                };
                state.set_scrub_index(Some(idx));
                cx.notify();
            }
        });
        Self {
            state,
            slider,
            _sub: sub,
        }
    }

    /// Number of retained timeline events.
    fn timeline_len(&self) -> usize {
        self.state.timeline_len()
    }

    /// Renders the view as a standalone pane.
    pub fn render_pane(&mut self, _cx: &mut Context<'_, Self>) -> impl IntoElement {
        let len = self.timeline_len();
        let live = len.max(1) - 1;
        let scrub = self.state.scrub_index().unwrap_or(live);
        let at = scrub.min(live);

        // Scrubber header (time-travel) + a reconstructed VM snapshot at the
        // scrubbed index, then the flamegraph of perf records.
        let mut rows: Vec<AnyElement> = vec![
            into_any(kv_row("events", len.to_string())),
            into_any(kv_row("scrubbed", format!("event {at}"))),
        ];

        // Reconstruct the VM state at the scrubbed index to make time-travel
        // tangible (ADR-0042): show the IP/registers as they were at that point.
        if let Some(snapshot) = self.state.state_at(at) {
            let offset = snapshot
                .bytecode_offset
                .map_or_else(|| "?".into(), |o| format!("0x{o:04X}"));
            rows.push(into_any(kv_row("scrub IP", offset)));
            rows.push(into_any(kv_row(
                "scrub gas",
                snapshot
                    .gas_remaining
                    .map_or_else(|| "?".into(), |g| g.to_string()),
            )));
        }

        let records: Vec<MetricRecord> = self.state.perf_records();
        rows.extend(render_pane_rows(&records));

        div()
            .flex_col()
            .gap(px(6.))
            .child(
                div()
                    .px(crate::row::ROW_PAD_X)
                    .py(px(4.))
                    .child(Slider::new(&self.slider)),
            )
            .child(rows_column(rows))
            .into_any_element()
    }
}

impl Render for TimelineView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        self.render_pane(cx)
    }
}

impl Drop for TimelineView {
    fn drop(&mut self) {
        // Clear the scrub so a freshly opened window follows the live edge.
        self.state.set_scrub_index(None);
    }
}
