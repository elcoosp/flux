use super::compare::*;
use super::emit::*;
use super::tree::*;

use ahash::{AHashMap, AHashSet};
use flux_ir::IRArena;
use flux_syntax::{Child, NodeId, NodeKind, Patch, Span};

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

    // FLUX-079: precompute the parent/index projection once per arena (O(n))
    // so the insert loop and the `reattach_pairs` inner loop read it in O(1)
    // instead of re-scanning the whole arena per node (the old
    // `find_parent_and_index` cold path was O(n·r·i)).
    let old_index = build_parent_index(old);
    let new_index = build_parent_index(new);

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
    // the same parent/index. Pair those up into a state-preserving
    // `Patch::Reattach` before falling back to remove+insert (roadmap Phase 3).
    let removed: Vec<NodeId> = old_ids.difference(&new_ids).copied().collect();
    let mut inserted: Vec<NodeId> = new_ids.difference(&old_ids).copied().collect();
    let pairs = reattach_pairs(old, new, &old_index, &new_index, &removed, &inserted);

    for id in &removed {
        if pairs.iter().any(|(old_id, _)| old_id == id) {
            continue;
        }
        patches.push(Patch::Remove { id: *id });
    }

    // Audit C8: Insert carries an absolute index into the new tree; emitting
    // inserts in hash-set order corrupts the host's child order when more
    // than one row is added per frame. Sort by (parent, index) — stable and
    // deterministic — before any Reattach filtering below.
    inserted.sort_by_key(|id| new_index.get(id).copied());

    for id in &inserted {
        if pairs.iter().any(|(_, new_id)| new_id == id) {
            continue;
        }
        if let Some((parent, index)) = new_index.get(id).copied() {
            let n = new.get(*id).expect("present in new");
            patches.push(Patch::Insert {
                parent,
                index,
                node: to_ref(&n),
            });
        } else if is_root_of_new(new, id) {
            // Audit C9: arena roots have no entry in `new_index` (no parent),
            // so new top-level components were silently dropped on hot reload.
            // Emit the insert against the stable synthetic wrapper id the
            // devserver's tree.rs uses for the multi-root Init (§D.12.2).
            let n = new.get(*id).expect("present in new");
            patches.push(Patch::Insert {
                parent: synthetic_root_id(),
                index: root_position(new, id),
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
pub(crate) fn reattach_pairs(
    old: &IRArena,
    new: &IRArena,
    old_index: &AHashMap<NodeId, (NodeId, u16)>,
    new_index: &AHashMap<NodeId, (NodeId, u16)>,
    removed: &[NodeId],
    inserted: &[NodeId],
) -> Vec<(NodeId, NodeId)> {
    let mut pairs: Vec<(NodeId, NodeId)> = Vec::new();
    let mut taken: AHashSet<NodeId> = AHashSet::new();

    // Audit P2.12(c): build a lookup from (component_id, kind, parent_slot) to
    // candidate inserted nodes, so pairing is a hash-join instead of an
    // O(removed × inserted) nested loop.
    let mut candidates: AHashMap<(u32, NodeKind, NodeId, u16), Vec<NodeId>> = AHashMap::new();
    for new_id in inserted {
        let Some(n) = new.get(*new_id) else { continue };
        if let Some((parent, index)) = new_index.get(new_id) {
            candidates
                .entry((u32::from(n.component_id()), n.kind(), *parent, *index))
                .or_default()
                .push(*new_id);
        }
    }

    for old_id in removed {
        let Some(o) = old.get(*old_id) else { continue };
        let Some((parent, index)) = old_index.get(old_id) else {
            continue;
        };
        let key = (u32::from(o.component_id()), o.kind(), *parent, *index);
        if let Some(cands) = candidates.get(&key) {
            for new_id in cands {
                if !taken.contains(new_id) {
                    taken.insert(*new_id);
                    pairs.push((*old_id, *new_id));
                    break;
                }
            }
        }
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;
    use flux_ir::{ArenaBuilder, Node};
    use flux_syntax::{Child, ComponentId, NodeId, NodeKind, Props, Span};

    fn build_tree(
        root_id: u32,
        root_kind: NodeKind,
        children: &[(u32, NodeKind)],
    ) -> flux_ir::IRArena {
        let mut b = ArenaBuilder::new();
        let root = Node {
            id: NodeId::from(root_id),
            kind: root_kind,
            component_id: ComponentId::from(0u32),
            props: Props::from_fields(vec![]),
            children: children
                .iter()
                .map(|(cid, _)| Child::Node(NodeId::from(*cid)))
                .collect(),
            handlers: vec![],
            span: Span::new(0, 0, 10),
        };
        b.pack(root);
        for (cid, kind) in children {
            let child = Node {
                id: NodeId::from(*cid),
                kind: *kind,
                component_id: ComponentId::from(*cid),
                props: Props::from_fields(vec![]),
                children: vec![],
                handlers: vec![],
                span: Span::new(0, 10, 20),
            };
            b.pack(child);
        }
        b.finish()
    }

    #[test]
    fn multi_insert_emits_ascending_indices_per_parent() {
        let old = build_tree(1, NodeKind::Component, &[(2, NodeKind::Primitive)]);
        let new = build_tree(
            1,
            NodeKind::Component,
            &[
                (3, NodeKind::Primitive),
                (4, NodeKind::Primitive),
                (2, NodeKind::Primitive),
            ],
        );
        let patches = diff(&old, &new);
        let inserts: Vec<(u32, u32)> = patches
            .iter()
            .filter_map(|p| match p {
                Patch::Insert { parent, index, .. } => {
                    Some((u32::from(*parent), u32::from(*index)))
                }
                _ => None,
            })
            .collect();
        let mut sorted = inserts.clone();
        sorted.sort();
        assert_eq!(
            inserts, sorted,
            "inserts must be emitted in (parent, index) order (audit C8)"
        );
    }

    #[test]
    fn new_root_component_is_inserted_on_hot_reload() {
        let old = build_tree(1, NodeKind::Component, &[]);
        let mut b = ArenaBuilder::new();
        b.pack(Node {
            id: NodeId::from(1u32),
            kind: NodeKind::Component,
            component_id: ComponentId::from(1u32),
            props: Props::from_fields(vec![]),
            children: vec![],
            handlers: vec![],
            span: Span::new(0, 0, 10),
        });
        b.pack(Node {
            id: NodeId::from(2u32),
            kind: NodeKind::Component,
            component_id: ComponentId::from(2u32),
            props: Props::from_fields(vec![]),
            children: vec![],
            handlers: vec![],
            span: Span::new(0, 10, 20),
        });
        let new = b.finish();
        let patches = diff(&old, &new);
        assert!(
            patches.iter().any(|p| matches!(p, Patch::Insert { .. })),
            "a newly added top-level component must produce an Insert patch (audit C9)"
        );
    }
}

/// Audit C9: true when `id` is a root of `new` (no parent in the arena).
fn is_root_of_new(new: &flux_ir::IRArena, id: &NodeId) -> bool {
    !new.all_ids().any(|nid| {
        new.get(nid).map_or(false, |n| {
            n.children()
                .iter()
                .any(|c| matches!(c, Child::Node(n) if *n == *id))
        })
    })
}

/// Audit C9: the stable synthetic wrapper id for multi-root Init frames.
fn synthetic_root_id() -> NodeId {
    flux_ir::compute_node_id(0, NodeKind::Component, Span::new(0, 0, 0), None)
}

/// Audit C9: the index of `id` among the new tree's roots.
fn root_position(new: &flux_ir::IRArena, id: &NodeId) -> u16 {
    let roots: Vec<_> = new
        .all_ids()
        .filter(|nid| {
            !new.all_ids().any(|pid| {
                new.get(pid).map_or(false, |p| {
                    p.children()
                        .iter()
                        .any(|c| matches!(c, Child::Node(n) if *n == *nid))
                })
            })
        })
        .collect();
    roots.iter().position(|nid| *nid == *id).unwrap_or(0) as u16
}
