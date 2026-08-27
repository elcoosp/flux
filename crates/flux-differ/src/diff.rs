//! Keyed tree differencing implementation (FLUX-014).
//!
//! See [`crate`] for the algorithm overview. The public entry point is
//! [`diff`]; the remaining items are small, test-supported helpers.

use std::hash::{Hash, Hasher};

use ahash::AHashSet;
use flux_ir::{IRArena, NodeView};
use flux_syntax::{
    Child, ClosureRef, HandlerId, NodeId, NodeRef, Patch, PropDiff, PropIdx, SignalId, Span, Value,
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
        if o.kind() != n.kind() || o.component_id() != n.component_id() {
            emit_replace(&mut patches, &n);
            continue;
        }
        // Task 1 (FLUX-014 P3): node-level prop skip. `props_equal` short-circuits
        // on the arena-stored `u64` hash, so identical-hash nodes emit no
        // `Update` and we avoid deserialising either cold blob. Computed once per
        // node and reused below (cheap, behaviour-preserving).
        let props_equal = props_equal(&o, &n);
        let o_children = child_ids(&o);
        let n_children = child_ids(&n);
        if o_children == n_children {
            // Same child set: only order may differ.
            if child_order(&o) == child_order(&n) {
                if props_equal {
                    if handlers_equal(old, new, &o.handlers(), &n.handlers()) {
                        continue; // truly identical
                    }
                    // Only handler bodies changed → state-preserving fast path.
                    emit_handler(&mut patches, new, n.handlers(), o.handlers());
                    continue;
                }
                // Props changed; structure (children) unchanged.
                patches.push(Patch::Update {
                    id: *id,
                    props_diff: props_diff(&o, &n),
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
            props_diff: props_diff(&o, &n),
        });
    }

    // Nodes removed from the new tree.
    for id in old_ids.difference(&new_ids) {
        patches.push(Patch::Remove { id: *id });
    }

    // Nodes inserted into the new tree.
    for id in new_ids.difference(&old_ids) {
        if let Some((parent, index)) = find_parent_and_index(new, *id) {
            let n = new.get(*id).expect("present in new");
            patches.push(Patch::Insert {
                parent,
                index,
                node: to_ref(&n),
            });
        }
    }

    patches
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

/// `true` when every prop key maps to the same value in both nodes.
///
/// Prefers the arena-stored prop hash (an O(1) `u64` compare) over unpacking
/// both cold blobs — see `IRArena::props_hash`. The hash is computed from all
/// `(PropIdx, Value)` fields at pack time, so a mismatch implies the fields
/// differ. When the hashes match we still re-check the actual fields as a
/// belt-and-braces guard against any path that bypassed `props_equal` (and to
/// keep behaviour byte-identical to the pre-hash baseline).
fn props_equal(o: &NodeView<'_>, n: &NodeView<'_>) -> bool {
    if o.props_hash() != n.props_hash() {
        return false;
    }
    o.props().fields() == n.props().fields()
}

/// Computes the [`PropDiff`] between two nodes.
fn props_diff(o: &NodeView<'_>, n: &NodeView<'_>) -> PropDiff {
    let o_fields = o.props();
    let o_fields = o_fields.fields();
    let n_fields = n.props();
    let n_fields = n_fields.fields();
    let changes: Vec<(PropIdx, Value)> = n_fields
        .iter()
        .filter(|(k, v)| !o_fields.iter().any(|(ok, ov)| ok == k && ov == v))
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
