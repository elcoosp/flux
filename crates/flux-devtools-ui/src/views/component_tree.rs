//! Component tree view (spec §5.3): the shadow-tree node hierarchy.

use std::{collections::HashMap, sync::Arc};

use gpui::{AnyElement, Context, IntoElement, Render, Window};

use crate::row::{empty_row, into_any, kv_row, rows_column};
use crate::state::DevToolsState;
use crate::time_travel::ViewFrame;

/// A node in the reconstructed component tree (parent links resolved).
struct TreeNode {
    frame: ViewFrame,
    children: Vec<TreeNode>,
}

/// Renders the live component tree as an indented, collapsible-style hierarchy
/// (depth → left padding, chevrons mark branches) instead of a flat list.
pub struct ComponentTreeView {
    state: Arc<DevToolsState>,
}

impl ComponentTreeView {
    /// Creates the view bound to the shared state.
    pub fn new(state: Arc<DevToolsState>) -> Self {
        Self { state }
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
        let known: std::collections::HashSet<u32> = ids.iter().copied().collect();
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

    /// A single tree row, rendered with the same `kv_row` primitive the other
    /// (working) panes use: key = indented marker + node id, value = geometry.
    fn row(&self, node: &TreeNode, depth: usize) -> AnyElement {
        let label = match &node.frame.frame {
            Some(rect) => format!(
                "{}×{} @ ({}, {})",
                rect.width, rect.height, rect.x, rect.y
            ),
            None => "(geometry pending)".to_string(),
        };
        let marker = if node.children.is_empty() { "•" } else { "▾" };
        let key = format!("{}{} node #{}", "  ".repeat(depth), marker, node.frame.node_id);
        into_any(kv_row(key, label))
    }

    fn render_tree(&self, nodes: &[TreeNode], depth: usize, out: &mut Vec<AnyElement>) {
        for n in nodes {
            out.push(self.row(n, depth));
            if !n.children.is_empty() {
                self.render_tree(&n.children, depth + 1, out);
            }
        }
    }

    /// Renders the view as a standalone pane.
    pub fn render_pane(&self, _cx: &Context<'_, Self>) -> AnyElement {
        let tree = self.tree();
        if tree.is_empty() {
            return into_any(empty_row("No layout frames received yet."));
        }
        let mut rows: Vec<AnyElement> = Vec::new();
        self.render_tree(&tree, 0, &mut rows);
        into_any(rows_column(rows))
    }
}

impl Render for ComponentTreeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        self.render_pane(cx)
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

        // The render path must produce one row per node (3) without panicking
        // and must not collapse to the empty-state.
        let mut rows: Vec<AnyElement> = Vec::new();
        view.render_tree(&tree, 0, &mut rows);
        assert_eq!(rows.len(), 3, "each node must yield one rendered row");
    }
}
