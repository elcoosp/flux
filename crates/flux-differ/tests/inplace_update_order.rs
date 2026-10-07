//! Round-20: the patch stream for in-place changes (Replace/Reattach/Update/
//! Handler) must be deterministic. That loop iterates
//! `old_ids.intersection(&new_ids)` — an `AHashSet` — whose iteration order is
//! randomized per process. Two updates on different nodes therefore emit in
//! whichever order the intersection produced.

use flux_differ::diff;
use flux_ir::{ArenaBuilder, Node};
use flux_syntax::{ComponentId, NodeId, NodeKind, Props, PropIdx, Span, Value};

fn tree(v0: i64, v1: i64) -> flux_ir::IRArena {
    let mut b = ArenaBuilder::new();
    b.pack(Node {
        id: NodeId::from(1u32),
        kind: NodeKind::Component,
        component_id: ComponentId::from(1u32),
        props: Props::from_fields(vec![]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 20),
    });
    b.pack(Node {
        id: NodeId::from(10u32),
        kind: NodeKind::Primitive,
        component_id: ComponentId::from(7u32),
        props: Props::from_fields(vec![(PropIdx::from(0u16), Value::Int(v0))]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 4),
    });
    b.pack(Node {
        id: NodeId::from(11u32),
        kind: NodeKind::Primitive,
        component_id: ComponentId::from(7u32),
        props: Props::from_fields(vec![(PropIdx::from(0u16), Value::Int(v1))]),
        children: vec![],
        handlers: vec![],
        span: Span::new(5, 0, 9),
    });
    b.finish()
}

#[test]
fn multi_node_update_order_is_deterministic() {
    // Both node ids (10, 11) exist in old and new with the same kind /
    // component, but each has a prop change → two `Patch::Update`s from the
    // intersection loop. Order is whatever the AHashSet produced.
    let old = tree(0, 0);

    let mut signatures: std::collections::BTreeSet<Vec<u32>> = Default::default();
    for _ in 0..32 {
        let new = tree(1, 2);
        let patches = diff(&old, &new);
        let sig: Vec<u32> = patches
            .iter()
            .filter_map(|p| match p {
                flux_syntax::Patch::Update { id, .. } => Some(u32::from(*id)),
                _ => None,
            })
            .collect();
        signatures.insert(sig);
    }

    assert_eq!(
        signatures.len(),
        1,
        "in-place patch order nondeterministic: {} distinct: {:?}",
        signatures.len(),
        signatures,
    );
}
