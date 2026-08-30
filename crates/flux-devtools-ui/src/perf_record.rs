//! Render-perf flamegraph model + view for the timeline pane (FLUX-059 / PRD-J).
//!
//! Consumes the render-perf harness [`MetricRecord`]s (PRD-J's canonical schema,
//! `flux-perf-harness`) that the dev server streams as `PerfRecord` telemetry
//! events. There is no new wire field — the records arrive verbatim and this
//! module only *reads* them, so DevTools and CI share one source of truth
//! (AGENTS.md §3.10 / PRD-J Implementation Decisions).
//!
//! The flamegraph groups records by `(Scenario, MetricKind)` — one lane per
//! measurement — and draws each lane's p95 latency against its §3.10 budget
//! ceiling. A lane whose bar reaches or exceeds 100% of its ceiling is drawn in
//! red (over budget); within budget it is green. The bar width is the budget
//! ratio on a linear scale so "looks full == at the limit" reads at a glance.

use flux_perf_harness::{evaluate, Budgets, LatencyMs, MetricKind, MetricRecord, Scenario};

use gpui::{px, AnyElement, Div, ParentElement, Styled};

use crate::row::{empty_row, into_any, kv_row};

/// One flamegraph lane: a single `(scenario, kind)` measurement and its derived
/// percentiles + budget verdict.
#[derive(Clone, Debug, PartialEq)]
pub struct FlameRow {
    /// The rendering tier / host / execution path the record describes.
    pub scenario: Scenario,
    /// The kind of measurement.
    pub kind: MetricKind,
    /// Tree size the record was taken against (node count).
    pub tree_size: u64,
    /// p50 latency (ms), or `None` if the record had no samples.
    pub p50: Option<f64>,
    /// p95 latency (ms), or `None` if the record had no samples.
    pub p95: Option<f64>,
    /// p99 latency (ms), or `None` if the record had no samples.
    pub p99: Option<f64>,
    /// The §3.10 p95 ceiling (ms) for this kind, or `None` if not budgeted.
    pub ceiling: Option<f64>,
    /// Whether the p95 passed the §3.10 budget.
    pub passed: bool,
}

impl FlameRow {
    /// The p95 latency as a fraction of its budget ceiling (1.0 == exactly at
    /// the limit). `None` when the kind is unbudgeted or has no samples.
    #[must_use]
    pub fn budget_ratio(&self) -> Option<f64> {
        match (self.p95, self.ceiling) {
            (Some(p95), Some(ceiling)) if ceiling > 0.0 => Some(p95 / ceiling),
            _ => None,
        }
    }
}

fn latency_opt(l: Option<LatencyMs>) -> Option<f64> {
    l.map(|v| v.as_f64())
}

/// Builds the flamegraph lanes from the retained [`MetricRecord`] stream.
///
/// Records are grouped by `(scenario, kind)`; when more than one record shares a
/// key, the **last** in arrival order wins (a later harness run supersedes an
/// earlier one for the same measurement). Lanes are ordered by `MetricKind`
/// then `Scenario` so the timeline reads consistently across sessions.
#[must_use]
pub fn flame_rows(records: &[MetricRecord]) -> Vec<FlameRow> {
    let budgets = Budgets::v1();
    let mut by_key: Vec<((Scenario, MetricKind), &MetricRecord)> = Vec::new();
    for record in records {
        // Replace any earlier record for the same (scenario, kind) key.
        if let Some(slot) = by_key
            .iter_mut()
            .find(|((sc, kd), _)| *sc == record.scenario && *kd == record.kind)
        {
            slot.1 = record;
        } else {
            by_key.push(((record.scenario, record.kind), record));
        }
    }
    let mut rows: Vec<FlameRow> = by_key
        .into_iter()
        .map(|((scenario, kind), record)| {
            let verdict = evaluate(record, &budgets);
            FlameRow {
                scenario,
                kind,
                tree_size: record.tree_size,
                p50: latency_opt(record.p50()),
                p95: latency_opt(record.p95()),
                p99: latency_opt(record.p99()),
                ceiling: budgets.ceiling_for(kind),
                passed: verdict.passed,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        let kind_a = format!("{:?}", a.kind);
        let kind_b = format!("{:?}", b.kind);
        kind_a
            .cmp(&kind_b)
            .then_with(|| format!("{:?}", a.scenario).cmp(&format!("{:?}", b.scenario)))
    });
    rows
}

/// Renders the flamegraph lanes as a scrollable column of budget-aware bars.
/// Returns an empty-state row when there are no records yet.
pub fn render_flamegraph(rows: &[FlameRow]) -> Vec<AnyElement> {
    if rows.is_empty() {
        return vec![into_any(empty_row(
            "awaiting render-perf MetricRecord stream (PerfRecord telemetry)",
        ))];
    }
    rows.iter().map(render_lane).collect()
}

/// Renders one flamegraph lane: a label, a budget-aware bar, and its
/// p50/p95/p99 readout.
fn render_lane(row: &FlameRow) -> AnyElement {
    let ratio = row.budget_ratio().unwrap_or(0.0);
    // Clamp the drawn width to 100% of the track; an over-budget lane fills the
    // track and is colored red so it reads as "at/over the limit".
    let fill = ratio.clamp(0.0, 1.0);
    // 240px track keeps labels readable; the fill is a fraction of that.
    const TRACK_PX: f32 = 240.0;
    let fill_px = px(TRACK_PX * fill as f32);

    let bar_color = if row.passed {
        gpui::rgb(0x3f_b47a)
    } else {
        gpui::rgb(0xe5_4d4d)
    };

    let p50 = row.p50.map_or("—".into(), |v| format!("{v:.3}"));
    let p95 = row.p95.map_or("—".into(), |v| format!("{v:.3}"));
    let p99 = row.p99.map_or("—".into(), |v| format!("{v:.3}"));
    let ceiling = row.ceiling.map_or("∞".into(), |c| format!("{c:.1}ms"));

    let label = format!("{:?}/{:?}", row.scenario, row.kind);
    let stats = format!("p50 {p50} · p95 {p95} · p99 {p99} · ceil {ceiling}");

    let bar: Div = gpui::div()
        .h(px(10.0))
        .w(fill_px)
        .rounded(px(2.0))
        .bg(bar_color);

    let lane: Div = gpui::div()
        .flex()
        .flex_col()
        .px(crate::row::ROW_PAD_X)
        .py(crate::row::ROW_PAD_Y)
        .border_b(px(1.0))
        .border_color(gpui::white().opacity(0.08))
        .child(
            gpui::div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    gpui::div()
                        .text_color(gpui::white().opacity(0.9))
                        .child(label),
                )
                .child(
                    gpui::div()
                        .text_xs()
                        .text_color(gpui::white().opacity(0.55))
                        .child(stats),
                ),
        )
        .child(
            gpui::div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(
                    gpui::div()
                        .w(px(TRACK_PX))
                        .h(px(10.0))
                        .rounded(px(2.0))
                        .bg(gpui::white().opacity(0.1))
                        .child(bar),
                )
                .child(
                    gpui::div()
                        .text_xs()
                        .text_color(if row.passed {
                            gpui::rgb(0x3f_b47a)
                        } else {
                            gpui::rgb(0xe5_4d4d)
                        })
                        .child(if row.passed { "OK" } else { "OVER" }),
                ),
        );
    into_any(lane)
}

