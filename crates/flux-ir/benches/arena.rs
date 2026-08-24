//! Criterion benchmark harness for `flux-ir` (performance budgets: AGENTS.md §3.6).
//!
//! Stub pre-wired by the foundation pass; replaced by the owning agent.

use criterion::{Criterion, criterion_group, criterion_main};

fn arena_benchmark(criterion: &mut Criterion) {
    criterion.bench_function("arena_placeholder", |bencher| bencher.iter(|| ()));
}

criterion_group!(benches, arena_benchmark);
criterion_main!(benches);
