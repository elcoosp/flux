use ahash::{AHashMap, AHashSet};
use flux_ir::{IRArena, NodeView};
use flux_syntax::{Child, NodeId, NodeKind, Span};

pub(crate) fn build_parent_index(arena: &IRArena) -> AHashMap<NodeId, (NodeId, u16)> {
    let mut index: AHashMap<NodeId, (NodeId, u16)> = AHashMap::new();
    for pid in arena.all_ids() {
        let parent = match arena.get(pid) {
            Some(p) => p,
            None => continue,
        };
        let mut child_index = 0u16;
        for child in parent.children() {
            for cid in child.node_ids() {
                index.insert(cid, (pid, child_index));
                child_index = child_index.saturating_add(1);
            }
        }
    }
    index
}

/// Returns the ordered list of child node-ids for `v` (ignoring splices'\
/// nested ordering beyond their item sequence). Used to detect reorders.
pub(crate) fn child_order(v: &NodeView<'_>) -> Vec<NodeId> {
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
pub(crate) fn child_ids(v: &NodeView<'_>) -> AHashSet<NodeId> {
    v.children()
        .iter()
        .flat_map(|c| match c {
            Child::Node(id) => vec![*id],
            Child::Splice { items } => items.iter().map(|(_, id)| *id).collect(),
            _ => vec![],
        })
        .collect()
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

/// Audit C9: true when `id` is a root of `new` (no parent in the arena).
pub(crate) fn is_root_of_new(new: &IRArena, id: &NodeId) -> bool {
    !new.all_ids().any(|nid| {
        new.get(nid).map_or(false, |n| {
            n.children()
                .iter()
                .any(|c| matches!(c, Child::Node(n) if *n == *id))
        })
    })
}

/// Audit C9: the stable synthetic wrapper id for multi-root Init frames.
pub(crate) fn synthetic_root_id() -> NodeId {
    flux_ir::compute_node_id(0, NodeKind::Component, Span::new(0, 0, 0), None)
}

/// Audit C9: the index of `id` among the new tree's roots.
pub(crate) fn root_position(new: &IRArena, id: &NodeId) -> u16 {
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
