//! Benchmark for `flux_differ::diff` (FLUX-014 acceptance: 50-node diff < 1 ms).

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use flux_differ::diff;
use flux_ir::{ArenaBuilder, Node};
use flux_syntax::{Child, ComponentId, NodeId, NodeKind, PropIdx, Props, Span, Value};
use std::hint::black_box;

/// Builds a chain of `n` nodes so there is real structure to reconcile.
fn chain(n: u32) -> flux_ir::IRArena {
    let mut bld = ArenaBuilder::new();
    let mut prev = NodeId::from(1u32);
    for i in 1..=n {
        let id = NodeId::from(i);
        let is_leaf = i == n;
        let child = if is_leaf {
            vec![]
        } else {
            vec![Child::Node(NodeId::from(i + 1))]
        };
        bld.pack(Node {
            id,
            kind: if i == 1 {
                NodeKind::Component
            } else {
                NodeKind::Primitive
            },
            component_id: ComponentId::from(1u32),
            props: Props::from_fields(vec![(PropIdx::from(0u16), Value::Int(i64::from(i)))]),
            children: child,
            handlers: vec![],
            span: Span::new(0, i * 10, i * 10 + 5),
        });
        prev = id;
    }
    let _ = prev;
    bld.finish()
}

fn bench_diff(c: &mut Criterion) {
    let mut group = c.benchmark_group("diff");
    for n in [1usize, 10, 50] {
        let old = chain(n as u32);
        // Mutate every node's prop to force a full Update pass.
        let mut bld = ArenaBuilder::new();
        for i in 1..=n as u32 {
            bld.pack(flux_ir::Node {
                id: NodeId::from(i),
                kind: if i == 1 {
                    NodeKind::Component
                } else {
                    NodeKind::Primitive
                },
                component_id: ComponentId::from(1u32),
                props: Props::from_fields(vec![(
                    PropIdx::from(0u16),
                    Value::Int(i64::from(i) + 1000),
                )]),
                children: if i == n as u32 {
                    vec![]
                } else {
                    vec![Child::Node(NodeId::from(i + 1))]
                },
                handlers: vec![],
                span: Span::new(0, i * 10, i * 10 + 5),
            });
        }
        let new = bld.finish();
        group.bench_with_input(BenchmarkId::new("prop_mutation", n), &n, |b, _| {
            b.iter(|| diff(black_box(&old), black_box(&new)))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_diff);
criterion_main!(benches);
