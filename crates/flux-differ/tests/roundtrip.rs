//! Round-trip and canonical-diff tests for `flux-differ` (FLUX-014).
//!
//! Acceptance criteria from the contract:
//! 1. Identical trees → empty patch.
//! 2. The four canonical minimal-diff cases (Replace / Update / Insert / Remove)
//!    plus Reorder emit exactly one minimal patch.
//! 3. Proptest: `diff` then a test-only `apply` reconstructs the new tree.
//!
//! `apply` is a test-only reconstruction helper (the production path is the
//! Swift/Kotlin runtime); it works over a `NodeId → NodeRef` map so it does not
//! need to mutate the append-only `IRArena`.

use ahash::AHashMap;
use flux_differ::diff;
use flux_ir::ArenaBuilder;
use flux_ir::Node;
use flux_syntax::{
    Child, ComponentId, NodeId, NodeKind, NodeRef, Patch, PropIdx, Props, Span, Value,
};
use proptest::prelude::*;

/// Build a tree with a root (id 1) and one child (id 2).
fn two_node_tree(child_kind: NodeKind, child_prop: i64) -> flux_ir::IRArena {
    let root = Node {
        id: NodeId::from(1u32),
        kind: NodeKind::Component,
        component_id: ComponentId::from(1u32),
        props: Props::from_fields(vec![]),
        children: vec![Child::Node(NodeId::from(2u32))],
        handlers: vec![],
        span: Span::new(0, 0, 10),
    };
    let child = Node {
        id: NodeId::from(2u32),
        kind: child_kind,
        component_id: ComponentId::from(2u32),
        props: Props::from_fields(vec![(PropIdx::from(0u16), Value::Int(child_prop))]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 10, 20),
    };
    let mut b = ArenaBuilder::new();
    b.pack(root);
    b.pack(child);
    b.finish()
}

/// Flatten an arena into an id→NodeRef map for comparison / reconstruction.
fn to_map(arena: &flux_ir::IRArena) -> AHashMap<NodeId, NodeRef> {
    let mut out = AHashMap::new();
    for id in arena.all_ids() {
        let v = arena.get(id).expect("present");
        out.insert(
            id,
            NodeRef {
                id,
                kind: v.kind(),
                component_id: v.component_id(),
                props: v.props(),
                children: v.children(),
                handlers: v.handlers(),
                span: v.span(),
            },
        );
    }
    out
}

/// Render a map deterministically for equality comparison in tests
/// (`NodeRef` does not derive `PartialEq` in `flux-syntax`).
fn debug_map(map: &AHashMap<NodeId, NodeRef>) -> String {
    let mut ids: Vec<NodeId> = map.keys().copied().collect();
    ids.sort_by_key(|id| *id);
    let mut out = String::new();
    for id in ids {
        out.push_str(&format!("{id:?}={:?}|", map[&id]));
    }
    out
}

/// Test-only applier: reconstruct the post-patch tree as an id→NodeRef map.
///
/// Insert patches are applied last and in ascending `(parent, index)` order so
/// that out-of-order patch delivery (the diff iterates `HashSet`s) still yields
/// the correct child ordering.
fn apply(old: &AHashMap<NodeId, NodeRef>, patches: &[Patch]) -> AHashMap<NodeId, NodeRef> {
    let mut map = old.clone();
    let mut inserts: Vec<&Patch> = Vec::new();
    for patch in patches {
        match patch {
            Patch::Insert { .. } => inserts.push(patch),
            Patch::Replace { id, node } => {
                map.insert(*id, node.clone());
            }
            Patch::Update { id, props_diff } => {
                if let Some(n) = map.get_mut(id) {
                    let mut fields: Vec<(PropIdx, Value)> = n
                        .props
                        .fields()
                        .iter()
                        .map(|(k, v)| (*k, v.clone()))
                        .collect();
                    for (k, v) in &props_diff.changes {
                        if let Some(slot) = fields.iter_mut().find(|(fk, _)| fk == k) {
                            slot.1 = v.clone();
                        } else {
                            fields.push((*k, v.clone()));
                        }
                    }
                    fields.retain(|(k, _)| !props_diff.removals.contains(k));
                    n.props = Props::from_fields(fields);
                }
            }
            Patch::Remove { id } => {
                for n in map.values_mut() {
                    n.children.retain(|c| c.node_ids().all(|cid| cid != *id));
                }
                map.remove(id);
            }
            Patch::Reorder { parent, keys } => {
                if let Some(p) = map.get_mut(parent) {
                    let key_pos = keys
                        .iter()
                        .enumerate()
                        .map(|(i, k)| (*k, i))
                        .collect::<AHashMap<_, _>>();
                    p.children.sort_by_key(|c| match c {
                        Child::Node(id) => key_pos.get(id).copied().unwrap_or(usize::MAX),
                        Child::Splice { .. } => usize::MAX,
                        _ => usize::MAX,
                    });
                }
            }
            // Handler-body swaps do not change the node map in this test applier;
            // the production runtime applies them to the VM, not the IR map.
            Patch::Handler { .. } => {}
            // Future patch kinds are non-exhaustive; ignore in this test applier.
            _ => {}
        }
    }
    inserts.sort_by_key(|p| match p {
        Patch::Insert { parent, index, .. } => (*parent, *index),
        _ => unreachable!(),
    });
    for patch in inserts {
        if let Patch::Insert {
            parent,
            index,
            node,
        } = patch
        {
            if let Some(p) = map.get_mut(parent) {
                let mut slot = p.children.len();
                let mut seen = 0usize;
                let mut placed = false;
                for (i, child) in p.children.iter().enumerate() {
                    if seen == *index as usize && !placed {
                        slot = i;
                        placed = true;
                        break;
                    }
                    seen += child.node_ids().count();
                    slot = i + 1;
                }
                let new_child = Child::Node(node.id);
                if placed {
                    p.children.insert(slot, new_child);
                } else {
                    p.children.push(new_child);
                }
            }
            map.insert(node.id, node.clone());
        }
    }
    map
}

