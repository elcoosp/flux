use super::compare::*;
use super::emit::*;
use super::tree::*;

use ahash::{AHashMap, AHashSet};
use flux_ir::IRArena;
use flux_syntax::{NodeId, Patch};

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
    // the SAME parent/index. Pair those up into a state-preserving
    // `Patch::Reattach` before falling back to remove+insert (roadmap Phase 3).
    let removed: Vec<NodeId> = old_ids.difference(&new_ids).copied().collect();
    let inserted: Vec<NodeId> = new_ids.difference(&old_ids).copied().collect();
    let pairs = reattach_pairs(old, new, &old_index, &new_index, &removed, &inserted);

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
        if let Some((parent, index)) = new_index.get(id).copied() {
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

    // FLUX-079: sort patches by (NodeId, Patch::tag) so the emitted stream is
    // deterministic regardless of hash-set iteration order (AHashSet is not
    // stable). Downstream code and binary serialisation depend on a stable order.
    patches.sort_by_key(patch_sort_key);

    patches
}

/// Stable key for sorting patches by target node id.
fn node_id_for_patch(p: &Patch) -> NodeId {
    let id: NodeId = match p {
        Patch::Replace { id, .. } => *id,
        Patch::Update { id, .. } => *id,
        Patch::Insert { parent, .. } => *parent,
        Patch::Remove { id } => *id,
        Patch::Reorder { parent, .. } => *parent,
        Patch::Handler { id, .. } => NodeId::from(*id),
        Patch::Reattach { old_id, .. } => *old_id,
        _ => NodeId::from(0u32), // exhaustive due to #[non_exhaustive] — unreachable
    };
    id
}

/// Stable key for sorting patches: (target node id, patch tag).
fn patch_sort_key(p: &Patch) -> (NodeId, u8) {
    (node_id_for_patch(p), p.tag())
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

    // FLUX-079: bucket `inserted` by (component_id, kind) so the inner loop
    // over `removed` does O(1) bucket lookup instead of O(|inserted|) per node.
    // This brings `reattach_pairs` from O(|removed| × |inserted|) down to
    // O(|removed| + |inserted|) amortized.
    let mut inserted_buckets: AHashMap<(u32, u8), Vec<NodeId>> = AHashMap::new();
    for &new_id in inserted {
        if taken.contains(&new_id) {
            continue;
        }
        let Some(n) = new.get(new_id) else { continue };
        let key = (n.component_id(), n.kind().tag());
        inserted_buckets.entry(key).or_default().push(new_id);
    }

    for &old_id in removed {
        let Some(o) = old.get(old_id) else { continue };
        let old_slot = old_index.get(&old_id).copied();
        let key = (o.component_id(), o.kind().tag());
        let bucket = match inserted_buckets.get(&key) {
            Some(b) => b,
            None => continue,
        };
        let mut matched = false;
        for &new_id in bucket {
            if taken.contains(&new_id) {
                continue;
            }
            if old_slot != new_index.get(&new_id).copied() {
                continue;
            }
            taken.insert(new_id);
            pairs.push((old_id, new_id));
            matched = true;
            break;
        }
        // Clean up the bucket if drained to keep memory bounded.
        if !matched {
            let b = inserted_buckets.get_mut(&key).unwrap();
            b.retain(|&id| !taken.contains(&id));
            if b.is_empty() {
                inserted_buckets.remove(&key);
            }
        }
    }
    pairs
}
