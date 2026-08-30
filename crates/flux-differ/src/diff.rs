//! Keyed tree differencing implementation (FLUX-014).
//!
//! See [`crate`] for the algorithm overview. The public entry point is
//! [`diff`]; the remaining items are small, test-supported helpers.

use std::hash::{Hash, Hasher};

use ahash::AHashSet;
use flux_ir::{IRArena, NodeView};
use flux_syntax::{
    Child, ClosureRef, HandlerId, NodeId, NodeRef, Patch, PropDiff, PropIdx, SignalId, Span,
    StringTable, Value,
};

/// Computes the minimal [`Patch`] stream transforming `old` into `new`.
///
/// Reconciliation is keyed on the stable [`NodeId`]s derived by
/// `flux_ir::compute_node_id`, so edits that preserve structure emit no
/// spurious remove+insert pairs.
#[must_use]
pub fn diff(old: &IRArena, new: &IRArena) -> Vec<Patch> {
    let mut patches = Vec::new();
    let old_ids: AHashSet<NodeId> = old.all_ids().collect();
    let new_ids: AHashSet<NodeId> = new.all_ids().collect();

    // Nodes present in both: compare for in-place changes.
    for id in old_ids.intersection(&new_ids) {
        let o = old.get(*id).expect("present in old");
        let n = new.get(*id).expect("present in new");
        if o.kind() != n.kind() {
            emit_replace(&mut patches, &n);
            continue;
        }
        if o.component_id() != n.component_id() {
            // Same node identity and same node kind, different component: the
            // author swapped the primitive at this position (`Column` → `Row`)
            // or re-specialised a generic. The live instance still belongs at
            // this slot, so re-key it in place instead of destroying it — a
            // `Replace` here is what used to reset input focus and scroll
            // position on a trivial refactor (roadmap Phase 3).
            patches.push(Patch::Reattach {
                old_id: *id,
                new_id: *id,
                node: to_ref(&n),
            });
            continue;
        }
        // Task 1 (FLUX-014 P3): node-level prop skip. `props_equal` short-circuits
        // on the arena-stored `u64` hash, so identical-hash nodes emit no
        // `Update` and we avoid deserialising either cold blob.
        let props_equal = props_equal(&o, &n, old, new);

        // LANE-H T2: structural fast-path. The arena precomputes `children_hash`
        // as an order-sensitive blake3 fold of the *full* child layout (slot,
        // key, id — see `IRArena::children_hash`). For a large tree where almost
        // every node is unchanged, reaching for `child_ids`/`child_order` would
        // allocate an `AHashSet` *and* a `Vec` for each of the 10k nodes on every
        // pass. When the props hash and the children hash both match the node is
        // provably structurally and prop-wise identical, so we skip straight to
        // the (cheap, rare) handler check and allocate nothing. This is
        // behaviour-preserving: equal layout ⇒ equal child set and order.
        if props_equal && o.children_hash() == n.children_hash() {
            if handlers_equal(old, new, &o.handlers(), &n.handlers()) {
                continue; // truly identical — no patch, no allocation
            }
            // Only handler bodies changed → state-preserving fast path.
            emit_handler(&mut patches, new, n.handlers(), o.handlers());
            continue;
        }

        // Fall-back path for nodes whose props and/or children changed: compare
        // the actual child sets/orders (the hot-path allocations above are
        // avoided for the dominant unchanged case).
        let o_children = child_ids(&o);
        let n_children = child_ids(&n);
        if o_children == n_children {
            // Same child set: only order may differ.
            if child_order(&o) == child_order(&n) {
                // Props changed; structure (children) unchanged.
                patches.push(Patch::Update {
                    id: *id,
                    props_diff: props_diff(&o, &n, old, new),
                });
                continue;
            }
            // Same set, different order → single Reorder, not remove+insert.
            patches.push(Patch::Reorder {
                parent: *id,
                keys: child_order(&n),
            });
            continue;
        }
        // Child set differs (an add/remove handled by the loops below) but the
        // parent node itself may still carry prop/handler changes.
        if props_equal {
            if handlers_equal(old, new, &o.handlers(), &n.handlers()) {
                continue;
            }
            emit_handler(&mut patches, new, n.handlers(), o.handlers());
            continue;
        }
        patches.push(Patch::Update {
            id: *id,
            props_diff: props_diff(&o, &n, old, new),
        });
    }

    // Nodes removed from the new tree.
    // Structural edits (a re-spanned or retagged subtree) surface as a
    // removed id plus an inserted id that still denote the SAME component at
    // the SAME parent/index. Pair those up into a state-preserving
    // `Patch::Reattach` before falling back to remove+insert (roadmap Phase 3).
    let removed: Vec<NodeId> = old_ids.difference(&new_ids).copied().collect();
    let inserted: Vec<NodeId> = new_ids.difference(&old_ids).copied().collect();
    let pairs = reattach_pairs(old, new, &removed, &inserted);

    for id in &removed {
        if pairs.iter().any(|(old_id, _)| old_id == id) {
            continue;
        }
        patches.push(Patch::Remove { id: *id });
    }

    for id in &inserted {
        if pairs.iter().any(|(_, new_id)| new_id == id) {
            continue;
        }
        if let Some((parent, index)) = find_parent_and_index(new, *id) {
            let n = new.get(*id).expect("present in new");
            patches.push(Patch::Insert {
                parent,
                index,
                node: to_ref(&n),
            });
        }
    }

    for (old_id, new_id) in pairs {
        let n = new.get(new_id).expect("present in new");
        patches.push(Patch::Reattach {
            old_id,
            new_id,
            node: to_ref(&n),
        });
    }

    patches
}