#[test]
fn identical_trees_produce_no_patches() {
    let a = two_node_tree(NodeKind::Primitive, 12);
    let b = two_node_tree(NodeKind::Primitive, 12);
    assert!(diff(&a, &b).is_empty());
}

#[test]
fn kind_change_emits_replace() {
    let a = two_node_tree(NodeKind::Primitive, 12);
    let b = two_node_tree(NodeKind::Component, 12);
    let patches = diff(&a, &b);
    assert_eq!(patches.len(), 1);
    assert!(matches!(
        &patches[0],
        Patch::Replace { id, .. } if *id == NodeId::from(2u32)
    ));
}

#[test]
fn prop_change_emits_update() {
    let a = two_node_tree(NodeKind::Primitive, 12);
    let b = two_node_tree(NodeKind::Primitive, 99);
    let patches = diff(&a, &b);
    assert_eq!(patches.len(), 1);
    match &patches[0] {
        Patch::Update { id, props_diff } => {
            assert_eq!(*id, NodeId::from(2u32));
            assert_eq!(
                props_diff.changes,
                vec![(PropIdx::from(0u16), Value::Int(99))]
            );
            assert!(props_diff.removals.is_empty());
        }
        other => panic!("expected Update, got {other:?}"),
    }
}

#[test]
fn child_insert_emits_insert() {
    let a = two_node_tree(NodeKind::Primitive, 12);
    // b = root with children [2, 3] and a third node 3.
    let root = Node {
        id: NodeId::from(1u32),
        kind: NodeKind::Component,
        component_id: ComponentId::from(1u32),
        props: Props::from_fields(vec![]),
        children: vec![
            Child::Node(NodeId::from(2u32)),
            Child::Node(NodeId::from(3u32)),
        ],
        handlers: vec![],
        span: Span::new(0, 0, 10),
    };
    let third = Node {
        id: NodeId::from(3u32),
        kind: NodeKind::Primitive,
        component_id: ComponentId::from(2u32),
        props: Props::from_fields(vec![]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 20, 30),
    };
    let mut bld = ArenaBuilder::new();
    bld.pack(root);
    bld.pack(two_node_child());
    bld.pack(third);
    let b = bld.finish();
    let patches = diff(&a, &b);
    assert_eq!(patches.len(), 1);
    assert!(matches!(
        &patches[0],
        Patch::Insert { parent, node, .. } if node.id == NodeId::from(3u32) && *parent == NodeId::from(1u32)
    ));
}

fn two_node_child() -> Node {
    Node {
        id: NodeId::from(2u32),
        kind: NodeKind::Primitive,
        component_id: ComponentId::from(2u32),
        props: Props::from_fields(vec![(PropIdx::from(0u16), Value::Int(12))]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 10, 20),
    }
}

#[test]
fn child_remove_emits_remove() {
    let a = two_node_tree(NodeKind::Primitive, 12);
    // b = root with no children.
    let root = Node {
        id: NodeId::from(1u32),
        kind: NodeKind::Component,
        component_id: ComponentId::from(1u32),
        props: Props::from_fields(vec![]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 10),
    };
    let mut bld = ArenaBuilder::new();
    bld.pack(root);
    let b = bld.finish();
    let patches = diff(&a, &b);
    assert_eq!(patches.len(), 1);
    assert!(matches!(
        &patches[0],
        Patch::Remove { id } if *id == NodeId::from(2u32)
    ));
}

