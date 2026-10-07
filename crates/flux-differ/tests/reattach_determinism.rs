//! Round-19: the `Reattach` patch stream order must be deterministic.
//!
//! `diff()` builds `removed` from `AHashSet::difference()`, whose iteration
//! order is randomized per process. `reattach_pairs` iterates `removed` and
//! pushes pairs in that order, and `diff()` emits `Reattach` patches from
//! `pairs`. Two span-shifted siblings (the canonical reattach case) therefore
//! emit in nondeterministic order.

use flux_differ::diff;
use flux_ir::{ArenaBuilder, Node};
use flux_syntax::{Child, ComponentId, NodeId, NodeKind, Props, Span};

/// A parent with two children whose ids vary between calls (span-shifted
/// edits produce the same shape with new ids).
fn two_children(child_a: u32, child_b: u32) -> flux_ir::IRArena {
    let mut b = ArenaBuilder::new();
    b.pack(Node {
        id: NodeId::from(1u32),
        kind: NodeKind::Component,
        component_id: ComponentId::from(1u32),
        props: Props::from_fields(vec![]),
        children: vec![
            Child::Node(NodeId::from(child_a)),
            Child::Node(NodeId::from(child_b)),
        ],
        handlers: vec![],
        span: Span::new(0, 0, 20),
    });
    b.pack(Node {
        id: NodeId::from(child_a),
        kind: NodeKind::Primitive,
        component_id: ComponentId::from(7u32),
        props: Props::from_fields(vec![]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 4),
    });
    b.pack(Node {
        id: NodeId::from(child_b),
        kind: NodeKind::Primitive,
        component_id: ComponentId::from(8u32),
        props: Props::from_fields(vec![]),
        children: vec![],
        handlers: vec![],
        span: Span::new(5, 0, 9),
    });
    b.finish()
}

#[test]
fn reattach_patch_order_is_deterministic() {
    // Two sibling nodes, both span-shifted between old and new. Both match
    // their counterpart on (component_id, kind, parent, index) → two
    // `Reattach` patches whose emission order follows `removed` (a HashSet
    // difference).
    let old = two_children(2, 3);

    let mut signatures: std::collections::BTreeSet<Vec<(u32, u32)>> = Default::default();
    for _ in 0..32 {
        let new = two_children(4, 5);
        let patches = diff(&old, &new);
        let sig: Vec<(u32, u32)> = patches
            .iter()
            .filter_map(|p| match p {
                flux_syntax::Patch::Reattach { old_id, new_id, .. } => {
                    Some((u32::from(*old_id), u32::from(*new_id)))
                }
                _ => None,
            })
            .collect();
        if sig.is_empty() {
            panic!("expected at least one Reattach; got none: {patches:?}");
        }
        signatures.insert(sig);
    }

    assert_eq!(
        signatures.len(),
        1,
        "Reattach patch stream order is nondeterministic: {} distinct orderings: {:?}",
        signatures.len(),
        signatures,
    );
}
