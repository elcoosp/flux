use flux_parser::Decl;
use flux_syntax::{DeclTag, Key, NodeId, NodeTag, Span};

/// Derives a stable [`NodeId`] from a node's structural position.
///
/// Delegates to the canonical [`flux_syntax::compute_node_id`] (see
/// `docs/adr/ir-node-id-bridge.md`) so the type checker and the IR produce
/// identical IDs for identical source constructs — this is what lets FLUX-018
/// lowering look up inferred types by `NodeId`. The contract (AGENTS.md §3.2)
/// specifies FNV-1a-32 over `(parent_id, kind, span, key)`; the canonical
/// implementation is exactly that (FNV-1a-32 over the canonical little-endian
/// layout, yielding a `u32`).
#[must_use]
pub(crate) fn compute_node_id(
    parent: NodeId,
    tag: impl NodeTag,
    span: Span,
    key: Option<Key>,
) -> NodeId {
    flux_syntax::compute_node_id(parent, tag, span, key)
}

/// Maps a surface declaration to its structural [`DeclTag`], matching the
/// discriminants the type checker has always used (see
/// `crates/flux-ir/src/lower/ids.rs`). The tags are stable across edits and
/// shared with lowering so `TypedAST::types` keys line up with the IR.
///
/// Wrapping the discriminant in [`DeclTag`] (rather than passing a bare `u8`)
/// is what guarantees the compiler rejects an expression tag where a
/// declaration tag is required — that is the whole point of the sealed
/// [`NodeTag`] trait introduced in `flux-syntax`.
#[must_use]
pub(crate) fn decl_tag(decl: &Decl) -> DeclTag {
    match decl {
        Decl::Use(_) => DeclTag(2),
        Decl::Component(_) => DeclTag(3),
        Decl::Fn(_) => DeclTag(4),
        Decl::Type(_) => DeclTag(5),
        Decl::Trait(_) => DeclTag(6),
        Decl::Capability(_) => DeclTag(7),
        Decl::Const(_) => DeclTag(8),
        _ => DeclTag(9),
    }
}
