use super::super::*;
use flux_ir::{ArenaBuilder, Node};
use flux_syntax::{Child, ComponentId, Key, NodeId, NodeKind, Patch, Props, Span};

#[test]
fn respanned_child_reattaches_rather_than_remove_insert() {
    // The same primitive at the same parent/index but with a new node id
    // (an edit shifted its span) must reattach, preserving its instance.
    let tree = |child_id: u32| {
        let mut b = ArenaBuilder::new();
        b.pack(Node {
            id: NodeId::from(1u32),
            kind: NodeKind::Component,
            component_id: ComponentId::from(1u32),
            props: Props::from_fields(vec![]),
            children: vec![Child::Node(NodeId::from(child_id))],
            handlers: vec![],
            span: Span::new(0, 0, 10),
        });
        b.pack(Node {
            id: NodeId::from(child_id),
            kind: NodeKind::Primitive,
            component_id: ComponentId::from(7u32),
            props: Props::from_fields(vec![]),
            children: vec![],
            handlers: vec![],
            span: Span::new(0, 0, 4),
        });
        b.finish()
    };
    let patches = diff(&tree(2), &tree(3));
    let reattach = patches
        .iter()
        .find(|p| matches!(p, Patch::Reattach { .. }))
        .expect("re-spanned child must reattach");
    match reattach {
        Patch::Reattach { old_id, new_id, .. } => {
            assert_eq!(*old_id, NodeId::from(2u32));
            assert_eq!(*new_id, NodeId::from(3u32));
        }
        other => panic!("expected Reattach, got {other:?}"),
    }
    assert!(
        !patches.iter().any(|p| matches!(p, Patch::Remove { .. })),
        "reattached node must not also be removed: {patches:?}"
    );
    assert!(
        !patches.iter().any(|p| matches!(p, Patch::Insert { .. })),
        "reattached node must not also be inserted: {patches:?}"
    );
}

#[test]
fn unrelated_component_still_remove_inserts() {
    // Different component at the same slot with a different id: no shared
    // identity, so state must NOT be transferred.
    let mut b1 = ArenaBuilder::new();
    b1.pack(Node {
        id: NodeId::from(1u32),
        kind: NodeKind::Component,
        component_id: ComponentId::from(1u32),
        props: Props::from_fields(vec![]),
        children: vec![Child::Node(NodeId::from(2u32))],
        handlers: vec![],
        span: Span::new(0, 0, 10),
    });
    b1.pack(Node {
        id: NodeId::from(2u32),
        kind: NodeKind::Primitive,
        component_id: ComponentId::from(7u32),
        props: Props::from_fields(vec![]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 4),
    });
    let a = b1.finish();

    let mut b2 = ArenaBuilder::new();
    b2.pack(Node {
        id: NodeId::from(1u32),
        kind: NodeKind::Component,
        component_id: ComponentId::from(1u32),
        props: Props::from_fields(vec![]),
        children: vec![Child::Node(NodeId::from(3u32))],
        handlers: vec![],
        span: Span::new(0, 0, 10),
    });
    b2.pack(Node {
        id: NodeId::from(3u32),
        kind: NodeKind::Primitive,
        component_id: ComponentId::from(9u32),
        props: Props::from_fields(vec![]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 4),
    });
    let b = b2.finish();

    let patches = diff(&a, &b);
    assert!(
        !patches.iter().any(|p| matches!(p, Patch::Reattach { .. })),
        "different components must not reattach: {patches:?}"
    );
    assert!(patches.iter().any(|p| matches!(p, Patch::Remove { .. })));
    assert!(patches.iter().any(|p| matches!(p, Patch::Insert { .. })));
}

#[test]
fn identical_props_do_not_mask_reorder() {
    // Two children with identical props but reordered: the differ must emit a
    // Reorder, NOT a no-op — so the prop skip must not suppress the child-set
    // check.
    let leaf = |id: u32| Node {
        id: NodeId::from(id),
        kind: NodeKind::Primitive,
        component_id: ComponentId::from(2u32),
        props: Props::from_fields(vec![]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 4),
    };
    let mut b1 = ArenaBuilder::new();
    b1.pack(Node {
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
    });
    b1.pack(leaf(2));
    b1.pack(leaf(3));
    let a = b1.finish();

    let mut b2 = ArenaBuilder::new();
    b2.pack(Node {
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
    });
    b2.pack(leaf(2));
    b2.pack(leaf(3));
    let b = b2.finish();

    let patches = diff(&a, &b);
    assert_eq!(patches.len(), 1, "identical props must not mask reorder");
    assert!(matches!(
        &patches[0],
        Patch::Reorder { parent, keys }
            if *parent == NodeId::from(1u32)
                && *keys == vec![NodeId::from(3u32), NodeId::from(2u32)]
    ));
}

#[test]
fn splice_items_use_precomputed_parent_index() {
    // FLUX-079: a `ForEach` splice's items must resolve their parent/index
    // through the precomputed index (covering `Child::Splice`, not just
    // `Child::Node`). Re-spawning one spliced item under the same splice
    // (same parent/index, same component/kind) must reattach state-
    // preservingly — proving the precomputed map sees through splices and
    // the removed/inserted ids pair up instead of remove+insert.
    let splice_child = |parent_id: u32, child_id: u32| Node {
        id: NodeId::from(parent_id),
        kind: NodeKind::Component,
        component_id: ComponentId::from(1u32),
        props: Props::from_fields(vec![]),
        children: vec![Child::Splice {
            items: vec![(Key::from(child_id as u64), NodeId::from(child_id))],
        }],
        handlers: vec![],
        span: Span::new(0, 0, 10),
    };
    let mut b_old = ArenaBuilder::new();
    b_old.pack(splice_child(1, 2));
    b_old.pack(Node {
        id: NodeId::from(2u32),
        kind: NodeKind::Primitive,
        component_id: ComponentId::from(7u32),
        props: Props::from_fields(vec![]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 4),
    });
    let a = b_old.finish();

    // Same child primitive, now re-spanned (new id 3) under the same splice.
    let mut b_new = ArenaBuilder::new();
    b_new.pack(splice_child(1, 3));
    b_new.pack(Node {
        id: NodeId::from(3u32),
        kind: NodeKind::Primitive,
        component_id: ComponentId::from(7u32),
        props: Props::from_fields(vec![]),
        children: vec![],
        handlers: vec![],
        span: Span::new(0, 0, 4),
    });
    let b = b_new.finish();

    let patches = diff(&a, &b);
    // The spliced child reparents state-preservingly (Reattach, not Remove+Insert),
    // and no spurious Remove/Insert is emitted.
    let reattach = patches
        .iter()
        .find(|p| matches!(p, Patch::Reattach { .. }))
        .expect("re-spanned spliced child must reattach");
    match reattach {
        Patch::Reattach {
            old_id,
            new_id,
            node,
        } => {
            assert_eq!(*old_id, NodeId::from(2u32));
            assert_eq!(*new_id, NodeId::from(3u32));
            assert_eq!(node.component_id, ComponentId::from(7u32));
        }
        other => panic!("expected Reattach, got {other:?}"),
    }
    assert!(
        !patches.iter().any(|p| matches!(p, Patch::Remove { .. })),
        "reattached spliced child must not also be removed: {patches:?}"
    );
    assert!(
        !patches.iter().any(|p| matches!(p, Patch::Insert { .. })),
        "reattached spliced child must not also be inserted: {patches:?}"
    );
}
