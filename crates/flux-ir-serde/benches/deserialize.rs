//! Criterion benchmarks for the Appendix D deserialization path (FLUX-013 / §3.10
//! "Serialize 50-node patch < 1 ms" companion).
//!
//! The suite only ever measured *serialization*. The host inbound path —
//! decoding an `Init` frame and a `Delta`/`Patch` set off the wire — was
//! unbenched. This file mirrors `serialize.rs`: it first serializes the 50-node
//! frame/patch set (reusing `build_50_node_patch`), then decodes it back and
//! asserts round-trip fidelity, so the timed region is dominated by parsing.

use criterion::{Criterion, criterion_group, criterion_main};
use flux_ir_serde::{Frame, deserialize_patches};
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

fn bench_deserialize_patches(c: &mut Criterion) {
    let (patches, table) = build_50_node_patch();
    let bytes = flux_ir_serde::serialize_patches(&patches, &table, &[]);
    assert!(!bytes.is_empty(), "serialized patch set must be non-empty");
    c.bench_function("deserialize_50_node_patch", |b| {
        b.iter(|| {
            let (back, _) =
                deserialize_patches(std::hint::black_box(&bytes)).expect("patch set must decode");
            assert_eq!(
                back.len(),
                patches.len(),
                "round-trip preserves patch count"
            );
            std::hint::black_box(back)
        });
    });
}

fn bench_deserialize_init_frame(c: &mut Criterion) {
    let (patches, table) = build_50_node_patch();
    let root = match &patches[0] {
        Patch::Replace { node, .. } => node.clone(),
        _ => unreachable!("first patch is a Replace"),
    };
    let frame = Frame::init(
        &root,
        &[],
        &[(0u32, Value::Int(0))],
        &[(0u32, "src/main.flux".to_string())],
        &table,
        &[],
        &[],
        &[],
    );
    let bytes = frame.to_bytes();
    assert!(!bytes.is_empty(), "init frame must serialize");
    c.bench_function("deserialize_init_frame", |b| {
        b.iter(|| {
            let back = Frame::from_init_bytes(std::hint::black_box(&bytes))
                .expect("init frame must decode");
            // The decoded root must carry the same node id as the source.
            assert_eq!(back.root.id, root.id, "round-trip preserves root id");
            std::hint::black_box(back)
        });
    });
}

criterion_group!(
    benches,
    bench_deserialize_patches,
    bench_deserialize_init_frame
);
criterion_main!(benches);
