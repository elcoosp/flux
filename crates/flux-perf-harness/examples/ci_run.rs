//! CI entry point for the render-perf harness (PRD-J).
//!
//! Builds a small in-process sample record and runs the §3.10 budget gate
//! (`GateVerdict`) so CI proves the harness crate compiles and executes end to
//! end on a fixed warm tree. This is a *demonstration* of the pipeline, not a
//! measurement of a real device run — the per-tier numbers come from the host
//! adapters (runtimes/) wiring `MeasureFn` closures, which is parallel-owned
//! work. See `crates/flux-perf-harness/README` / PRD-J.

use flux_perf_harness::{
    gate::{Budgets, evaluate},
    metric::{LatencyMs, MetricKind, MetricRecord, MetricSample, Scenario},
};

fn main() {
    // A fixed warm fixture (mirrors HarnessDriver::new(FixtureTree::with_nodes(N))).
    const TREE_SIZE: u64 = 200;

    // Node-mutation latencies well under the 3 ms §3.10 budget.
    let mut samples = Vec::new();
    for ms in [1.1_f64, 1.4, 0.9, 2.1, 1.0, 1.7, 0.8, 1.2] {
        samples.push(MetricSample::latency(LatencyMs::from_raw(ms)));
    }

    let record = MetricRecord::new(
        Scenario::IosImperativeDev,
        MetricKind::NodeMutation,
        TREE_SIZE,
        samples,
    );

    let verdict = evaluate(&record, &Budgets::v1());
    println!(
        "scenario={:?} kind={:?} samples={} p50={:?} p95={:?} verdict={:?}",
        record.scenario,
        record.kind,
        record.samples.len(),
        record.p50().map(|l| l.as_f64()),
        record.p95().map(|l| l.as_f64()),
        verdict,
    );

    // Emit the record as stable JSON so a downstream job can archive/compare it.
    match serde_json::to_string_pretty(&record) {
        Ok(json) => println!("metric_record_json:\n{json}"),
        Err(e) => eprintln!("failed to serialize record: {e}"),
    }

    // The demonstration run always exits 0; this binary proves the harness
    // *executes*. The hard budget gate is enforced by `cargo test -p
    // flux-perf-harness` (gate::tests) and by host-adapter CI once the
    // runtimes/ measurement wires in.
    println!("perf-harness demonstration run complete");
}
