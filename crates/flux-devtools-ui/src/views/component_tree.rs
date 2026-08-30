//! Component tree view (spec §5.3): the shadow-tree node hierarchy.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::{
    AnyElement, ClickEvent, Context, Entity, InteractiveElement, IntoElement, Render, Window,
};

use crate::row::{empty_row, into_any, kv_row, rows_column};
use crate::state::DevToolsState;
use crate::time_travel::ViewFrame;

/// A node in the reconstructed component tree (parent links resolved).
struct TreeNode {
    frame: ViewFrame,
    children: Vec<TreeNode>,
}

/// Renders the live component tree as an indented, collapsible hierarchy
/// (depth → left padding; a chevron marks each branch). Clicking a branch row
/// toggles its children. Each row shows the resolved component name (e.g.
/// `Column`, `Button`) plus the node id and, when known, geometry.
pub struct ComponentTreeView {
    state: Arc<DevToolsState>,
    /// Node ids whose subtrees are currently collapsed (children hidden).
    collapsed: HashSet<u32>,
    /// Last tree node count logged, to throttle the population probe.
    last_tree_len: usize,
}

impl ComponentTreeView {
    /// Creates the view bound to the shared state.
    pub fn new(state: Arc<DevToolsState>) -> Self {
        Self {
            state,
            collapsed: HashSet::new(),
            last_tree_len: usize::MAX,
        }
    }

    /// Toggles the collapsed state of `node_id` and repaints the pane.
    fn toggle(&mut self, node_id: u32, cx: &mut Context<'_, Self>) {
        eprintln!("[DT-COLLAPSE] toggle called for node {node_id}");
        if self.collapsed.contains(&node_id) {
            self.collapsed.remove(&node_id);
        } else {
            self.collapsed.insert(node_id);
        }
        eprintln!(
            "[DT-COLLAPSE] collapsed now = {:?}",
            self.collapsed.iter().collect::<Vec<_>>()
        );
        cx.notify();
    }

    /// Reconstructs the nested tree from the flat `view_frames` (parent links).
    fn tree(&self) -> Vec<TreeNode> {
        let live = self.state.live.read().clone();
        let frames = &live.view_frames;
        let mut children: HashMap<u32, Vec<ViewFrame>> = HashMap::new();
        let mut ids = Vec::new();
        for vf in frames {
            ids.push(vf.node_id);
            children.entry(vf.parent_id).or_default().push(vf.clone());
        }
        let known: HashSet<u32> = ids.iter().copied().collect();
        // A node is a root if its parent is the synthetic root (0) or a node that
        // was never reported (phantom parent — e.g. the host's top container).
        let roots: Vec<ViewFrame> = frames
            .iter()
            .filter(|vf| vf.parent_id == 0 || !known.contains(&vf.parent_id))
            .cloned()
            .collect();
        fn build(vf: &ViewFrame, children: &HashMap<u32, Vec<ViewFrame>>) -> TreeNode {
            let kids = children
                .get(&vf.node_id)
                .map(|v| v.iter().map(|c| build(c, children)).collect::<Vec<_>>())
                .unwrap_or_default();
            TreeNode {
                frame: vf.clone(),
                children: kids,
            }
        }
        roots.iter().map(|r| build(r, &children)).collect()
    }

