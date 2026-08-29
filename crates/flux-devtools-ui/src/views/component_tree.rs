//! Component tree view (spec §5.3): the shadow-tree node hierarchy.

use std::{collections::HashMap, sync::Arc};

use gpui::{AnyElement, Context, Div, IntoElement, ParentElement, Render, Styled, Window, div, px};

use crate::row::into_any;
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

    fn row(&self, node: &TreeNode, depth: usize) -> Div {
        let has_children = !node.children.is_empty();
        let marker = if has_children { "▾" } else { "•" };
        let label = match &node.frame.frame {
            Some(rect) => format!(
                "node #{}  {}×{} @ ({}, {})",
                node.frame.node_id, rect.width, rect.height, rect.x, rect.y
            ),
            None => format!("node #{}  (geometry pending)", node.frame.node_id),
        };
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .pl(px(12.0 * depth as f32 + 8.0))
            .py(px(3.))
            .child(
                div()
                    .w(px(12.))
                    .text_color(gpui::white().opacity(0.5))
                    .child(marker),
            )
            .child(div().text_color(gpui::white().opacity(0.85)).child(label))
    }

    fn render_tree(&self, nodes: &[TreeNode], depth: usize, out: &mut Vec<AnyElement>) {
        for n in nodes {
            out.push(into_any(self.row(n, depth)));
            if !n.children.is_empty() {
                self.render_tree(&n.children, depth + 1, out);
            }
        }
    }

    /// Renders the view as a standalone pane.
    pub fn render_pane(&self, _cx: &Context<'_, Self>) -> impl IntoElement {
        let tree = self.tree();
        if tree.is_empty() {
            return div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .h_full()
                .text_color(gpui::white().opacity(0.45))
                .child("No layout frames received yet.");
        }
        let mut rows: Vec<AnyElement> = Vec::new();
        self.render_tree(&tree, 0, &mut rows);
        div().flex().flex_col().children(rows)
    }
}

impl Render for ComponentTreeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        self.render_pane(cx)
    }
}
