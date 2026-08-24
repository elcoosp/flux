//! Criterion benchmark harness for `flux-parser` (performance budgets: AGENTS.md §3.6).
//!
//! Stub pre-wired by the foundation pass; replaced by the owning agent.

use criterion::{Criterion, criterion_group, criterion_main};

fn parse_benchmark(criterion: &mut Criterion) {
    criterion.bench_function("parse_placeholder", |bencher| bencher.iter(|| ()));
}

criterion_group!(benches, parse_benchmark);
criterion_main!(benches);
