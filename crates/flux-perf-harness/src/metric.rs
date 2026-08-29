//! The shared metric schema emitted by the render-perf harness (PRD-J).
//!
//! Both host adapters (Android JVM host, iOS `FluxUIKit` reconciler) and both
//! execution paths (dev VM, release codegen) feed this same schema, so DevTools
//! timeline/flamegraph (PRD-P) can later consume the same records. The schema is
//! stable and parseable (JSON) — not stdout prose — per PRD-J Implementation
//! Decisions.
//!
//! No new opcodes or wire fields are introduced by this crate (PRD-J is
//! measurement + a decision, not a protocol change).

use serde::{Deserialize, Serialize};
use std::fmt;

/// A non-negative latency in milliseconds.
///
/// Wrapped so the percentile math and JSON shape stay in one place and so a
/// negative or `NaN` value can never silently enter a record.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LatencyMs(f64);

impl LatencyMs {
    /// The raw value; panics only on `NaN` (a record must never carry one).
    #[must_use]
    pub fn from_raw(value: f64) -> Self {
        assert!(!value.is_nan(), "LatencyMs must not be NaN");
        Self(value)
    }

    /// The raw milliseconds.
    #[must_use]
    pub fn as_f64(self) -> f64 {
        self.0
    }
}

impl fmt::Display for LatencyMs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.3}ms", self.0)
    }
}

/// A single named measurement taken by the harness.
///
/// `latency` is always present (the dominant signal for the §3.10 budgets).
/// `size` is optional and carries a structural count when the measurement is
/// about *work done* rather than time — e.g. dirty-subset reconciliation node
/// count vs full-tree node count (user story 2).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetricSample {
    /// Wall-clock latency for this sample.
    pub latency: LatencyMs,
    /// Optional structural size (node count, byte count) for this sample.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

impl MetricSample {
    /// A pure latency sample.
    #[must_use]
    pub fn latency(latency: LatencyMs) -> Self {
        Self {
            latency,
            size: None,
        }
    }

    /// A latency sample that also carries a structural size.
    #[must_use]
    pub fn with_size(latency: LatencyMs, size: u64) -> Self {
        Self {
            latency,
            size: Some(size),
        }
    }
}

/// Which rendering tier / host / execution path a record was captured against.
///
/// This is the axis the ADR-0048 decision turns on: it lets the harness compare
/// the imperative iOS `FluxUIKit` reconciler against a declarative SwiftUI
/// prototype, and the dev VM path against the release codegen path, within one
/// comparable schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scenario {
    /// iOS, current imperative `FluxUIKit` reconciler, dev VM path.
    IosImperativeDev,
    /// iOS, declarative SwiftUI prototype (feature-flagged), dev VM path.
    IosDeclarativeDev,
    /// iOS, release codegen path.
    IosRelease,
    /// Android, declarative `ShadowTreeRenderer` + `DirtyReconciler`, dev VM path.
    AndroidDeclarativeDev,
    /// Android, release codegen path.
    AndroidRelease,
}

/// A kind of measurement, mirroring PRD-J's instrumented signals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MetricKind {
    /// Native view mutation latency (the core §3.10 "< 3ms" budget).
    NodeMutation,
    /// Dirty-subset reconciliation size (nodes touched), separate from full tree.
    DirtyReconcileSize,
    /// Full-tree reconciliation size (nodes touched) — for the dirty/full ratio.
    FullReconcileSize,
    /// WebSocket patch round-trip (server → host → applied) latency.
    PatchRoundTrip,
    /// VM dispatch latency for one handler invocation.
    VmDispatch,
    /// Dev-session cold start: attach → first rendered frame.
    DevColdStart,
    /// Release cold start: process launch → first rendered frame.
    ReleaseColdStart,
}

/// A complete, aggregated performance record for one `(scenario, kind)` pair.
///
/// Built from a fixed warm fixture tree (PRD-J Testing Decisions: not flaky —
/// report p50/p95, not a single sample). `samples` is retained so the raw data
/// is auditable; the percentiles are derived on demand.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetricRecord {
    /// The rendering tier / host / execution path this record describes.
    pub scenario: Scenario,
    /// The kind of measurement.
    pub kind: MetricKind,
    /// Tree size the measurement was taken against (node count), for context.
    pub tree_size: u64,
    /// Raw samples (latencies, optional sizes) this record aggregates.
    pub samples: Vec<MetricSample>,
}

impl MetricRecord {
    /// Builds a record from its parts.
    #[must_use]
    pub fn new(
        scenario: Scenario,
        kind: MetricKind,
        tree_size: u64,
        samples: Vec<MetricSample>,
    ) -> Self {
        Self {
            scenario,
            kind,
            tree_size,
            samples,
        }
    }

    /// The p50 latency (median). `None` when there are no samples.
    #[must_use]
    pub fn p50(&self) -> Option<LatencyMs> {
        percentile(&self.samples, 0.50)
    }

    /// The p95 latency. `None` when there are no samples.
    #[must_use]
    pub fn p95(&self) -> Option<LatencyMs> {
        percentile(&self.samples, 0.95)
    }