/// Renders the full timeline/flamegraph pane body: a summary header plus the
/// flamegraph lanes, for `render_pane` to drop into the pane's scroll body.
#[must_use]
pub fn render_timeline_body(rows: &[FlameRow], record_count: usize) -> Vec<AnyElement> {
    let over = rows.iter().filter(|r| !r.passed).count();
    let mut out: Vec<AnyElement> = vec![
        into_any(kv_row("perf records", record_count.to_string())),
        into_any(kv_row("lanes", rows.len().to_string())),
        into_any(kv_row("over budget", over.to_string())),
    ];
    out.extend(render_flamegraph(rows));
    out
}

/// Convenience: build the rows from a record slice and render the pane body.
#[must_use]
pub fn render_pane_rows(records: &[MetricRecord]) -> Vec<AnyElement> {
    let rows = flame_rows(records);
    let count = records.len();
    render_timeline_body(&rows, count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flux_perf_harness::{LatencyMs, MetricRecord, MetricSample, Scenario};

    fn record(scenario: Scenario, kind: MetricKind, p95: f64) -> MetricRecord {
        let samples = vec![MetricSample::latency(LatencyMs::from_raw(p95))];
        MetricRecord::new(scenario, kind, 50, samples)
    }

    #[test]
    fn empty_stream_renders_empty_state() {
        let rows = flame_rows(&[]);
        assert!(rows.is_empty());
        let body = render_pane_rows(&[]);
        // Header rows (3) + one empty-state row.
        assert_eq!(body.len(), 4);
    }

    #[test]
    fn over_budget_lane_is_flagged_red() {
        // NodeMutation ceiling is 3.0ms; a 4.0ms record must fail the budget.
        let rows = flame_rows(&[record(
            Scenario::AndroidDeclarativeDev,
            MetricKind::NodeMutation,
            4.0,
        )]);
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].passed);
        assert!((rows[0].budget_ratio().unwrap() - 4.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn within_budget_lane_passes() {
        let rows = flame_rows(&[record(
            Scenario::IosImperativeDev,
            MetricKind::NodeMutation,
            1.5,
        )]);
        assert!(rows[0].passed);
    }

    #[test]
    fn duplicate_key_takes_latest_record() {
        // Two records for the same (scenario, kind): the later one wins.
        let stream = vec![
            record(Scenario::LoopbackE2e, MetricKind::SaveToPhoton, 10.0),
            record(Scenario::LoopbackE2e, MetricKind::SaveToPhoton, 80.0),
        ];
        let rows = flame_rows(&stream);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].p95, Some(80.0));
    }

    #[test]
    fn distinct_keys_make_distinct_lanes() {
        let stream = vec![
            record(
                Scenario::AndroidDeclarativeDev,
                MetricKind::NodeMutation,
                1.0,
            ),
            record(Scenario::AndroidDeclarativeDev, MetricKind::VmDispatch, 1.0),
            record(Scenario::IosImperativeDev, MetricKind::NodeMutation, 1.0),
        ];
        let rows = flame_rows(&stream);
        assert_eq!(rows.len(), 3);
    }
}
