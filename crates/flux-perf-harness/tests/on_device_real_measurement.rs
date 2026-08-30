//! Real measurement path for the render-perf harness (FLUX-066).
//!
//! Unlike `examples/ci_run.rs` (a fixed-warm demonstration record), this test
//! drives the harness's own [`HarnessDriver`] through a genuine [`MeasureFn`]
//! that times the reference VM's dispatch of a counter handler (the same
//! `READ_SIGNAL; ADD_I64; WRITE_SIGNAL; HALT` shape the hosts execute on a tap).
//! The resulting [`MetricRecord`] carries *measured* latencies, and the §3.10
//! budget gate is evaluated against them — proving the whole pipeline (driver →
//! measure → record → gate) works on real numbers, not a hardcoded sample.
//!
//! The measurement is pure Rust (no device needed), so it runs on every
//! `cargo test -p flux-perf-harness`. The on-device tier timings live in the
//! host adapters (`runtimes/`); this is the harness-core half of the proof.

use flux_perf_harness::{
    driver::{FixtureTree, HarnessDriver, MeasureFn},
    gate::{Budgets, evaluate},
    metric::{LatencyMs, MetricKind, MetricRecord, MetricSample, Scenario},
};
use flux_syntax::Value;
use flux_vm_ref::{InMemorySignals, run};

/// `READ_SIGNAL r0, 1 ; LOAD_INT_CONST r1, 1 ; ADD_I64 r0, r0, r1 ;
///  WRITE_SIGNAL 1, r0 ; HALT` — the canonical increment handler.
const INCREMENT: &[u8] = &[
    0x10, 0x00, 0x01, 0x00, 0x00, 0x00, // READ_SIGNAL r0, signal 1
    0xB0, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // LOAD_INT_CONST r1, 1
    0x20, 0x00, 0x00, 0x01, // ADD_I64 r0, r0, r1
    0x11, 0x01, 0x00, 0x00, 0x00, 0x00, // WRITE_SIGNAL signal 1, r0
    0x00, // HALT
];

/// Times one reference-VM dispatch of the increment handler. Returns a
/// `MetricSample` whose latency is the wall-clock cost of evaluating the
/// handler against a live signal graph.
fn measure_vm_dispatch(_tree: &FixtureTree) -> MetricSample {
    let mut signals = InMemorySignals::from_signals([(1u32, Value::Int(0))]);
    let start = std::time::Instant::now();
    let _ = run(INCREMENT, &mut signals, Value::Null);
    let elapsed_ms = start.elapsed().as_secs_f64() * 1_000.0;
    MetricSample::latency(LatencyMs::from_raw(elapsed_ms))
}

#[test]
fn harness_runs_real_vm_dispatch_measurement() {
    let driver = HarnessDriver::new(FixtureTree::standard(), 200);
    let measure: MeasureFn = Box::new(measure_vm_dispatch);
    let record: MetricRecord = driver.run(Scenario::LoopbackE2e, MetricKind::VmDispatch, &measure);

    // The record must carry exactly the requested samples...
    assert_eq!(record.samples.len(), 200);
    // ...and every sample must be a finite, non-negative latency (a real
    // measurement, not a placeholder).
    for s in &record.samples {
        assert!(s.latency.as_f64().is_finite());
        assert!(s.latency.as_f64() >= 0.0);
    }
    let p50 = record.p50().expect("p50 present").as_f64();
    let p95 = record.p95().expect("p95 present").as_f64();
    // Percentiles must be internally consistent for a monotonic distribution.
    assert!(p50 <= p95 + f64::EPSILON);

    // The §3.10 VmDispatch budget (p95 ≤ 2 ms) must hold on the reference VM.
    let verdict = evaluate(&record, &Budgets::v1());
    assert!(
        verdict.passed,
        "VmDispatch p95 {p95:.3}ms exceeded §3.10 ceiling {}ms: {}",
        verdict.ceiling, verdict.reason
    );
}

#[test]
fn harness_record_round_trips_shape_through_json() {
    let driver = HarnessDriver::new(FixtureTree::with_nodes(128), 64);
    let measure: MeasureFn = Box::new(measure_vm_dispatch);
    let record: MetricRecord = driver.run(
        Scenario::AndroidDeclarativeDev,
        MetricKind::VmDispatch,
        &measure,
    );
    // The produced record must round-trip through stable JSON (the same shape the
    // host adapters emit and `examples/ci_ondevice.rs` consumes). Arbitrary `f64`
    // latencies don't survive JSON text bit-exactly (e.g. `0.0104999…` ↔
    // `0.0105`), so assert the *shape* survives: same scenario/kind/tree size,
    // same sample count, and percentile-agreement within float epsilon.
    let json = record.to_json().expect("serialize");
    let back = MetricRecord::from_json(&json).expect("parse");
    assert_eq!(back.scenario, record.scenario);
    assert_eq!(back.kind, record.kind);
    assert_eq!(back.tree_size, record.tree_size);
    assert_eq!(back.samples.len(), record.samples.len());
    let eps = 1e-6;
    assert!((back.p50().unwrap().as_f64() - record.p50().unwrap().as_f64()).abs() < eps);
    assert!((back.p95().unwrap().as_f64() - record.p95().unwrap().as_f64()).abs() < eps);
}
