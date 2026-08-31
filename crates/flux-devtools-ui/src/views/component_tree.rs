//! Component tree view (spec §5.3): the shadow-tree node hierarchy.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::{
    AnyElement, ClickEvent, Context, ElementId, Entity, Focusable, InteractiveElement, IntoElement,
    Render, Window, prelude::*, px,
};
use gpui_component::ActiveTheme as _;
use gpui_component::input::{Input, InputEvent, InputState};

use crate::row::{empty_row, into_any, kv_row, rows_column};
use crate::state::DevToolsState;
use crate::time_travel::ViewFrame;
use gpui_component::button::Button;

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
    /// Live search box entity (lazily created on first render, which owns the
    /// window/`cx` needed to construct an `InputState`).
    search: Option<Entity<InputState>>,
    /// The current search query; when non-empty the tree is filtered to nodes
    /// whose component name matches (case-insensitive), keeping any branch that
    /// has a matching descendant.
    query: String,
    /// Retained subscription so the search box input stays observed.
    _search_sub: Option<gpui::Subscription>,
    /// Last tree node count logged, to throttle the population probe.
    last_tree_len: usize,
}

impl ComponentTreeView {
    /// Creates the view bound to the shared state.
    pub fn new(state: Arc<DevToolsState>) -> Self {
        Self {
            state,
            collapsed: HashSet::new(),
            search: None,
            query: String::new(),
            _search_sub: None,
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

    /// Focuses the component-tree search box. Safe to call before the search
    /// input has been lazily created: it triggers one render (which builds the
    /// `InputState`) and then focuses it on the following frame.
    pub fn focus_search(&self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.search.is_none() {
            // Force the search box into existence, then focus next frame.
            cx.notify();
            let this = cx.entity();
            window.defer(cx, move |window, cx| {
                this.update(cx, |this, cx| {
                    if let Some(input) = this.search.as_ref() {
                        input.focus_handle(cx).focus(window, cx);
                    }
                });
            });
        } else if let Some(input) = self.search.as_ref() {
            input.focus_handle(cx).focus(window, cx);
        }
    }

    /// Collapses or expands every branch in the tree (used by the header
    /// "Toggle all" button). If any branch is currently expanded it collapses
    /// all of them; otherwise it expands all.
    fn toggle_all(&mut self, cx: &mut Context<'_, Self>) {
        eprintln!("[DT-COLLAPSE] toggle_all called");
        let tree = self.tree();
        let mut branches: Vec<u32> = Vec::new();
        fn collect(nodes: &[TreeNode], out: &mut Vec<u32>) {
            for n in nodes {
                if !n.children.is_empty() {
                    out.push(n.frame.node_id);
                    collect(&n.children, out);
                }
            }
        }
        collect(&tree, &mut branches);
        let any_expanded = branches.iter().any(|id| !self.collapsed.contains(id));
        for id in &branches {
            if any_expanded {
                self.collapsed.insert(*id);
            } else {
                self.collapsed.remove(id);
            }
        }
        eprintln!(
            "[DT-COLLAPSE] toggle_all -> {} branches, collapsed={any_expanded}",
            branches.len()
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
        let built: Vec<TreeNode> = roots.iter().map(|r| build(r, &children)).collect();
        if self.query.is_empty() {
            return built;
        }
        // Filter: keep a node if its name matches, or any descendant matches.
        let q = self.query.to_lowercase();
        fn matches(node: &TreeNode, q: &str) -> bool {
            let name_match = node
                .frame
                .component_name
                .as_deref()
                .map(|n| n.to_lowercase().contains(q))
                .unwrap_or(false);
            let child_match = node.children.iter().any(|c| matches(c, q));
            name_match || child_match
        }
        fn prune(nodes: &[TreeNode], q: &str) -> Vec<TreeNode> {
            nodes
                .iter()
                .filter(|n| matches(n, q))
                .map(|n| TreeNode {
                    frame: n.frame.clone(),
                    children: prune(&n.children, q),
                })
                .collect()
        }
        prune(&built, &q)
    }

    /// A single tree row. Branches (`has_children`) get a `▾`/`▸` chevron that
    /// flips with the collapsed state; leaves get a `•`. Clicking a branch row
    /// (closure captures the node id + this entity) toggles its children. Hover
    /// tints the row with the theme's `muted` color.
    fn row(
        &self,
        node: &TreeNode,
        depth: usize,
        this: Entity<Self>,
        cx: &Context<'_, Self>,
    ) -> AnyElement {
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
            Some(rect) => format!(" [{:.2}×{:.2}]", rect.width, rect.height,),
            None => " [pending]".to_string(),
        };
        let key = format!(
            "{}{} {}{}  #{}",
            "  ".repeat(depth),
            chevron,
            name,
            geo,
            node.frame.node_id,
        );
        let node_id = node.frame.node_id;
        let row = kv_row(key, "").id(ElementId::from(format!("ctrow-{node_id}")));
        let mut row = row;
        if has_children {
            // Give the row a stable element id so it becomes a `Stateful<Div>`,
            // which implements `StatefulInteractiveElement` and thus has a
            // working `.on_click(...)`. (Calling `Div::interactivity().on_click`
            // discards the listener and never fires — the `Button` component
            // uses this `Stateful` pattern, which is what actually works.)
            row = row.on_click(move |_event: &ClickEvent, _window, cx| {
                this.update(cx, |this, cx| this.toggle(node_id, cx));
            });
        }
        let hover_bg = cx.theme().muted;
        into_any(row.hover(|s| s.bg(hover_bg)))
    }

    fn render_tree(
        &self,
        nodes: &[TreeNode],
        depth: usize,
        this: Entity<Self>,
        cx: &Context<'_, Self>,
        out: &mut Vec<AnyElement>,
    ) {
        for n in nodes {
            out.push(self.row(n, depth, this.clone(), cx));
            let is_collapsed = self.collapsed.contains(&n.frame.node_id);
            if !n.children.is_empty() && !is_collapsed {
                self.render_tree(&n.children, depth + 1, this.clone(), cx, out);
            }
        }
    }

    /// Renders the view as a standalone pane.
    pub fn render_pane(
        &mut self,
        window: &mut Window,
        this: Entity<Self>,
        cx: &mut Context<'_, Self>,
    ) -> AnyElement {
        // Lazily build the search box on first render (we need `window`/`cx`
        // here, which `new` doesn't have). Subscribe once so typing repaints;
        // the query is read live from the input below.
        if self.search.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search components…"));
            self._search_sub = Some(cx.subscribe(
                &input,
                move |_this: &mut Self, _entity: Entity<InputState>, _ev: &InputEvent, cx| {
                    // The input updates its own state on change; repaint so the
                    // live query read below reflects the typed text.
                    cx.notify();
                },
            ));
            self.search = Some(input);
        }
        // Read the live query from the input (authoritative source of truth).
        if let Some(input) = self.search.as_ref() {
            self.query = input.read(cx).value().to_string();
        }

        let tree = self.tree();
        if tree.is_empty() {
            return into_any(empty_row(if self.query.is_empty() {
                "No layout frames received yet."
            } else {
                "No nodes match your search."
            }));
        }
        // Throttled population probe (debug only): confirm telemetry reaches the
        // tree with a stable node count.
        let total: usize = tree.iter().map(|r| 1 + r.children.len()).sum();
        if total != self.last_tree_len {
            self.last_tree_len = total;
            eprintln!("[DT-TREE] populated roots={} nodes={}", tree.len(), total);
        }
        let mut rows: Vec<AnyElement> = Vec::new();
        self.render_tree(&tree, 0, this.clone(), cx, &mut rows);
        // Search box (debounced filter by component name) on top, then the
        // "Toggle all" button. Both sit in the header row above the tree.
        let search_box = gpui::div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .min_w(px(0.))
            .child(Input::new(self.search.as_ref().unwrap()));
        // Header with a real button (proves the click path works independent of
        // row-level hit-testing) that collapses/expands every branch at once.
        let header = gpui::div().flex().flex_row().items_center().child(
            Button::new("ct-toggle-all")
                .label("Toggle all")
                .ml(px(8.))
                .px(px(12.))
                .py(px(6.))
                .h_auto()
                .text_sm()
                .on_click(move |_event: &ClickEvent, _window, cx| {
                    // Defer the state mutation: the click handler runs while
                    // this entity is already mid-update, so a direct
                    // re-entrant `update` would panic and blank the pane
                    // (the button "disappears"). `App::defer` runs it after
                    // the current event settles — the safe gpui pattern.
                    let this = this.clone();
                    cx.defer(move |cx| {
                        this.update(cx, |this, cx| this.toggle_all(cx));
                    });
                }),
        );
        let mut content: Vec<AnyElement> = Vec::new();
        content.push(into_any(search_box));
        content.push(into_any(header));
        content.push(into_any(rows_column(rows)));
        // Background color and overflow clipping for clean pane isolation.
        gpui::div()
            .flex()
            .flex_col()
            .flex_1()
            .overflow_hidden()
            .bg(cx.theme().background)
            .child(rows_column(content))
            .into_any_element()
    }
}

impl Render for ComponentTreeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        self.render_pane(window, cx.entity(), cx)
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

