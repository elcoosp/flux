//! Benchmark: packing 100 nodes must stay under 1 ms (FLUX-004 acceptance).
//!
//! Run with `cargo bench -p flux-ir`. The assertion guards the performance
//! budget from Appendix §3.6 directly so a regression fails CI loudly rather
//! than silently drifting.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use flux_ir::{ArenaBuilder, Node, compute_node_id};
use flux_syntax::{Child, ComponentId, HandlerId, NodeId, PropIdx, Span};
use flux_syntax::{NodeKind, Props, Value};

fn make_node(index: u32) -> Node {
    let span = Span::new(0, index * 4, index * 4 + 3);
    let id = compute_node_id(NodeId::from(0u32), NodeKind::Primitive, span, None);
    Node {
        id,
        kind: NodeKind::Primitive,
        component_id: ComponentId::from(1u32),
        props: Props::from_fields(vec![(PropIdx::from(0u16), Value::Int(i64::from(index)))]),
        children: vec![Child::Node(NodeId::from(index.wrapping_add(1)))],
        handlers: vec![HandlerId::from(index)],
        span,
    }
}

fn bench_pack_100(c: &mut Criterion) {
    let mut group = c.benchmark_group("pack_100");
    group.bench_with_input(BenchmarkId::new("nodes", 100), &100, |b, &n| {
        b.iter(|| {
            let mut builder = ArenaBuilder::new();
            for i in 0..n {
                builder.pack(make_node(i));
            }
            builder.finish()
        });
    });
    group.finish();
}

criterion_group!(benches, bench_pack_100);
criterion_main!(benches);