#[test]
fn child_reorder_emits_reorder_not_remove_insert() {
    let leaf = |id: u32| Node {
        id: NodeId::from(id),
        kind: NodeKind::Primitive,
        component_id: ComponentId::from(2u32),
        props: Props::from_fields(vec![]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 5),
    };
    let root_a = Node {
        id: NodeId::from(1u32),
        kind: NodeKind::Component,
        component_id: ComponentId::from(1u32),
        props: Props::from_fields(vec![]),
        children: vec![
            Child::Node(NodeId::from(2u32)),
            Child::Node(NodeId::from(3u32)),
        ],
        handlers: vec![],
        span: Span::new(0, 0, 10),
    };
    let mut b1 = ArenaBuilder::new();
    b1.pack(root_a);
    b1.pack(leaf(2));
    b1.pack(leaf(3));
    let a = b1.finish();

    let root_b = Node {
        id: NodeId::from(1u32),
        kind: NodeKind::Component,
        component_id: ComponentId::from(1u32),
        props: Props::from_fields(vec![]),
        children: vec![
            Child::Node(NodeId::from(3u32)),
            Child::Node(NodeId::from(2u32)),
        ],
        handlers: vec![],
        span: Span::new(0, 0, 10),
    };
    let mut b2 = ArenaBuilder::new();
    b2.pack(root_b);
    b2.pack(leaf(2));
    b2.pack(leaf(3));
    let b = b2.finish();

    let patches = diff(&a, &b);
    assert_eq!(
        patches.len(),
        1,
        "expected a single Reorder, got {patches:?}"
    );
    assert!(matches!(
        &patches[0],
        Patch::Reorder { parent, keys } if *parent == NodeId::from(1u32)
            && *keys == vec![NodeId::from(3u32), NodeId::from(2u32)]
    ));
}

#[test]
fn diff_then_apply_reconstructs_new_tree() {
    let a = two_node_tree(NodeKind::Primitive, 12);
    let b = two_node_tree(NodeKind::Primitive, 99);
    let patches = diff(&a, &b);
    let rebuilt = apply(&to_map(&a), &patches);
    assert_eq!(debug_map(&rebuilt), debug_map(&to_map(&b)));
}

/// Builds a star tree: root (id 1) with `len` child leaves (ids 2..=len+1),
/// each leaf given a kind picked by `kinds[i]` and a prop picked by `props[i]`.
fn star_tree(len: u32, kinds: &[u8], props: &[i64]) -> flux_ir::IRArena {
    let mut children = Vec::new();
    for i in 0..len {
        children.push(Child::Node(NodeId::from(2u32 + i)));
    }
    let root = Node {
        id: NodeId::from(1u32),
        kind: NodeKind::Component,
        component_id: ComponentId::from(1u32),
        props: Props::from_fields(vec![]),
        children,
        handlers: vec![],
        span: Span::new(0, 0, 10),
    };
    let mut bld = ArenaBuilder::new();
    bld.pack(root);
    for i in 0..len {
        let kind = NodeKind::ALL[(kinds[i as usize] as usize) % NodeKind::ALL.len()];
        let leaf = Node {
            id: NodeId::from(2u32 + i),
            kind,
            component_id: ComponentId::from(2u32),
            props: Props::from_fields(vec![(PropIdx::from(0u16), Value::Int(props[i as usize]))]),
            children: vec![],
            handlers: vec![],
            span: Span::new(0, 10 * (i + 1), 10 * (i + 2)),
        };
        bld.pack(leaf);
    }
    bld.finish()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Diffing a tree and applying the result reconstructs the target tree,
    /// over randomized star trees of varying size and content.
    #[test]
    fn proptest_diff_apply_round_trips(
        len_a in 0u32..5,
        len_b in 0u32..5,
        kinds_a in proptest::collection::vec(0u8..7, 5),
        kinds_b in proptest::collection::vec(0u8..7, 5),
        props_a in proptest::collection::vec(0i64..100, 5),
        props_b in proptest::collection::vec(0i64..100, 5),
    ) {
        let a = star_tree(len_a, &kinds_a, &props_a);
        let b = star_tree(len_b, &kinds_b, &props_b);
        let patches = diff(&a, &b);
        let rebuilt = apply(&to_map(&a), &patches);
        prop_assert_eq!(debug_map(&rebuilt), debug_map(&to_map(&b)));
    }
}
