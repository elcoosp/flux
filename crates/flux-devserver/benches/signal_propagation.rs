//! Criterion benchmark: signal-propagation / minimal-patch emission (§3.10
//! "Signal propagation (10 dirty cells) < 1 ms").
//!
//! This is the server-side half of the reactive budget: when a handler writes a
//! signal, the [`DependencyIndex`] (reverse `SignalId → {NodeId}` map) computes
//! `dirty = dependents[written]` and [`emit_minimal_updates`] re-materialises
//! exactly those nodes. The timed region is the scope computation plus patch
//! assembly for a graph with many dependents, mirroring the ADR-0027 dispatch
//! algorithm the host triggers on every interaction.

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

use flux_devserver::{DependencyIndex, NodeSignalDeps, emit_minimal_updates};
use flux_ir::ArenaBuilder;
use flux_syntax::{ComponentId, NodeId, NodeKind, PropIdx, Props, SignalId, Span, Value};

/// Builds an arena of `n` primitive nodes, each reading a distinct signal so the
/// index has `n` edges to walk.
fn build_arena(n: u32) -> flux_ir::IRArena {
    let mut bld = ArenaBuilder::new();
    for i in 0..n {
        bld.pack(flux_ir::Node {
            id: NodeId::from(i + 1),
            kind: NodeKind::Primitive,
            component_id: ComponentId::from(1u32),
            props: Props::from_fields(vec![(PropIdx::from(0u16), Value::Int(i as i64 + 1))]),
            children: vec![],
            handlers: vec![],
            span: Span::new(0, i * 4, i * 4 + 3),
        });
    }
    bld.finish()
}

/// One edge per node: node `i+1` reads signal `i`.
fn build_deps(n: u32) -> Vec<NodeSignalDeps> {
    (0..n)
        .map(|i| NodeSignalDeps {
            id: NodeId::from(i + 1),
            signal_deps: vec![SignalId::from(i)],
        })
        .collect()
}

fn bench_signal_propagation(c: &mut Criterion) {
    let mut group = c.benchmark_group("signal_propagation");
    for n in [10u32, 100, 1_000] {
        let arena = build_arena(n);
        let mut index = DependencyIndex::default();
        index.rebuild(&build_deps(n));
        assert!(index.is_active(), "index must be active with deps");
        group.bench_with_input(format!("emit/{n}_dirty"), &n, |b, _| {
            b.iter(|| {
                // Write every signal at once → every node is dirty → the bench
                // measures worst-case fan-out (all nodes re-materialised).
                let mut total = 0usize;
                for i in 0..n {
                    let patches = emit_minimal_updates(
                        SignalId::from(i),
                        black_box(&arena),
                        black_box(&index),
                    )
                    .expect("active index emits");
                    total += patches.len();
                }
                assert_eq!(total as u32, n, "every node must be re-materialised once");
                total
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_signal_propagation);
criterion_main!(benches);