/// Pairs each removed node with an inserted node that denotes the same live
/// instance, so the host can re-key rather than re-materialise it.
///
/// Two nodes pair up only when they agree on **component identity** (same
/// `component_id`, same `kind`) and on **position** (same parent slot and index
/// in their respective trees). Both conditions are required: matching on
/// component alone would re-key an unrelated sibling and silently move state to
/// the wrong node. Each id pairs at most once.
fn reattach_pairs(
    old: &IRArena,
    new: &IRArena,
    removed: &[NodeId],
    inserted: &[NodeId],
) -> Vec<(NodeId, NodeId)> {
    let mut pairs: Vec<(NodeId, NodeId)> = Vec::new();
    let mut taken: AHashSet<NodeId> = AHashSet::new();
    for old_id in removed {
        let Some(o) = old.get(*old_id) else { continue };
        let old_slot = find_parent_and_index(old, *old_id);
        for new_id in inserted {
            if taken.contains(new_id) {
                continue;
            }
            let Some(n) = new.get(*new_id) else { continue };
            if o.component_id() != n.component_id() || o.kind() != n.kind() {
                continue;
            }
            if old_slot != find_parent_and_index(new, *new_id) {
                continue;
            }
            taken.insert(*new_id);
            pairs.push((*old_id, *new_id));
            break;
        }
    }
    pairs
}

/// Emits a `Replace` patch carrying the full new node.
fn emit_replace(patches: &mut Vec<Patch>, n: &NodeView<'_>) {
    patches.push(Patch::Replace {
        id: n.id(),
        node: to_ref(n),
    });
}

/// Emits `Handler` patches for the differing handler bodies (state-preserving).
fn emit_handler(
    patches: &mut Vec<Patch>,
    new: &IRArena,
    new_handlers: Vec<HandlerId>,
    old_handlers: Vec<HandlerId>,
) {
    for hid in new_handlers
        .iter()
        .chain(old_handlers.iter())
        .collect::<AHashSet<_>>()
    {
        if !new_handlers.contains(hid) || !old_handlers.contains(hid) {
            continue;
        }
        if let Some(cl) = new.closure(*hid) {
            patches.push(Patch::Handler {
                id: *hid,
                closure: closure_ref(&cl.bytecode, cl.captured_signals.clone(), cl.span),
            });
        }
    }
}

/// Builds a `ClosureRef` from a closure's bytecode. The digest is a content
/// hash; the canonical BLAKE3 form is produced by the serialization crate
/// (FLUX-013). Here a stable `u64` hash suffices for diff identity.
fn closure_ref(bytecode: &[u8], captured: Vec<SignalId>, span: Span) -> ClosureRef {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytecode.hash(&mut hasher);
    ClosureRef {
        hash: hasher.finish(),
        bytecode_offset: 0,
        bytecode_len: bytecode.len() as u16,
        captured_signals: captured,
        span,
        excerpt: None,
    }
}

/// Converts a `NodeView` into a standalone `NodeRef` for embedding in patches.
fn to_ref(v: &NodeView<'_>) -> NodeRef {
    NodeRef {
        id: v.id(),
        kind: v.kind(),
        component_id: v.component_id(),
        props: v.props(),
        children: v.children(),
        handlers: v.handlers(),
        span: v.span(),
    }
}

/// Finds the parent of `child_id` in `arena` and the child's index among the
/// parent's flattened child node list.
fn find_parent_and_index(arena: &IRArena, child_id: NodeId) -> Option<(NodeId, u16)> {
    for pid in arena.all_ids() {
        let parent = arena.get(pid)?;
        let mut index = 0u16;
        for child in parent.children() {
            if let Child::Node(cid) = child {
                if cid == child_id {
                    return Some((pid, index));
                }
                index = index.saturating_add(1);
            }
        }
    }
    None
}

