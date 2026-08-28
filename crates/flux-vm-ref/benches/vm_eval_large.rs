//! Criterion benchmark: large-signal-graph handler evaluation (LANE-H, T1).
//!
//! The §3.10 budget caps a 50-instruction VM handler at < 2 ms of eval time.
//! Production apps keep a graph of thousands of signals, so this bench drives a
//! 50-instruction handler (alternating `READ_SIGNAL` / `WRITE_SIGNAL` over a
//! 10k-signal `InMemorySignals` store) and asserts the per-eval wall time stays
//! inside the budget while the graph is two orders of magnitude larger than the
//! micro-benchmarks.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use flux_syntax::Value;
use flux_vm_ref::{InMemorySignals, run};
use std::hint::black_box;
use std::time::Instant;

/// Asserts `work` completes in under `budget_us` on average over a warm-up plus
/// fixed measurement window, so a regression against the §3.10 budget fails CI
/// loudly instead of silently drifting.
const WARMUP: u32 = 10;
const MEASURE: u32 = 200;

fn assert_within_budget_us<F, T>(name: &str, budget_us: u128, work: F)
where
    F: Fn() -> T,
{
    for _ in 0..WARMUP {
        black_box(work());
    }
    let started = Instant::now();
    for _ in 0..MEASURE {
        black_box(work());
    }
    let avg_us = started.elapsed().as_micros() / u128::from(MEASURE);
    assert!(
        avg_us < budget_us,
        "{name} averaged {avg_us} µs, over the §3.10 budget of {budget_us} µs"
    );
}

/// Number of signals in the graph under test (production-scale).
const SIGNAL_GRAPH_SIZE: u32 = 10_000;

/// Builds a 50-instruction handler that alternates `READ_SIGNAL`/`WRITE_SIGNAL`
/// across the signal graph (opcode layout from Appendix E §E.1):
/// `READ_SIGNAL dst, id` (6 bytes) and `WRITE_SIGNAL id, src` (6 bytes),
/// `HALT` (1 byte). Reading from and writing to distinct ids keeps the handler
/// from collapsing into a no-op while staying well under the gas budget.
fn make_program() -> Vec<u8> {
    let mut prog: Vec<u8> = Vec::with_capacity(50 * 6 + 1);
    for i in 0..50u32 {
        // Register file is 16 wide (r0..=r15); map the loop counter into it.
        let reg = (i % 15) as u8;
        if i % 2 == 0 {
            // READ_SIGNAL r{reg}, signal{(i * 7) % N}
            prog.push(0x10); // READ_SIGNAL
            prog.push(reg); // dst reg
            let id = (i.wrapping_mul(7)) % SIGNAL_GRAPH_SIZE;
            prog.extend_from_slice(&id.to_le_bytes());
        } else {
            // WRITE_SIGNAL signal{(i * 3) % N}, r{reg}
            prog.push(0x11); // WRITE_SIGNAL
            let id = (i.wrapping_mul(3)) % SIGNAL_GRAPH_SIZE;
            prog.extend_from_slice(&id.to_le_bytes());
            prog.push(reg); // src reg
        }
    }
    prog.push(0x00); // HALT
    prog
}

fn bench_vm_eval_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("vm_eval_large");
    let program = make_program();
    let signals = InMemorySignals::from_signals(
        (0..SIGNAL_GRAPH_SIZE).map(|id| (id, Value::Int(i64::from(id)))),
    );
    // §3.10 budget: a 50-instruction handler must evaluate in under 2 ms even
    // against a 10k-signal graph.
    assert_within_budget_us("vm_eval_50instr", 2_000, || {
        let mut s = signals.clone();
        run(black_box(&program), &mut s, Value::Int(0)).expect("handler runs")
    });
    group.bench_with_input(
        BenchmarkId::new("50instr", SIGNAL_GRAPH_SIZE),
        &SIGNAL_GRAPH_SIZE,
        |b, _| {
            b.iter(|| {
                let mut s = signals.clone();
                let out = run(black_box(&program), &mut s, Value::Int(0)).expect("handler runs");
                // The store must carry at least the written signals so the
                // bench is not optimised away and the writes are observable.
                assert!(!out.signals.is_empty(), "handler must emit signal writes");
                out
            })
        },
    );
    group.finish();
}

criterion_group!(benches, bench_vm_eval_large);
criterion_main!(benches);