    fn frame_named(node_id: u32, parent_id: u32, name: &str) -> ViewFrame {
        let mut f = frame(node_id, parent_id);
        f.component_name = Some(name.to_string());
        f
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
    fn search_query_filters_by_component_name() {
        let state = DevToolsState::new();
        // root Column (1) -> Button (2) -> Text (3); plus a sibling Image (4).
        state.push_view_frame(frame_named(1, 0, "Column"));
        state.push_view_frame(frame_named(2, 1, "Button"));
        state.push_view_frame(frame_named(3, 2, "Text"));
        state.push_view_frame(frame_named(4, 1, "Image"));
        let mut view = ComponentTreeView::new(std::sync::Arc::new(state));

        // No query: full tree present.
        assert_eq!(view.tree().len(), 1);
        assert_eq!(view.tree()[0].children.len(), 2);

        // Match "button": the Column root stays (has a matching descendant) and
        // the Button branch is kept; the Image sibling is pruned.
        view.query = "button".to_string();
        let filtered = view.tree();
        assert_eq!(filtered.len(), 1);
        let children = &filtered[0].children;
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].frame.component_name.as_deref(), Some("Button"));

        // Match "image": Column root kept, only the Image branch remains.
        view.query = "image".to_string();
        let filtered = view.tree();
        assert_eq!(filtered[0].children.len(), 1);
        assert_eq!(
            filtered[0].children[0].frame.component_name.as_deref(),
            Some("Image")
        );

        // Match with no descendant hit prunes everything under the root.
        view.query = "zzz".to_string();
        assert!(view.tree().is_empty());
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