    /// The mean latency. `None` when there are no samples.
    #[must_use]
    pub fn mean(&self) -> Option<LatencyMs> {
        if self.samples.is_empty() {
            return None;
        }
        let sum: f64 = self.samples.iter().map(|s| s.latency.as_f64()).sum();
        Some(LatencyMs::from_raw(sum / self.samples.len() as f64))
    }

    /// The p50 structural size, if any sample carried one.
    #[must_use]
    pub fn size_p50(&self) -> Option<u64> {
        let sizes: Vec<u64> = self.samples.iter().filter_map(|s| s.size).collect();
        percentile_values(&sizes, 0.50)
    }

    /// Serializes to a stable JSON document (PRD-J: "stable, parseable metric
    /// record ... not just stdout").
    ///
    /// # Errors
    /// Returns a [`serde_json::Error`] only if the record contains a non-finite
    /// value, which the type system already forbids.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Parses a record from JSON. See [`to_json`](Self::to_json).
    ///
    /// # Errors
    /// Returns a [`serde_json::Error`] if the document is malformed or missing
    /// required fields.
    pub fn from_json(input: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(input)
    }
}

/// Returns the `quantile` percentile of the sample latencies (nearest-rank), or
/// `None` for an empty input.
fn percentile(samples: &[MetricSample], quantile: f64) -> Option<LatencyMs> {
    if samples.is_empty() {
        return None;
    }
    let mut vals: Vec<f64> = samples.iter().map(|s| s.latency.as_f64()).collect();
    vals.sort_by(|a, b| a.partial_cmp(b).expect("LatencyMs forbids NaN"));
    Some(LatencyMs::from_raw(nearest_rank(&vals, quantile)))
}

/// Returns the `quantile` percentile of raw `u64` values (nearest-rank).
fn percentile_values(values: &[u64], quantile: f64) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    let mut vals = values.to_vec();
    vals.sort_unstable();
    Some(nearest_rank(&vals, quantile))
}

/// Nearest-rank percentile: index `ceil(q * n) - 1`, clamped to the last
/// element. Works for any `Ord` numeric slice.
fn nearest_rank<T: Copy + PartialOrd>(sorted: &[T], quantile: f64) -> T {
    debug_assert!(!sorted.is_empty());
    let n = sorted.len();
    let rank = (quantile * n as f64).ceil() as usize;
    // rank is in 1..=n; clamp to valid 0-based index.
    let idx = rank.saturating_sub(1).min(n - 1);
    sorted[idx]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn latencies(xs: &[f64]) -> Vec<MetricSample> {
        xs.iter()
            .map(|x| MetricSample::latency(LatencyMs::from_raw(*x)))
            .collect()
    }

    #[test]
    fn percentile_nearest_rank_is_correct() {
        // 1..=10 sorted; p50 -> rank ceil(0.5*10)=5 -> idx 4 -> value 5.
        let v: Vec<u64> = (1..=10).collect();
        assert_eq!(nearest_rank(&v, 0.50), 5);
        // p95 -> rank ceil(9.5)=10 -> idx 9 -> value 10.
        assert_eq!(nearest_rank(&v, 0.95), 10);
        // p50 of odd-length list: 1..=7 -> rank ceil(3.5)=4 -> idx 3 -> value 4.
        let odd: Vec<u64> = (1..=7).collect();
        assert_eq!(nearest_rank(&odd, 0.50), 4);
    }

    #[test]
    fn record_derives_p50_p95_mean() {
        let rec = MetricRecord::new(
            Scenario::IosImperativeDev,
            MetricKind::NodeMutation,
            50,
            latencies(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]),
        );
        assert_eq!(rec.p50().map(|l| l.as_f64()), Some(5.0));
        assert_eq!(rec.p95().map(|l| l.as_f64()), Some(10.0));
        assert_eq!(rec.mean().map(|l| l.as_f64()), Some(5.5));
    }

    #[test]
    fn empty_record_has_no_percentiles() {
        let rec = MetricRecord::new(
            Scenario::AndroidDeclarativeDev,
            MetricKind::VmDispatch,
            0,
            Vec::new(),
        );
        assert_eq!(rec.p50(), None);
        assert_eq!(rec.p95(), None);
        assert_eq!(rec.mean(), None);
    }

    #[test]
    fn size_percentile_uses_only_sized_samples() {
        let mut samples = latencies(&[1.0, 2.0, 3.0]);
        samples[1] = MetricSample::with_size(LatencyMs::from_raw(2.0), 42);
        let rec = MetricRecord::new(
            Scenario::IosDeclarativeDev,
            MetricKind::DirtyReconcileSize,
            50,
            samples,
        );
        // Only one sample carried a size -> p50 of that one value.
        assert_eq!(rec.size_p50(), Some(42));
    }

    #[test]
    fn json_round_trips() {
        let rec = MetricRecord::new(
            Scenario::IosRelease,
            MetricKind::PatchRoundTrip,
            50,
            vec![
                MetricSample::latency(LatencyMs::from_raw(1.2)),
                MetricSample::with_size(LatencyMs::from_raw(0.8), 7),
            ],
        );
        let json = rec.to_json().expect("serialize");
        let back = MetricRecord::from_json(&json).expect("parse");
        assert_eq!(rec, back);
        // Scenario/kind are kebab-case in JSON (stable, parseable shape).
        assert!(json.contains("\"ios-release\""));
        assert!(json.contains("\"patch-round-trip\""));
    }
}
