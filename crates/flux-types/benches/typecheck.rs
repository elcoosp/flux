//! Criterion benchmark harness for `flux-types` (performance budgets: AGENTS.md §3.6).
//!
//! Stub pre-wired by the foundation pass; replaced by the owning agent.

use criterion::{Criterion, criterion_group, criterion_main};

fn typecheck_benchmark(criterion: &mut Criterion) {
    criterion.bench_function("typecheck_placeholder", |bencher| bencher.iter(|| ()));
}

criterion_group!(benches, typecheck_benchmark);
criterion_main!(benches);
