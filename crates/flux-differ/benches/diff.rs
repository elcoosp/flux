//! Benchmark for `flux_differ::diff` (FLUX-014 acceptance: 50-node diff < 1 ms).

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use flux_differ::diff;
use flux_ir::{ArenaBuilder, Node};
use flux_syntax::{Child, ComponentId, Key, NodeId, NodeKind, PropIdx, Props, Span, Value};
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

/// Builds a wide `n`-node tree whose `n` leaves are a single `Child::Splice`
/// (modelling a `ForEach` list), so a mid-list 500-item splice exercises the
/// reattach-pairing cold path on a realistic dynamic list (FLUX-079).
fn spliced(n: u32) -> flux_ir::IRArena {
    let mut bld = ArenaBuilder::new();
    let root = NodeId::from(1u32);
    let items: Vec<(Key, NodeId)> = (1..=n)
        .map(|i| (Key::from(u64::from(i)), NodeId::from(1 + i)))
        .collect();
    bld.pack(Node {
        id: root,
        kind: NodeKind::Component,
        component_id: ComponentId::from(1u32),
        props: Props::from_fields(vec![]),
        children: vec![Child::Splice { items }],
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

/// Returns a copy of `base` whose splice drops the 500 items `[offset..offset+500)`
/// and appends 500 fresh items at the tail — a large list splice, the workload
/// FLUX-079's precompute targets. The surviving items keep their ids (so the
/// reattach pairing must run), and the new tail items must each resolve their
/// parent/index through the precomputed map.
fn with_spliced_append(
    base: &flux_ir::IRArena,
    n: u32,
    offset: u32,
    count: u32,
) -> flux_ir::IRArena {
    let mut bld = ArenaBuilder::new();
    let root = NodeId::from(1u32);
    let kept: Vec<(Key, NodeId)> = (1..=n)
        .filter(|i| !(*i > offset && *i <= offset + count))
        .map(|i| (Key::from(u64::from(i)), NodeId::from(1 + i)))
        .collect();
    let appended: Vec<(Key, NodeId)> = (0..count)
        .map(|k| {
            let id = NodeId::from(1 + n + k);
            (Key::from(u64::from(1 + n + k)), id)
        })
        .collect();
    let mut items = kept;
    items.extend(appended);
    bld.pack(Node {
        id: root,
        kind: NodeKind::Component,
        component_id: ComponentId::from(1u32),
        props: Props::from_fields(vec![]),
        children: vec![Child::Splice { items }],
        handlers: vec![],
        span: Span::new(0, 0, 4),
    });
    for i in 1..=n {
        if i > offset && i <= offset + count {
            continue;
        }
        let id = NodeId::from(1 + i);
        let v = base.get(id).expect("leaf present");
        bld.pack(Node {
            id,
            kind: v.kind(),
            component_id: v.component_id(),
            props: v.props(),
            children: vec![],
            handlers: vec![],
            span: v.span(),
        });
    }
    for k in 0..count {
        let id = NodeId::from(1 + n + k);
        bld.pack(Node {
            id,
            kind: NodeKind::Primitive,
            component_id: ComponentId::from(2u32),
            props: Props::from_fields(vec![(
                PropIdx::from(0u16),
                Value::Int(i64::from(1u32 + n + k)),
            )]),
            children: vec![],
            handlers: vec![],
            span: Span::new(0, 0, 4),
        });
    }
    bld.finish()
}

fn bench_diff_list_splice(c: &mut Criterion) {
    // FLUX-079 acceptance: diff two 10k-node arenas differing by a 500-item
    // list splice. The §3.10 diff-50-node budget is sub-millisecond; a 10k
    // reattach-heavy diff is allowed to be larger but must stay well under the
    // linear-in-changed-node bound (no O(n·r·i) blow-up). We assert the emitted
    // patch set is non-empty (the splice produced Inserts/Removes) and measure.
    let mut group = c.benchmark_group("diff_list_splice");
    let n = 10_000u32;
    let offset = 4_000u32;
    let count = 500u32;
    let old = spliced(n);
    let new = with_spliced_append(&old, n, offset, count);
    group.bench_with_input(BenchmarkId::new("ten_k_minus_500_splice", n), &n, |b, _| {
        b.iter(|| {
            let patches = diff(black_box(&old), black_box(&new));
            assert!(
                !patches.is_empty(),
                "a list splice must emit at least one patch"
            );
            patches
        })
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_diff,
    bench_diff_identical_subtrees,
    bench_diff_large,
    bench_diff_list_splice
);
criterion_main!(benches);
