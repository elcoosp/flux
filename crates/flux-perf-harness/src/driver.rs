//! Deterministic harness driver (PRD-J): a fixed warm fixture tree plus a
//! measurement loop that produces a [`MetricRecord`] from a host-supplied timing
//! closure.
//!
//! The driver is platform-neutral Rust and fully unit-testable: it does **not**
//! touch UI, the network, or a device. The host adapters (in `runtimes/ios` and
//! `runtimes/android`) supply the actual `measure` closure that times the real
//! reconciler / VM; the driver owns the fixed-tree construction, the sample loop,
//! and the percentile aggregation so every platform reports identically-shaped
//! data (PRD-J: "one source of truth", "not flaky — fixed warm tree, p50/p95").

use crate::metric::{MetricKind, MetricRecord, MetricSample, Scenario};

/// A fixed, deterministic fixture tree used for every measurement run.
///
/// `node_count` leaves are arranged under a balanced binary spine so the tree is
/// identical across runs and across platforms — the measurement must vary, not
/// the input. The tree is intentionally just a count + depth description; the
/// host adapter decides how to materialize it (Compose views, UIKit views, codegen
/// output). Keeping it data-only means the driver needs no UI dependency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixtureTree {
    /// Total node count (leaves + internal spine).
    pub node_count: u64,
    /// Tree depth (longest root→leaf path), used by adapters for scroll/list
    /// fixtures.
    pub depth: u16,
}

impl FixtureTree {
    /// A default ~50-node subtree matching the §3.10 budget wording
    /// ("~50-node subtree").
    #[must_use]
    pub fn standard() -> Self {
        Self {
            node_count: 50,
            depth: 6,
        }
    }

    /// Builds a fixture tree from an explicit node count, deriving a sane depth.
    #[must_use]
    pub fn with_nodes(node_count: u64) -> Self {
        let depth = ((node_count as f64).log2().max(1.0).ceil()) as u16;
        Self { node_count, depth }
    }
}

/// A timing closure supplied by a host adapter: runs one measurement against the
/// provided fixture tree and returns the observed sample. The closure owns all
/// platform specifics (reconcile a dirty node, dispatch a handler, round-trip a
/// patch).
pub type MeasureFn = Box<dyn Fn(&FixtureTree) -> MetricSample + Send + Sync>;

/// The harness driver: owns the fixture tree and the sample-collection loop.
#[derive(Clone, Debug)]
pub struct HarnessDriver {
    tree: FixtureTree,
    sample_count: usize,
}

impl HarnessDriver {
    /// Creates a driver for `tree`, collecting `sample_count` samples per run.
    #[must_use]
    pub fn new(tree: FixtureTree, sample_count: usize) -> Self {
        debug_assert!(sample_count > 0, "sample_count must be positive");
        Self { tree, sample_count }
    }

    /// The fixed fixture tree this driver measures against.
    #[must_use]
    pub fn tree(&self) -> FixtureTree {
        self.tree
    }

    /// Runs `measure` `sample_count` times against the fixed tree and returns a
    /// complete [`MetricRecord`] for `(scenario, kind)`.
    ///
    /// The loop is deterministic: same tree, same count, no randomness, so the
    /// resulting percentiles are reproducible (PRD-J Testing Decisions).
    #[must_use]
    pub fn run(&self, scenario: Scenario, kind: MetricKind, measure: &MeasureFn) -> MetricRecord {
        let samples: Vec<MetricSample> = (0..self.sample_count)
            .map(|_| measure(&self.tree))
            .collect();
        MetricRecord::new(scenario, kind, self.tree.node_count, samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metric::LatencyMs;

    #[test]
    fn driver_collects_requested_sample_count() {
        let driver = HarnessDriver::new(FixtureTree::standard(), 12);
        // A fake measure that returns a constant latency (no device needed).
        let measure: MeasureFn = Box::new(|_| MetricSample::latency(LatencyMs::from_raw(2.0)));
        let rec = driver.run(
            Scenario::AndroidDeclarativeDev,
            MetricKind::NodeMutation,
            &measure,
        );
        assert_eq!(rec.samples.len(), 12);
        assert_eq!(rec.tree_size, 50);
        assert_eq!(rec.p50().map(|l| l.as_f64()), Some(2.0));
    }

    #[test]
    fn driver_passes_tree_to_measure() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU64, Ordering};

        let driver = HarnessDriver::new(FixtureTree::with_nodes(200), 3);
        let seen = Arc::new(AtomicU64::new(0));
        let seen_clone = seen.clone();
        let measure: MeasureFn = Box::new(move |t| {
            seen_clone.store(t.node_count, Ordering::SeqCst);
            MetricSample::with_size(LatencyMs::from_raw(1.0), t.node_count)
        });
        let rec = driver.run(
            Scenario::IosImperativeDev,
            MetricKind::FullReconcileSize,
            &measure,
        );
        assert_eq!(seen.load(Ordering::SeqCst), 200);
        assert_eq!(rec.size_p50(), Some(200));
    }

    #[test]
    #[should_panic]
    fn zero_samples_is_rejected() {
        let _ = HarnessDriver::new(FixtureTree::standard(), 0);
    }
}
