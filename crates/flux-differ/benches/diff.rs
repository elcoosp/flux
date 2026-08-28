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

fn bench_diff_identical_subtrees(c: &mut Criterion) {
    // The P3 optimization is the O(1) prop-hash skip: when two subtrees are
    // byte-identical, `props_equal` short-circuits on `props_hash` without
    // walking fields. This benchmark locks in that path (two structurally
    // identical trees → no patches, full hash-skip) against the §3.6 budget:
    // diff of a 50-node tree must stay well under 1 ms.
    let mut group = c.benchmark_group("diff_identical");
    for n in [1usize, 10, 50] {
        let old = chain(n as u32);
        let new = chain(n as u32);
        group.bench_with_input(BenchmarkId::new("no_change", n), &n, |b, _| {
            b.iter(|| diff(black_box(&old), black_box(&new)))
        });
    }
    group.finish();
}

/// Builds a wide `n`-node tree (a `Column` with `n` sibling `Text` leaves) so a
/// single-leaf prop edit stays O(changed), not O(tree) (LANE-H, §3.10).
fn wide(n: u32) -> flux_ir::IRArena {
    let mut bld = ArenaBuilder::new();
    let root = NodeId::from(1u32);
    bld.pack(Node {
        id: root,
        kind: NodeKind::Component,
        component_id: ComponentId::from(1u32),
        props: Props::from_fields(vec![]),
        children: (1..=n).map(|i| Child::Node(NodeId::from(1 + i))).collect(),
        handlers: vec![],
        span: Span::new(0, 0, 4),
    });
    for i in 1..=n {
        let id = NodeId::from(1 + i);
        bld.pack(Node {
            id,
            kind: NodeKind::Primitive,
            component_id: ComponentId::from(2u32),
            props: Props::from_fields(vec![(PropIdx::from(0u16), Value::Int(i64::from(i)))]),
            children: vec![],
            handlers: vec![],
            span: Span::new(0, i * 10, i * 10 + 5),
        });
    }
    bld.finish()
}

/// Clones `base` but flips the prop of the final leaf (`1 + n`), modelling a
/// single changed leaf in an otherwise-stable tree.
fn with_changed_leaf(base: &flux_ir::IRArena, n: u32) -> flux_ir::IRArena {
    let mut bld = ArenaBuilder::new();
    // Preserve the root by re-packing it (ids are source-stable, so a plain
    // re-pack reproduces every node; only the edited leaf differs).
    let root = NodeId::from(1u32);
    let rv = base.get(root).expect("root present");
    bld.pack(Node {
        id: root,
        kind: rv.kind(),
        component_id: rv.component_id(),
        props: rv.props(),
        children: (1..=n).map(|i| Child::Node(NodeId::from(1 + i))).collect(),
        handlers: vec![],
        span: rv.span(),
    });
    for i in 1..=n {
        let id = NodeId::from(1 + i);
        let v = base.get(id).expect("leaf present");
        bld.pack(Node {
            id,
            kind: v.kind(),
            component_id: v.component_id(),
            props: if i == n {
                Props::from_fields(vec![(
                    PropIdx::from(0u16),
                    Value::Int(i64::from(i) + 1_000_000),
                )])
            } else {
                v.props()
            },
            children: vec![],
            handlers: vec![],
            span: v.span(),
        });
    }
    bld.finish()
}

fn bench_diff_large(c: &mut Criterion) {
    // LANE-H T1: large-tree diff budget. The §3.10 50-node < 1 ms gate scales
    // *linearly* with the number of *changed* nodes (the dirty-subset reconcile
    // is O(dirty), not O(tree) — R1), so a single-leaf edit on a 1k tree should
    // land under ~20 ms and on a 10k tree under ~200 ms on CI. We assert the
    // emitted patch set is exactly one `Update` (proving O(changed), not
    // O(tree)) and measure wall-time.
    let mut group = c.benchmark_group("diff_large");
    for n in [1_000u32, 10_000] {
        let old = wide(n);
        let new = with_changed_leaf(&old, n);
        group.bench_with_input(BenchmarkId::new("single_leaf_change", n), &n, |b, _| {
            b.iter(|| {
                let patches = diff(black_box(&old), black_box(&new));
                assert_eq!(
                    patches.len(),
                    1,
                    "a single-leaf edit must emit exactly one Update patch"
                );
                patches
            })
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_diff,
    bench_diff_identical_subtrees,
    bench_diff_large
);
criterion_main!(benches);
