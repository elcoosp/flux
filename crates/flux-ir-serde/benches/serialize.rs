//! Criterion benchmarks for the Appendix D serializer (FLUX-013).
//!
//! Acceptance budget: serializing a 50-node patch set must complete in under
//! 1 ms, and a 50-node Init frame must stay under 20 KB.

use criterion::{Criterion, criterion_group, criterion_main};
use flux_ir_serde::{Frame, serialize_patches};
use flux_syntax::{Child, NodeKind, Patch, Props, Span, StringTable, Value};

fn build_50_node_patch() -> (Vec<Patch>, StringTable) {
    let mut table = StringTable::new();
    let mut patches = Vec::with_capacity(50);
    for i in 0..50u32 {
        let label = table.intern(&format!("node-{i}"));
        patches.push(Patch::Replace {
            id: i + 1,
            node: flux_syntax::NodeRef {
                id: i + 1,
                kind: NodeKind::Primitive,
                component_id: (i % 7) + 1,
                props: Props::from_fields(vec![
                    (0u16, Value::Str(label)),
                    (1u16, Value::Int(i as i64)),
                ]),
                children: vec![Child::Node((i + 1) * 100)],
                handlers: vec![],
                span: Span::new(0, i * 4, i * 4 + 4),
            },
        });
    }
    (patches, table)
}

fn bench_serialize_patches(c: &mut Criterion) {
    let (patches, table) = build_50_node_patch();
    c.bench_function("serialize_50_node_patch", |b| {
        b.iter(|| {
            let bytes = serialize_patches(
                std::hint::black_box(&patches),
                std::hint::black_box(&table),
                &[],
            );
            std::hint::black_box(bytes)
        });
    });
}

fn bench_init_frame_size(c: &mut Criterion) {
    let (patches, table) = build_50_node_patch();
    let root = match &patches[0] {
        Patch::Replace { node, .. } => node.clone(),
        _ => unreachable!("first patch is a Replace"),
    };
    c.bench_function("init_50_node_frame_size", |b| {
        b.iter(|| {
            let frame = Frame::init(
                std::hint::black_box(&root),
                std::hint::black_box(&[]),
                std::hint::black_box(&[(0u32, Value::Int(0))]),
                std::hint::black_box(&[(0u32, "src/main.flux".to_string())]),
                std::hint::black_box(&table),
                &[],
                &[],
                &[],
            );
            let bytes = frame.to_bytes();
            assert!(bytes.len() < 20 * 1024, "Init frame over 20 KB");
            std::hint::black_box(bytes)
        });
    });
}

criterion_group!(benches, bench_serialize_patches, bench_init_frame_size);
criterion_main!(benches);
