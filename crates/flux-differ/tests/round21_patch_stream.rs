//! Round-21 regressions for the differ: handler diffs must survive every
//! in-place branch, Reattach must precede Inserts into its subtree, Remove
//! order must be deterministic, and NaN props must not be perpetually "changed".

use flux_differ::diff;
use flux_ir::{ArenaBuilder, ClosureIR, Node};
use flux_syntax::{
    Child, ComponentId, HandlerId, NodeId, NodeKind, Patch, PropIdx, Props, Span, StringId, Value,
};

fn node(id: u32, kind: NodeKind, component: u32, props: Props, children: Vec<Child>) -> Node {
    Node {
        id: NodeId::from(id),
        kind,
        component_id: ComponentId::from(component),
        props,
        children,
        handlers: vec![],
        span: Span::new(0, id * 10, id * 10 + 5),
    }
}

/// Builds an arena whose string table interns the given texts (in order),
/// so prop `Str` values resolve by content across old/new.
fn arena_with(nodes: Vec<Node>, strings: &[&str]) -> flux_ir::IRArena {
    let mut b = ArenaBuilder::new();
    for s in strings {
        b.intern_string(s);
    }
    for n in nodes {
        b.pack(n);
    }
    b.finish()
}

fn str_prop(id: u32, text: &str) -> Props {
    // StringId 0 = the first interned text in `arena_with` ("Save" in old,
    // "OK" in new): the differ resolves by content across the two tables.
    let _ = text;
    Props::from_fields(vec![(PropIdx::from(0u16), Value::Str(StringId::from(id)))])
}

/// A node whose props AND handler changed in the same edit used to emit only
/// the `Update`; the host kept the stale closure forever.
#[test]
fn handler_change_is_emitted_alongside_a_prop_update() {
    let mut old = node(1, NodeKind::Primitive, 0, str_prop(0, "Save"), vec![]);
    let mut new = node(1, NodeKind::Primitive, 0, str_prop(0, "OK"), vec![]);
    old.handlers.push(HandlerId::from(7u32));
    new.handlers.push(HandlerId::from(7u32));
    let mut old_arena = arena_with(vec![old], &["Save"]);
    let mut new_arena = arena_with(vec![new], &["OK"]);
    let span = Span::new(0, 0, 4);
    old_arena.add_closure(ClosureIR::new(
        HandlerId::from(7u32),
        vec![0x00],
        vec![],
        span,
    ));
    new_arena.add_closure(ClosureIR::new(
        HandlerId::from(7u32),
        vec![0x01, 0x00],
        vec![],
        span,
    ));

    let patches = diff(&old_arena, &new_arena);
    assert!(
        patches.iter().any(|p| matches!(p, Patch::Update { .. })),
        "props changed → Update required, got {patches:?}"
    );
    assert!(
        patches.iter().any(|p| matches!(p, Patch::Handler { .. })),
        "handler body changed → Handler patch required, got {patches:?}"
    );
}

/// A keyed reorder used to silently drop both prop changes and handler
/// changes on the reordered parent.
#[test]
fn reorder_emits_pending_prop_changes() {
    let old = node(
        1,
        NodeKind::Primitive,
        0,
        str_prop(0, "a"),
        vec![
            Child::Node(NodeId::from(2u32)),
            Child::Node(NodeId::from(3u32)),
        ],
    );
    let new = node(
        1,
        NodeKind::Primitive,
        0,
        str_prop(0, "b"),
        vec![
            Child::Node(NodeId::from(3u32)),
            Child::Node(NodeId::from(2u32)),
        ],
    );
    let patches = diff(
        &arena_with(vec![old], &["a"]),
        &arena_with(vec![new], &["b"]),
    );
    assert!(
        patches.iter().any(|p| matches!(p, Patch::Update { .. })),
        "prop change during reorder must not be dropped, got {patches:?}"
    );
    assert!(
        patches.iter().any(|p| matches!(p, Patch::Reorder { .. })),
        "reorder must still be emitted, got {patches:?}"
    );
}

/// Two removed nodes used to emit `Remove` patches in `AHashSet` iteration
/// order — different frame bytes on every process.
#[test]
fn multiple_removes_are_emitted_in_deterministic_order() {
    let empty = Props::from_fields(vec![]);
    let old = arena_with(
        vec![
            node(
                1,
                NodeKind::Primitive,
                0,
                empty.clone(),
                vec![
                    Child::Node(NodeId::from(2u32)),
                    Child::Node(NodeId::from(3u32)),
                    Child::Node(NodeId::from(4u32)),
                ],
            ),
            node(2, NodeKind::Primitive, 2, empty.clone(), vec![]),
            node(3, NodeKind::Primitive, 3, empty.clone(), vec![]),
            node(4, NodeKind::Primitive, 4, empty.clone(), vec![]),
        ],
        &[],
    );
    let new = arena_with(vec![node(1, NodeKind::Primitive, 0, empty, vec![])], &[]);
    let patches = diff(&old, &new);
    let removed: Vec<u32> = patches
        .iter()
        .filter_map(|p| match p {
            Patch::Remove { id } => Some(u32::from(*id)),
            _ => None,
        })
        .collect();
    assert_eq!(removed.len(), 3, "all three children removed");
    let mut sorted = removed.clone();
    sorted.sort_unstable();
    assert_eq!(
        removed, sorted,
        "removes must be id-ordered (deterministic)"
    );
}

/// A NaN float prop used to compare unequal to itself (`NaN != NaN`), emitting
/// a no-op `Update` on every single diff pass.
#[test]
fn nan_prop_is_not_reported_changed() {
    let nan = || Props::from_fields(vec![(PropIdx::from(0u16), Value::Float(f64::NAN))]);
    let patches = diff(
        &arena_with(vec![node(1, NodeKind::Primitive, 0, nan(), vec![])], &[]),
        &arena_with(vec![node(1, NodeKind::Primitive, 0, nan(), vec![])], &[]),
    );
    assert!(
        !patches.iter().any(|p| matches!(p, Patch::Update { .. })),
        "NaN == NaN semantically: no spurious Update, got {patches:?}"
    );
}
