use ahash::{AHashMap, AHashSet};
use flux_ir::{IRArena, NodeView};
use flux_syntax::{Child, NodeId};

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

/// Returns the ordered list of child node-ids for `v` (ignoring splices'
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