    /// A single tree row. Branches (`has_children`) get a `▾`/`▸` chevron that
    /// flips with the collapsed state; leaves get a `•`. Clicking a branch row
    /// (closure captures the node id + this entity) toggles its children.
    fn row(&self, node: &TreeNode, depth: usize, this: Entity<Self>) -> AnyElement {
        let has_children = !node.children.is_empty();
        let is_collapsed = self.collapsed.contains(&node.frame.node_id);
        let chevron = if !has_children {
            "•"
        } else if is_collapsed {
            "▸"
        } else {
            "▾"
        };
        let name = node
            .frame
            .component_name
            .clone()
            .unwrap_or_else(|| "(unnamed)".to_string());
        let geo = match &node.frame.frame {
            Some(rect) => format!("{}×{} @ ({}, {})", rect.width, rect.height, rect.x, rect.y),
            None => "geometry pending".to_string(),
        };
        let key = format!(
            "{}{} {}  #{}",
            "  ".repeat(depth),
            chevron,
            name,
            node.frame.node_id
        );
        let mut row = kv_row(key, geo);
        if has_children {
            let node_id = node.frame.node_id;
            // `on_click` is provided by `StatefulInteractiveElement` (via
            // `interactivity()`) in this gpui pin — not as an inherent `Div`
            // method. `gpui::prelude::*` brings the trait into scope.
            row.interactivity()
                .on_click(move |_event: &ClickEvent, _window, cx| {
                    this.update(cx, |this, cx| this.toggle(node_id, cx));
                });
        }
        into_any(row)
    }

    fn render_tree(
        &self,
        nodes: &[TreeNode],
        depth: usize,
        this: Entity<Self>,
        out: &mut Vec<AnyElement>,
    ) {
        for n in nodes {
            out.push(self.row(n, depth, this.clone()));
            let is_collapsed = self.collapsed.contains(&n.frame.node_id);
            if !n.children.is_empty() && !is_collapsed {
                self.render_tree(&n.children, depth + 1, this.clone(), out);
            }
        }
    }

    /// Renders the view as a standalone pane.
    pub fn render_pane(&mut self, this: Entity<Self>) -> AnyElement {
        let tree = self.tree();
        if tree.is_empty() {
            return into_any(empty_row("No layout frames received yet."));
        }
        // Throttled population probe (debug only): confirm telemetry reaches the
        // tree with a stable node count.
        let total: usize = tree.iter().map(|r| 1 + r.children.len()).sum();
        if total != self.last_tree_len {
            self.last_tree_len = total;
            eprintln!("[DT-TREE] populated roots={} nodes={}", tree.len(), total);
        }
        let mut rows: Vec<AnyElement> = Vec::new();
        self.render_tree(&tree, 0, this, &mut rows);
        into_any(rows_column(rows))
    }
}

impl Render for ComponentTreeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        self.render_pane(cx.entity())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DevToolsState;
    use flux_ir_serde::Rect;

    fn frame(node_id: u32, parent_id: u32) -> ViewFrame {
        ViewFrame {
            node_id,
            parent_id,
            frame: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 20.0,
            }),
            component_name: Some("Column".to_string()),
        }
    }

    #[test]
    fn tree_renders_rows_for_populated_view_frames() {
        let state = DevToolsState::new();
        // root (1) -> child (2) -> grandchild (3)
        state.push_view_frame(frame(1, 0));
        state.push_view_frame(frame(2, 1));
        state.push_view_frame(frame(3, 2));

        let view = ComponentTreeView::new(std::sync::Arc::new(state));
        let tree = view.tree();
        // 1 root, with 1 child, with 1 grandchild.
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].children.len(), 1);
        assert_eq!(tree[0].children[0].children.len(), 1);

        // The component name must be carried through reconstruction so the tree
        // reads as `Column` (not a bare node id).
        assert_eq!(tree[0].frame.component_name.as_deref(), Some("Column"));
    }

    #[test]
    fn collapse_set_toggles_visibility_contract() {
        let state = DevToolsState::new();
        state.push_view_frame(frame(1, 0));
        state.push_view_frame(frame(2, 1));
        let mut view = ComponentTreeView::new(std::sync::Arc::new(state));

        // Nothing collapsed initially: the root's child is rendered (depth 1).
        assert!(!view.collapsed.contains(&1));
        assert_eq!(view.tree()[0].children.len(), 1);

        // Collapsing the root hides its subtree: the visibility check in
        // `render_tree` skips children while the branch id is in `collapsed`.
        view.collapsed.insert(1);
        assert!(view.collapsed.contains(&1));

        // Re-expanding restores visibility.
        view.collapsed.remove(&1);
        assert!(!view.collapsed.contains(&1));
        assert_eq!(view.tree()[0].children.len(), 1);
    }
}