/// Returns the ordered list of child node-ids for `v` (ignoring splices'
/// nested ordering beyond their item sequence). Used to detect reorders.
fn child_order(v: &NodeView<'_>) -> Vec<NodeId> {
    v.children()
        .iter()
        .flat_map(|c| match c {
            Child::Node(id) => vec![*id],
            Child::Splice { items } => items.iter().map(|(_, id)| *id).collect(),
            _ => vec![],
        })
        .collect()
}

/// Flattens a node's children into their node-id set.
fn child_ids(v: &NodeView<'_>) -> AHashSet<NodeId> {
    v.children()
        .iter()
        .flat_map(|c| match c {
            Child::Node(id) => vec![*id],
            Child::Splice { items } => items.iter().map(|(_, id)| *id).collect(),
            _ => vec![],
        })
        .collect()
}

/// `true` when `a` and `b` denote the same value, resolving interned strings to
/// their *content* rather than comparing positional [`flux_syntax::StringId`]s.
///
/// String literals are interned positionally per compile: `"first"` and
/// `"second"` can both land on `StringId(1)` in their respective compiles, so a
/// naive `Value` comparison reports them equal and a hot reload would silently
/// drop the edited label. Resolving through each arena's own `StringTable`
/// recovers the actual text so literal-text edits are detected and shipped.
fn values_equal(a: &Value, b: &Value, ta: &StringTable, tb: &StringTable) -> bool {
    match (a, b) {
        (Value::Str(id_a), Value::Str(id_b)) => match (ta.resolve(*id_a), tb.resolve(*id_b)) {
            (Some(sa), Some(sb)) => sa == sb,
            _ => id_a == id_b,
        },
        (Value::List(la), Value::List(lb)) => {
            la.len() == lb.len() && la.iter().zip(lb).all(|(x, y)| values_equal(x, y, ta, tb))
        }
        (Value::Record(ra), Value::Record(rb)) => {
            ra.len() == rb.len()
                && ra
                    .iter()
                    .zip(rb)
                    .all(|((ka, x), (kb, y))| ka == kb && values_equal(x, y, ta, tb))
        }
        _ => a == b,
    }
}

/// `true` when every prop key maps to the same value in both nodes.
///
/// Prefers the arena-stored prop hash (an O(1) `u64` compare) over unpacking
/// both cold blobs — see `IRArena::props_hash`. The hash is computed from all
/// `(PropIdx, Value)` fields at pack time, so a mismatch implies the fields
/// differ. When the hashes match we re-check the actual fields as a guard, and
/// compare interned strings by *content* (see [`values_equal`]) so literal-text
/// edits are not masked by positional `StringId` interning.
fn props_equal(o: &NodeView<'_>, n: &NodeView<'_>, old: &IRArena, new: &IRArena) -> bool {
    if o.props_hash() != n.props_hash() {
        return false;
    }
    let of = o.props();
    let of = of.fields();
    let nf = n.props();
    let nf = nf.fields();
    of.len() == nf.len()
        && of.iter().zip(nf).all(|((ka, va), (kb, vb))| {
            ka == kb && values_equal(va, vb, old.string_table(), new.string_table())
        })
}

/// Computes the [`PropDiff`] between two nodes.
///
/// The change list is built with content-aware value comparison (see
/// [`values_equal`]) so a literal-text edit — where the positional `StringId`
/// is unchanged but the resolved text differs — appears as a real change and
/// is shipped to the host on the `Patch::Update`.
fn props_diff(o: &NodeView<'_>, n: &NodeView<'_>, old: &IRArena, new: &IRArena) -> PropDiff {
    let o_fields = o.props();
    let o_fields = o_fields.fields();
    let n_fields = n.props();
    let n_fields = n_fields.fields();
    let changes: Vec<(PropIdx, Value)> = n_fields
        .iter()
        .filter(|(k, v)| {
            !o_fields.iter().any(|(ok, ov)| {
                ok == k && values_equal(ov, v, old.string_table(), new.string_table())
            })
        })
        .map(|(k, v)| (*k, v.clone()))
        .collect();
    let removals: Vec<PropIdx> = o_fields
        .iter()
        .filter(|(k, _)| !n_fields.iter().any(|(nk, _)| nk == k))
        .map(|(k, _)| *k)
        .collect();
    PropDiff { changes, removals }
}

