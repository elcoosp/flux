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
use gpui_component::ActiveTheme as _;
use gpui_component::slider::{Slider, SliderEvent, SliderState, SliderValue};

use flux_perf_harness::MetricRecord;


use crate::row::{into_any, kv_row, rows_column};
use crate::state::DevToolsState;

/// Renders the time-travel timeline and the render-perf flamegraph.
pub struct TimelineView {
    state: Arc<DevToolsState>,
    /// The slider's backing state entity (range 0..timeline_len).
    slider: Entity<SliderState>,
    /// Subscription to slider changes so we can write `scrub_index`.
    _sub: Subscription,
    /// The `timeline_len` value the current slider Entity was built for.
    /// When this changes in `render_pane`, the slider is rebuilt with the
    /// new range and re-subscribed. The previous one-shot init in `new()`
    /// ran at window-open time — *before* any telemetry had arrived — so the
    /// slider range was permanently `0..=0` and time-travel scrubbing was
    /// dead on arrival (ADR-0042 headline UX). Rebuilding on length change
    /// keeps the range live as telemetry flows in.
    last_len: usize,

    /// Cached flamegraph rows. Rebuilt only when the state's perf-record
    /// generation changes — the audit flagged that the previous path cloned
    /// the whole `perf_records` buffer (up to 1024 `MetricRecord`s) and
    /// re-deduped + re-sorted on every render. With the cache, a repaint
    /// with no new perf data reuses the previous `Vec<FlameRow>` untouched.
    cached_flame_rows: Vec<crate::perf_record::FlameRow>,
    /// Generation this cache was built for; `u64::MAX` means "never built",
    /// forcing a first-frame compute.
    cached_generation: u64,
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
            last_len: len,
            cached_flame_rows: Vec::new(),
            cached_generation: u64::MAX,
        }
    }

    /// Number of retained timeline events.
    fn timeline_len(&self) -> usize {
        self.state.timeline_len()
    }

    /// Renders the view as a standalone pane.
    pub fn render_pane(&mut self, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let len = self.timeline_len();

        // Audit fix: rebuild the slider when the timeline length changes so
        // its `max` tracks the live event count. The previous single-shot
        // init in `new()` produced a 0..=0 range for the whole session, which
        // made the time-travel scrubber unusable as soon as the window
        // opened (before any telemetry had arrived). Rebuilding here also
        // re-subscribes so the new entity's Change/Release events write
        // `scrub_index`; the old `_sub` is dropped, so no leaked observers.
        if len != self.last_len {
            self.last_len = len;
            let state_for_sub = self.state.clone();
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(0.0)
                    .max((len.max(1) - 1) as f32)
                    .step(1.0)
                    .default_value((len.max(1) - 1) as f32)
            });
            let sub = cx.subscribe(&slider, move |_, _, event: &SliderEvent, cx| {
                let value = match event {
                    SliderEvent::Change(v) | SliderEvent::Release(v) => *v,
                };
                let idx = match value {
                    SliderValue::Single(f) => f as usize,
                    _ => 0,
                };
                state_for_sub.set_scrub_index(Some(idx));
                cx.notify();
            });
            self.slider = slider;
            self._sub = sub;
        }

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

        // Audit fix (perf): cache the flamegraph rows across frames, keyed on
        // the state's perf-record generation counter. The previous path cloned
        // the whole record buffer (up to 1024 `MetricRecord`s) and rebuilt +
        // sorted the flame rows on every repaint, even when no new perf data
        // had arrived. The generation counter (bumped only on ingest) lets
        // this view reuse its cache until telemetry actually changes.
        let generation = self.state.perf_record_generation();
        if generation != self.cached_generation {
            self.cached_generation = generation;
            let records: Vec<MetricRecord> = self.state.perf_records();
            self.cached_flame_rows = crate::perf_record::flame_rows(&records);
        }
        let record_count = self.state.perf_record_count();
        rows.extend(crate::perf_record::render_timeline_body(
            &self.cached_flame_rows,
            record_count,
        ));
        // Background color and overflow clipping for clean pane isolation.
        div()
            .flex()
            .flex_col()
            .flex_1()
            .overflow_hidden()
            .bg(cx.theme().background)
            .child(
                div()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .px(crate::row::ROW_PAD_X)
                            .py(px(4.))
                            .child(Slider::new(&self.slider)),
                    )
                    .child(rows_column(rows)),
            )
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
