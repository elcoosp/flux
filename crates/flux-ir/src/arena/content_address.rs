//! Content-addressed id remapping for `IRArena` (FLUX-074, item A).
//!
//! `content_address` rebuilds the arena with ids derived from each node's
//! structural content (kind/component/prop-hash/children/parent/position) so a
//! pure text-above edit keeps the node's id and its view instance survives hot
//! reload. The bottom-up/top-down passes and the remap helpers live here.
use super::IRArena;
use super::blob::hash_children;
use crate::builder::{ArenaBuilder, Node};
use ahash::AHashMap;
use flux_syntax::{Child, NodeId};
impl IRArena {
    /// Remaps every node id in this arena to a **content-addressed** id (FLUX-074,
    /// item A).
    ///
    /// The final id for a node is derived from its structural content — its wire
    /// `kind`, its `component_id`, its prop *value* hash, its children's content
    /// hashes (computed bottom-up), its position among its parent's children, and its
    /// parent's own content id — and ignores source spans. Because the parent/position
    /// are unchanged by a pure text-above edit (which only shifts spans, never
    /// content or structure), a node whose source moved but whose content is identical
    /// keeps its id. That is what lets its view instance survive a hot reload instead
    /// of being torn down and rebuilt (FLUX-074, the roast's core ask).
    ///
    /// `span` is retained for diagnostics; only the *identity* changes. Every other
    /// field (props, handlers, closure table, string table) is preserved, and the
    /// ADR-0027 signal-graph side-tables (`signal_deps`/`prop_thunk`/`prop_layout`/
    /// `item_slot`) are re-keyed under the new ids so they stay attached to the same
    /// node.
    ///
    /// The computation is acyclic in two passes: a bottom-up pass assigns a
    /// *local* content id (parent-independent), then a top-down pass mixes in the
    /// parent's final id and the child's position to break sibling collisions while
    /// Returns the `old_id → content_addressed_id` mapping so callers that keep
    /// node-id-keyed state outside the arena (e.g. the devserver's
    /// `prop_thunks` table) can re-key it in lockstep.
    pub fn content_address(&mut self) -> AHashMap<NodeId, NodeId> {
        let ids: Vec<NodeId> = self.all_ids().collect();

        // 1. Discover each node's parent (a Flux reactive tree is a tree, not a DAG:
        //    every node has at most one parent). Roots have no parent → parent id 0.
        let mut parent_of: AHashMap<NodeId, Option<NodeId>> = AHashMap::new();
        for &id in &ids {
            parent_of.insert(id, None);
        }
        for &id in &ids {
            if let Some(view) = self.get(id) {
                for child in view.children() {
                    for cid in child.node_ids() {
                        parent_of.insert(cid, Some(id));
                    }
                }
            }
        }

        // 2. Bottom-up pass: a parent-independent *local* content id per node, derived
        //    from its own content and its children's local ids. Uses an explicit
        //    stack (not recursion) so deep trees cannot overflow (audit P2.12 part a).
        let mut local_ids: AHashMap<NodeId, NodeId> = AHashMap::with_capacity(ids.len());
        compute_all_local_ids(&mut local_ids, self, &ids);

        // 3. Top-down pass: mix the parent's final id + this node's position into the
        //    final id. Parent is assigned before child, so this never cycles. Roots get
        //    parent id 0 and position 0.
        let mut final_ids: AHashMap<NodeId, NodeId> = AHashMap::with_capacity(ids.len());
        let roots: Vec<NodeId> = ids
            .iter()
            .copied()
            .filter(|id| parent_of.get(id).copied().flatten().is_none())
            .collect();
        assign_all_final_ids(&local_ids, &mut final_ids, self, &roots);

        // 4. Rebuild a fresh arena with remapped ids and remapped child references.
        let mut builder = ArenaBuilder::new();
        for &id in &ids {
            let view = match self.get(id) {
                Some(v) => v,
                None => continue,
            };
            let new_id = final_ids[&id];
            let new_children = remap_children(&view.children(), &final_ids);
            builder.pack(Node {
                id: new_id,
                kind: view.kind(),
                component_id: view.component_id(),
                props: view.props(),
                children: new_children,
                handlers: view.handlers(),
                span: view.span(),
            });
        }
        let mut new_arena = builder.finish();
        // `content_address` remaps node ids but must preserve the interning
        // table — literal strings are content, not structure, and the wire
        // `Init` frame (Appendix D §D.12.2) ships the full string table so the
        // host can resolve literal ids. `ArenaBuilder::new()` starts with an
        // empty table, so carry the original over explicitly.
        new_arena = new_arena.with_string_table(self.string_table().clone());

        // 5. Re-attach closures and ADR-0027 signal metadata under the new ids.
        for c in self.closures.values() {
            new_arena.add_closure(c.clone());
        }
        for &id in &ids {
            let new_id = final_ids[&id];
            let deps = self.signal_deps_of(id).to_vec();
            let thunk = self.prop_thunk_of(id).cloned();
            let layout = self.prop_layout_of(id).to_vec();
            let item_slot = self.item_slot_of(id);
            new_arena.set_signal_metadata(new_id, deps, thunk, layout, item_slot);
        }

        // 6. Swap the rebuilt arena into `self`.
        *self = new_arena;
        final_ids
    }
}