/// `true` when both nodes bind the same handler ids AND every shared handler's
/// closure body is byte-identical.
///
/// Comparing content — not just ids — is required for hot reload: a prop
/// thunk (e.g. an interpolated string literal) keeps its stable `HandlerId`
/// across edits while its bytecode changes. An id-only compare would report
/// "no change" and suppress the `Patch::Handler` that drives the host's
/// re-materialize, silently breaking hot reload (FLUX-019 regression).
fn handlers_equal(
    old: &IRArena,
    new: &IRArena,
    o_handlers: &[HandlerId],
    n_handlers: &[HandlerId],
) -> bool {
    let o_set: AHashSet<HandlerId> = o_handlers.iter().copied().collect();
    let n_set: AHashSet<HandlerId> = n_handlers.iter().copied().collect();
    if o_set != n_set {
        return false;
    }
    o_set
        .iter()
        .all(|hid| match (old.closure(*hid), new.closure(*hid)) {
            (Some(a), Some(b)) => a.bytecode == b.bytecode,
            _ => false,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flux_ir::{ArenaBuilder, Node};
    use flux_syntax::{ComponentId, NodeKind, Props};

    /// Builds a single-node arena (id 1) with the given prop.
    fn single_prop(prop_value: Value) -> IRArena {
        let node = Node {
            id: NodeId::from(1u32),
            kind: NodeKind::Primitive,
            component_id: ComponentId::from(1u32),
            props: Props::from_fields(vec![(PropIdx::from(0u16), prop_value)]),
            children: vec![],
            handlers: vec![],
            span: Span::new(0, 0, 4),
        };
        let mut b = ArenaBuilder::new();
        b.pack(node);
        b.finish()
    }

    #[test]
    fn identical_hash_emits_no_update() {
        // Same prop value -> identical Props::hash -> no Update patch.
        let a = single_prop(Value::Int(12));
        let b = single_prop(Value::Int(12));
        let patches = diff(&a, &b);
        assert!(
            patches.is_empty(),
            "identical-hash nodes must emit no patch"
        );
        // The arena hash and Props::hash agree.
        let va = a.get(NodeId::from(1u32)).unwrap();
        let vb = b.get(NodeId::from(1u32)).unwrap();
        assert_eq!(va.props_hash(), vb.props_hash());
        assert_eq!(va.props_hash(), va.props().hash());
    }

    #[test]
    fn different_hash_emits_correct_update() {
        let a = single_prop(Value::Int(12));
        let b = single_prop(Value::Int(99));
        let patches = diff(&a, &b);
        assert_eq!(
            patches.len(),
            1,
            "different-hash must emit exactly one patch"
        );
        match &patches[0] {
            Patch::Update { id, props_diff } => {
                assert_eq!(*id, NodeId::from(1u32));
                assert_eq!(
                    props_diff.changes,
                    vec![(PropIdx::from(0u16), Value::Int(99))]
                );
            }
            other => panic!("expected Update, got {other:?}"),
        }
    }

    /// Builds a single-node arena whose node is `component_id`/`kind`.
    fn single_node(component: u32, kind: NodeKind) -> IRArena {
        let node = Node {
            id: NodeId::from(1u32),
            kind,
            component_id: ComponentId::from(component),
            props: Props::from_fields(vec![]),
            children: vec![],
            handlers: vec![],
            span: Span::new(0, 0, 4),
        };
        let mut b = ArenaBuilder::new();
        b.pack(node);
        b.finish()
    }

    #[test]
    fn component_swap_at_same_node_reattaches_instead_of_replacing() {
        // `Column` → `Row` at the same node id: state must survive, so the
        // differ emits a state-preserving Reattach, never a Replace.
        let a = single_node(1, NodeKind::Primitive);
        let b = single_node(2, NodeKind::Primitive);
        let patches = diff(&a, &b);
        assert_eq!(patches.len(), 1);
        match &patches[0] {
            Patch::Reattach {
                old_id,
                new_id,
                node,
            } => {
                assert_eq!(*old_id, NodeId::from(1u32));
                assert_eq!(*new_id, NodeId::from(1u32));
                assert_eq!(node.component_id, ComponentId::from(2u32));
            }
            other => panic!("expected Reattach, got {other:?}"),
        }
        assert!(patches[0].is_state_preserving());
    }

    #[test]
    fn kind_change_still_replaces() {
        // A genuine node-kind change (Primitive → If) is not the same construct
        // and must NOT silently inherit another node's state.
        let a = single_node(1, NodeKind::Primitive);
        let b = single_node(1, NodeKind::If);
        let patches = diff(&a, &b);
        assert_eq!(patches.len(), 1);
        assert!(matches!(&patches[0], Patch::Replace { .. }));
    }

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
}
