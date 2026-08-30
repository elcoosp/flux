//! CI gate predicate for the render-perf harness (PRD-J user story 8): fail the
//! build when a §3.10 budget is exceeded.
//!
//! The gate is a pure function over a [`MetricRecord`] — no I/O, fully
//! unit-testable. It is the single place the §3.10 budgets live, so they are
//! enforced, not asserted in prose.

use crate::metric::{LatencyMs, MetricKind, MetricRecord};

/// The §3.10 performance budgets, expressed as maximum **p95** latency per
/// [`MetricKind`]. Values are in milliseconds. These are the numbers PRD-J exists
/// to verify; they are declared here so the gate and the docs share one source.
#[derive(Clone, Copy, Debug)]
pub struct Budgets {
    /// Per-kind p95 ceiling in milliseconds.
    ceilings: [(MetricKind, f64); 8],
}

impl Budgets {
    /// The default §3.10 budgets. `NodeMutation` is the headline "< 3ms" budget;
    /// the others are conservative ceilings for the remaining instrumented signals
    /// (tuned as real baselines land via the harness).
    #[must_use]
    pub fn v1() -> Self {
        Self {
            ceilings: [
                (MetricKind::NodeMutation, 3.0),
                (MetricKind::DirtyReconcileSize, f64::INFINITY), // size, not latency
                (MetricKind::FullReconcileSize, f64::INFINITY),  // size, not latency
                (MetricKind::PatchRoundTrip, 10.0),
                (MetricKind::VmDispatch, 2.0),
                (MetricKind::DevColdStart, 200.0),
                (MetricKind::ReleaseColdStart, 500.0),
                // FLUX-073 loopback save→photon. Generous starting ceiling; the
                // harness records the first measured baseline and we tighten
                // toward the §3.10 "Save → pixels < 100 ms" target as the
                // watcher/debounce contribution is measured and reduced.
                (MetricKind::SaveToPhoton, 250.0),
            ],
        }
    }

    /// The p95 ceiling (ms) for `kind`, or `None` if the kind is not budgeted.
    #[must_use]
    pub fn ceiling_for(&self, kind: MetricKind) -> Option<f64> {
        self.ceilings
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, v)| *v)
    }
}

/// The outcome of evaluating one record against the budgets.
#[derive(Clone, Debug, PartialEq)]
pub struct GateVerdict {
    /// Whether the record passed.
    pub passed: bool,
    /// Human-readable reason (empty when passed).
    pub reason: String,
    /// The observed p95 (ms), for reporting.
    pub observed_p95: f64,
    /// The ceiling (ms) that was checked.
    pub ceiling: f64,
}

/// Evaluates `record` against `budgets`. A record passes when its p95 latency is
/// at or below the configured ceiling for its kind. Size-only kinds (reconcile
/// sizes) are not latency-gated and always pass.
#[must_use]
pub fn evaluate(record: &MetricRecord, budgets: &Budgets) -> GateVerdict {
    let ceiling = match budgets.ceiling_for(record.kind) {
        Some(c) => c,
        None => {
            return GateVerdict {
                passed: true,
                reason: String::new(),
                observed_p95: record.p95().map_or(0.0, |l| l.as_f64()),
                ceiling: f64::INFINITY,
            };
        }
    };

    let observed = record
        .p95()
        .map_or(f64::INFINITY, |l: LatencyMs| l.as_f64());
    let passed = observed <= ceiling;
    GateVerdict {
        passed,
        reason: if passed {
            String::new()
        } else {
            format!(
                "{:?}/{:?} p95 {:.3}ms exceeds §3.10 ceiling {:.3}ms (tree_size {})",
                record.scenario, record.kind, observed, ceiling, record.tree_size
            )
        },
        observed_p95: observed,
        ceiling,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::{FixtureTree, HarnessDriver};
    use crate::metric::LatencyMs;
    use crate::metric::{MetricSample, Scenario};
    use crate::{MeasureFn, MetricRecord};

    fn rec_with_p95(kind: MetricKind, p95: f64, n: usize) -> MetricRecord {
        // Build a record whose p95 equals `p95` (all samples equal -> p95 = p95).
        let samples: Vec<MetricSample> = (0..n)
            .map(|_| MetricSample::latency(LatencyMs::from_raw(p95)))
            .collect();
        MetricRecord::new(Scenario::AndroidDeclarativeDev, kind, 50, samples)
    }

    #[test]
    fn node_mutation_under_3ms_passes() {
        let v = evaluate(
            &rec_with_p95(MetricKind::NodeMutation, 2.5, 10),
            &Budgets::v1(),
        );
        assert!(v.passed);
        assert_eq!(v.reason, "");
    }

    #[test]
    fn node_mutation_over_3ms_fails() {
        let v = evaluate(
            &rec_with_p95(MetricKind::NodeMutation, 4.0, 10),
            &Budgets::v1(),
        );
        assert!(!v.passed);
        assert!(v.reason.contains("exceeds"));
        assert_eq!(v.observed_p95, 4.0);
        assert_eq!(v.ceiling, 3.0);
    }

    #[test]
    fn reconcile_size_kinds_are_not_latency_gated() {
        // Even a "huge" p95 on a size-only kind must pass (size is reported, not gated).
        let v = evaluate(
            &rec_with_p95(MetricKind::DirtyReconcileSize, 999.0, 10),
            &Budgets::v1(),
        );
        assert!(v.passed);
    }

    #[test]
    fn gate_integrates_with_driver() {
        let driver = HarnessDriver::new(FixtureTree::standard(), 8);
        let measure: MeasureFn = Box::new(|_| MetricSample::latency(LatencyMs::from_raw(1.5)));
        let rec = driver.run(
            Scenario::IosImperativeDev,
            MetricKind::NodeMutation,
            &measure,
        );
        let v = evaluate(&rec, &Budgets::v1());
        assert!(v.passed);
        assert_eq!(v.observed_p95, 1.5);
    }
}
