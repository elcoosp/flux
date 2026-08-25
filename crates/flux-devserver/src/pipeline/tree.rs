//! Arena helpers for the compile pipeline: root materialisation and multi-file
//! arena merging (FLUX-019).

use std::path::Path;

use flux_ir::IRArena;
use flux_syntax::{Child, NodeId, NodeKind, NodeRef, Props, Span};

/// Renders `path` relative to `root` when possible, for host-visible diagnostics.
pub(crate) fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

/// Packs every node of `extra` into `base`, preserving node identity.
///
/// Node IDs are content-derived from `(parent, kind, span, key)` (ADR-0013), so
/// two distinct files cannot collide: their spans carry different `file_id`s.
pub(crate) fn merge_arenas(mut base: IRArena, extra: &IRArena) -> IRArena {
    for id in extra.all_ids() {
        if let Some(view) = extra.get(id) {
            base.pack(flux_ir::Node {
                id: view.id(),
                kind: view.kind(),
                component_id: view.component_id(),
                props: view.props(),
                children: view.children(),
                handlers: view.handlers(),
                span: view.span(),
            });
        }
    }
    base
}

/// Materialises the tree root shipped in an `Init` frame.
///
/// A node is a root when no other node lists it as a child. A project with
/// exactly one root ships that node directly. A project with zero or several
/// roots (several top-level components — the common case) ships a synthetic
/// `Component` wrapper whose children are those roots: §D.12.2 carries exactly
/// one root node, and the host renders the wrapper's children in declaration
/// order.
pub(crate) fn root_node(arena: &IRArena) -> NodeRef {
    let roots = root_ids(arena);
    if let [only] = roots.as_slice() {
        if let Some(view) = arena.get(*only) {
            return NodeRef {
                id: view.id(),
                kind: view.kind(),
                component_id: view.component_id(),
                props: view.props(),
                children: view.children(),
                handlers: view.handlers(),
                span: view.span(),
            };
        }
    }
    let span = Span::new(0, 0, 0);
    NodeRef {
        id: flux_ir::compute_node_id(0, NodeKind::Component, span, None),
        kind: NodeKind::Component,
        component_id: flux_syntax::ComponentId::from(0u32),
        props: Props::default(),
        children: roots.into_iter().map(Child::Node).collect(),
        handlers: Vec::new(),
        span,
    }
}

/// Every node in `arena` that no other node lists as a child, in pack order.
fn root_ids(arena: &IRArena) -> Vec<NodeId> {
    let mut referenced: Vec<NodeId> = Vec::new();
    for id in arena.all_ids() {
        if let Some(view) = arena.get(id) {
            referenced.extend(view.children().iter().flat_map(Child::node_ids));
        }
    }
    arena
        .all_ids()
        .filter(|id| !referenced.contains(id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_arena_yields_synthetic_root_with_no_children() {
        let root = root_node(&IRArena::default());
        assert_eq!(root.kind, NodeKind::Component);
        assert!(root.children.is_empty());
    }

    #[test]
    fn display_path_is_relative_to_root() {
        assert_eq!(
            display_path(Path::new("/tmp/app"), Path::new("/tmp/app/src/a.flux")),
            "src/a.flux"
        );
    }

    #[test]
    fn display_path_falls_back_to_absolute_when_outside_root() {
        assert_eq!(
            display_path(Path::new("/tmp/app"), Path::new("/other/a.flux")),
            "/other/a.flux"
        );
    }
}