/// Bottom-up memoised computation of every node's *local* content id.
///
/// The local id folds the node's kind/component_id/prop hash and the local ids
/// of its children (resolved first), but NOT its parent or position — so
/// identical subtrees share a local id. The top-down pass turns local ids into
/// final, position-disambiguated ids.
///
/// Uses an explicit stack rather than recursion so deep trees cannot overflow
/// the Rust call stack (audit P2.12 part a). A two-phase marker
/// (`children_processed` bool) preserves post-order semantics: a node's local
/// id is computed only after every descendant's local id is cached.
fn compute_all_local_ids(local: &mut AHashMap<NodeId, NodeId>, arena: &IRArena, ids: &[NodeId]) {
    // Stack entries: (node_id, children_have_been_processed).
    // Phase 1 (false): push children that aren't cached yet, then re-push
    // self with true. Phase 2 (true): all children are cached, compute self.
    let mut stack: Vec<(NodeId, bool)> = ids.iter().map(|&id| (id, false)).collect();
    while let Some((id, children_done)) = stack.pop() {
        if local.contains_key(&id) {
            continue;
        }
        if children_done {
            let view = arena
                .get(id)
                .expect("node present during content addressing");
            let remapped_children: Vec<Child> = view
                .children()
                .iter()
                .map(|child| match child {
                    Child::Node(cid) => Child::Node(
                        *local
                            .get(cid)
                            .expect("child local id computed before parent"),
                    ),
                    Child::Splice { items } => Child::Splice {
                        items: items
                            .iter()
                            .map(|(k, cid)| {
                                (
                                    *k,
                                    *local
                                        .get(cid)
                                        .expect("spliced child local id computed before parent"),
                                )
                            })
                            .collect(),
                    },
                    other => other.clone(),
                })
                .collect();
            let children_hash = hash_children(&remapped_children);
            let local_id = flux_syntax::content_addressed_id(
                0,
                view.kind().tag(),
                view.component_id(),
                view.props_hash(),
                children_hash,
                None,
            );
            local.insert(id, local_id);
        } else {
            stack.push((id, true));
            if let Some(view) = arena.get(id) {
                for child in &view.children() {
                    match child {
                        Child::Node(cid) => {
                            if !local.contains_key(cid) {
                                stack.push((*cid, false));
                            }
                        }
                        Child::Splice { items } => {
                            for (_, cid) in items {
                                if !local.contains_key(cid) {
                                    stack.push((*cid, false));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

/// Top-down assignment of every node's *final* content id by mixing in its
/// parent's final id and its own position (index among the parent's children, or
/// the `ForEach` splice key for spliced items).
///
/// Runs parent-before-child, so `parent_final` is always already known — the
/// stack-based traversal never revisits the parent and therefore cannot cycle.
/// Uses an explicit stack rather than recursion (audit P2.12 part a). Roots
/// receive `parent_final = 0` and `position = root_slot` so distinct roots
/// (audit P2.20) remain distinct even when structurally identical.
fn assign_all_final_ids(
    local: &AHashMap<NodeId, NodeId>,
    final_ids: &mut AHashMap<NodeId, NodeId>,
    arena: &IRArena,
    roots: &[NodeId],
) {
    // Stack entries: (parent_final_id, position, node_id).
    let mut stack: Vec<(NodeId, u64, NodeId)> = Vec::with_capacity(roots.len() * 2);
    for (root_slot, root) in roots.iter().enumerate() {
        stack.push((0, root_slot as u64, *root));
    }
    while let Some((parent_final, position, id)) = stack.pop() {
        let view = arena
            .get(id)
            .expect("node present during content addressing");
        let children_local = remap_children(&view.children(), local);
        let children_hash = hash_children(&children_local);
        let final_id = flux_syntax::content_addressed_id(
            parent_final,
            view.kind().tag(),
            view.component_id(),
            view.props_hash(),
            children_hash,
            Some(position),
        );
        final_ids.insert(id, final_id);

        // Collect children, then push in reverse so they are processed in
        // original left-to-right order by the LIFO stack.
        let mut children: Vec<(u64, NodeId)> = Vec::new();
        for (pos, child) in view.children().iter().enumerate() {
            match child {
                Child::Node(cid) => children.push((pos as u64, *cid)),
                Child::Splice { items } => {
                    for (k, cid) in items {
                        children.push((*k, *cid));
                    }
                }
                _ => {}
            }
        }
        for (pos, cid) in children.into_iter().rev() {
            stack.push((final_id, pos, cid));
        }
    }
}

/// Returns a copy of `children` with every `NodeId` replaced by `remap(id)`.
fn remap_children(children: &[Child], remap: &AHashMap<NodeId, NodeId>) -> Vec<Child> {
    children
        .iter()
        .map(|child| match child {
            Child::Node(cid) => Child::Node(*remap.get(cid).unwrap_or(cid)),
            Child::Splice { items } => Child::Splice {
                items: items
                    .iter()
                    .map(|(k, cid)| (*k, *remap.get(cid).unwrap_or(cid)))
                    .collect(),
            },
            other => other.clone(),
        })
        .collect()
}
