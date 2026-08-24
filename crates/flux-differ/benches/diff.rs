//! Criterion benchmark harness for `flux-differ` (performance budgets: AGENTS.md §3.6).
//!
//! Stub pre-wired by the foundation pass; replaced by the owning agent.

use criterion::{Criterion, criterion_group, criterion_main};

fn diff_benchmark(criterion: &mut Criterion) {
    criterion.bench_function("diff_placeholder", |bencher| bencher.iter(|| ()));
}

criterion_group!(benches, diff_benchmark);
criterion_main!(benches);
