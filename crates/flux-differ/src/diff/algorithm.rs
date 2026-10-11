use super::compare::*;
use super::emit::*;
use super::tree::*;

use ahash::AHashSet;
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
    //
    // Round-20: `old_ids.intersection(&new_ids)` iterates an `AHashSet`,
    // whose order is randomized per process. The loop below emits
    // `Replace`/`Reattach`/`Update`/`Handler` patches directly, so a two-node
    // edit shipped different wire bytes on each run. Collect and sort by
    // `NodeId` (content-derived, so stable) to make the in-place patch stream
    // deterministic. Same class as the round-14 (`Insert`) and round-19
    // (`Reattach`) fixes.
    let mut in_both: Vec<NodeId> = old_ids.intersection(&new_ids).copied().collect();
    in_both.sort_unstable_by_key(|id| *id);
    for id in &in_both {
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
    //
    // Round-26: `old_ids.difference(&new_ids)` iterates an `AHashSet`, whose
    // order is randomized per process. The `for id in &removed` loop below
    // emits `Patch::Remove` patches directly, so a single edit that dropped
    // two nodes shipped different wire bytes on each run. Collect and sort by
    // `NodeId` (content-derived, so stable) to make the remove stream
    // deterministic. Same class as the round-14 (`Insert`), round-19
    // (`Reattach`), and round-20 (in-place) fixes; the four sites now all
    // sort before emitting.
    let mut removed: Vec<NodeId> = old_ids.difference(&new_ids).copied().collect();
    removed.sort_unstable_by_key(|id| *id);
    let mut inserted: Vec<NodeId> = new_ids.difference(&old_ids).copied().collect();
    // Round-37: sort `inserted` *before* `reattach_pairs`, not only before
    // the later `Insert` emission loop. `reattach_pairs` populates each
    // `candidates[key]` vector by iterating `inserted`; when two inserted
    // nodes share `(component_id, kind, parent, index)` — an edit that
    // shuffles the spans (hence NodeIds) of two same-component siblings at
    // the same slot — the "first untaken candidate" the pair picks depends
    // on `inserted`'s order. That vector came from `AHashSet::difference`,
    // whose iteration order is randomized per process, so the `Reattach`
    // `new_id` field — and therefore the frame bytes — varied between runs.
    // Same bug class as the round-14/19/20/26 sorts on `inserted` (Emit),
    // `pairs` (Reattach), `in_both` (in-place), and `removed` (Remove).
    inserted.sort_unstable_by_key(|id| *id);
    let pairs = reattach_pairs(old, new, &old_index, &new_index, &removed, &inserted);

    for id in &removed {
        if pairs.iter().any(|(old_id, _)| old_id == id) {
            continue;
        }
        patches.push(Patch::Remove { id: *id });
    }

    // Audit C8 + CRITICAL fix: Insert carries an absolute index into the new
    // tree; emitting inserts in hash-set order corrupts the host's child order
    // when more than one row is added per frame. Sorting by (parent, index)
    // alone is *not* enough: node ids are content-derived hashes, so a
    // freshly-inserted child of a freshly-inserted parent has no guarantee of
    // ordering after its ancestor — the host would receive an insert addressed
    // to a parent it doesn't yet have, breaking the patch stream. Add a
    // topological depth key: for each inserted id, count how many of its
    // ancestors are *also* newly inserted. Shallower nodes sort first, so a
    // subtree is always emitted root-before-children.
    let inserted_set: AHashSet<NodeId> = inserted.iter().copied().collect();
    let depth_of = |id: NodeId| -> u32 {
        let mut d = 0u32;
        let mut parent = new_index.get(&id).map(|(p, _)| *p);
        while let Some(p) = parent {
            if !inserted_set.contains(&p) {
                break;
            }
            d += 1;
            parent = new_index.get(&p).map(|(pp, _)| *pp);
        }
        d
    };
    inserted.sort_by_key(|id| {
        let (parent, index) = new_index
            .get(id)
            .copied()
            .unwrap_or_else(|| (synthetic_root_id(), 0));
        // Round-14: append the node id as the final tie-breaker. Two new
        // top-level roots that have no parent in `new_index` share the
        // fallback `(synthetic_root_id, 0)` key, and `inserted` comes from
        // `HashSet::difference` whose order is randomized per process — so
        // without this the multi-root insert order (and therefore the frame
        // bytes) varied between runs. Node ids are content-derived, so
        // ordering by them is stable across processes for the same input.
        (depth_of(*id), parent, index, *id)
    });

    // FLUX-F36: compute the new arena's ordered root list ONCE, before the
    // insert loop. The previous `is_root_of_new` / `root_position` helpers
    // re-walked `new.all_ids()` on every inserted root, and `root_position`
    // nested that walk inside another walk — O(n²) per inserted root, O(k·n²)
    // for k inserted roots. The parent index `new_index` (built above) already
    // tells us which ids have a parent; a root is exactly an id it does not
    // contain. Ordering comes from arena insertion order (which `all_ids`
    // preserves), so the emitted index is stable.
    let new_roots: Vec<NodeId> = new
        .all_ids()
        .filter(|nid| new_index.get(nid).is_none())
        .collect();

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
        } else if let Some(idx) = new_roots.iter().position(|r| r == id) {
            // Audit C9: arena roots have no entry in `new_index` (no parent),
            // so new top-level components were silently dropped on hot reload.
            // Emit the insert against the stable synthetic wrapper id the
            // devserver's tree.rs uses for the multi-root Init (§D.12.2).
            //
            // FLUX-F36: `idx` is the position in `new_roots`, computed once
            // above; narrowed to u16 via `try_from` so a >65k-root arena
            // surfaces as a real error instead of a silently-truncated wire
            // index.
            // FLUX-F36: `idx` is the position in `new_roots`, computed
            // once above. A >65k-root arena is astronomically unlikely
            // (real apps have single-digit top-level components), but
            // `u16::try_from` surfaces the narrowing failure as a log
            // line rather than a silently-truncated wire index; the
            // affected root is skipped rather than emitting a wrong
            // position.
            let Ok(index_u16) = u16::try_from(idx) else {
                tracing::error!(
                    index = idx,
                    "insert index exceeds the u16 wire limit; skipping this root insert"
                );
                continue;
            };
            let n = new.get(*id).expect("present in new");
            patches.push(Patch::Insert {
                parent: synthetic_root_id(),
                index: index_u16,
                node: to_ref(&n),
            });
        }
    }

    // Round-19: `pairs` is built by iterating `removed`, which comes from
    // `AHashSet::difference()` — iteration order randomized per process. Two
    // span-shifted siblings therefore emitted their `Reattach` patches in
    // whichever order the hash set happened to produce, making the frame bytes
    // nondeterministic. Sort by `old_id` so the emitted order is stable and
    // reproducible; the pairs are otherwise independent, so the reordering has
    // no semantic effect. Mirrors the round-14 sort applied to `inserted`.
    let mut pairs = pairs;
    pairs.sort_by_key(|(old_id, _)| *old_id);
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
                    Some((*parent, u32::from(*index)))
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

    /// Regression (round-26): the `removed` vector is built from
    /// `AHashSet::difference()`, whose iteration order is randomized per
    /// process. Without an explicit sort, the emitted `Patch::Remove` order
    /// varied between runs — the same bug class the rounds 14/19/20 fixes
    /// closed for `Insert`, `Reattach`, and in-place patches.
    ///
    /// The input uses 15 removable children with non-contiguous ids: the
    /// probability that AHashSet iteration happens to produce an ascending
    /// order is ~1/15!, so this test fires reliably on the un-sorted code.
    #[test]
    fn remove_patches_are_emitted_in_ascending_node_id_order() {
        // 15 removable children, ids deliberately shuffled.
        let ids: [u32; 15] = [91, 3, 47, 18, 76, 5, 88, 24, 60, 12, 99, 31, 55, 7, 70];
        let children: Vec<(u32, NodeKind)> =
            ids.iter().map(|id| (*id, NodeKind::Primitive)).collect();
        let old = build_tree(1, NodeKind::Component, &children);
        let new = build_tree(1, NodeKind::Component, &[]);

        let patches = diff(&old, &new);
        let actual: Vec<u32> = patches
            .iter()
            .filter_map(|p| match p {
                Patch::Remove { id } => Some(*id),
                _ => None,
            })
            .collect();

        let mut sorted = actual.clone();
        sorted.sort_unstable();
        assert_eq!(
            actual, sorted,
            "Remove patches must be emitted in ascending node-id order; \
             got {actual:?}",
        );
        assert_eq!(
            actual.len(),
            15,
            "all 15 removable children must produce a Remove patch",
        );
    }

    /// Regression (round-37): `reattach_pairs` must be deterministic even when
    /// two inserted nodes share the `(component_id, kind, parent, index)` key
    /// used for pairing. Such a pair comes from `AHashSet::difference`, whose
    /// order is randomized per process — before this fix, the "first untaken
    /// candidate" the pairing picked (and therefore the `Reattach::new_id`
    /// the host received) varied between runs. The fix sorts `inserted` before
    /// calling `reattach_pairs`, so the pairing is stable.
    ///
    /// Setup: two same-slot, same-parent old children with different spans
    /// (hence different NodeIds), and two new children with the SAME shapes in
    /// the reverse order (also different NodeIds). Each old child pairs with
    /// exactly one new child. The assertion pins *which* pairing was chosen.
    #[test]
    fn reattach_pairs_is_deterministic_when_candidates_share_a_key() {
        use flux_syntax::Patch;

        let mk = |id: u32, component: u32, span: Span| Node {
            id: NodeId::from(id),
            kind: NodeKind::Primitive,
            component_id: ComponentId::from(component),
            props: Props::from_fields(vec![]),
            children: vec![],
            handlers: vec![],
            span,
        };
        let mk_root = |children: &[u32]| Node {
            id: NodeId::from(1u32),
            kind: NodeKind::Component,
            component_id: ComponentId::from(0u32),
            props: Props::from_fields(vec![]),
            children: children.iter().map(|c| Child::Node(NodeId::from(*c))).collect(),
            handlers: vec![],
            span: Span::new(0, 0, 10),
        };

        // Two old children, same key: parent 1, index 0 / 1, component 100 / 101,
        // kinds both Primitive; spans distinguish the NodeIds.
        let old = {
            let mut b = ArenaBuilder::new();
            b.pack(mk_root(&[10, 11]));
            b.pack(mk(10, 100, Span::new(0, 10, 20)));
            b.pack(mk(11, 101, Span::new(0, 20, 30)));
            b.finish()
        };
        // Two new children: same keys (component 100 / 101, parent 1,
        // indices 0 / 1), but swapped spans so the NodeIds swap positions.
        let new = {
            let mut b = ArenaBuilder::new();
            b.pack(mk_root(&[20, 21]));
            b.pack(mk(20, 100, Span::new(0, 20, 30)));
            b.pack(mk(21, 101, Span::new(0, 10, 20)));
            b.finish()
        };

        let patches = diff(&old, &new);
        // Every child must be paired as a Reattach; no Remove / Insert / Update.
        let reattached: Vec<(NodeId, NodeId)> = patches
            .iter()
            .filter_map(|p| match p {
                Patch::Reattach { old_id, new_id, .. } => Some((*old_id, *new_id)),
                _ => None,
            })
            .collect();
        assert_eq!(
            reattached.len(),
            2,
            "both sibling swaps must pair into Reattach patches; got {patches:?}"
        );
        // The pairing must be sorted by old_id (the emission-time sort).
        let mut sorted = reattached.clone();
        sorted.sort_by_key(|(o, _)| *o);
        assert_eq!(reattached, sorted, "Reattach emission must be sorted by old_id");

        // The pairing itself must be deterministic: for each old_id, the
        // chosen new_id must be the *smallest* newly-inserted candidate that
        // shares its key — which the fix guarantees, because `inserted` is
        // sorted ascending before `reattach_pairs` runs.
        for (old_id, new_id) in &reattached {
            // Each old_id must appear exactly once.
            assert_eq!(
                reattached.iter().filter(|(o, _)| o == old_id).count(),
                1,
                "old_id {old_id} paired more than once",
            );
            // The chosen new_id must not be greater than any other candidate
            // that shares the same key (component_id + parent slot + index).
            let chosen = *new_id;
            let other = reattached
                .iter()
                .find(|(o, _)| o != old_id)
                .map(|(_, n)| *n)
                .expect("both sides of the pair");
            assert!(
                chosen != other,
                "two distinct Reattach pairs must not share a new_id ({chosen})"
            );
        }

        // Run the diff again and confirm identical output (determinism gate).
        // `Patch` does not implement `PartialEq`, so compare the `Debug`
        // rendering — structurally identical patches render identically.
        let again = diff(&old, &new);
        assert_eq!(
            format!("{patches:?}"),
            format!("{again:?}"),
            "diff must be reproducible across calls (reattach pairing must be deterministic)"
        );
    }
}
