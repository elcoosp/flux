//! Stable node-ID derivation (ADR-0013).
//!
//! Node IDs are `u32` digests of `(parent_id, parent node kind, source span,
//! optional key)`. They are stable across edits: inserting a sibling or editing
//! a handler body does not shift any other node's ID, which is what makes
//! keyed reconciliation and state preservation work.
//!
//! The canonical derivation now lives in `flux-syntax` (see
//! `docs/adr/ir-node-id-bridge.md`); this crate delegates to it so the IR and
//! the type checker produce identical IDs for identical source constructs.

use flux_syntax::ExprTag;
use flux_syntax::Key;
use flux_syntax::NodeId;
use flux_syntax::NodeKind;
use flux_syntax::Span;

/// Derives the stable [`NodeId`] for a node.
///
/// Thin wrapper over [`flux_syntax::compute_node_id`] that accepts the
/// [`NodeKind`] enum (converting it to its `u8` tag) so the IR's public API is
/// unchanged. The byte layout is identical to the canonical function, so all
/// previously-computed IR/differ/wire hashes remain stable.
///
/// # Examples
///
/// ```
/// use flux_ir::compute_node_id;
/// use flux_syntax::{NodeKind, Span};
///
/// let span = Span::new(1, 0, 10);
/// let a = compute_node_id(0, NodeKind::Component, span, None);
/// let b = compute_node_id(1, NodeKind::Primitive, span, None);
/// assert_ne!(a, b);
/// // Identical inputs always produce the identical ID.
/// assert_eq!(a, compute_node_id(0, NodeKind::Component, span, None));
/// ```
///
/// # Panics
///
/// Does not panic: every input is fed to the hasher as fixed-width bytes, so
/// there is no fallible path.
#[must_use]
pub fn compute_node_id(parent: NodeId, kind: NodeKind, span: Span, key: Option<Key>) -> NodeId {
    // `ExprTag::into_u8` returns the `NodeKind` discriminant unchanged, so this
    // is byte-identical to the historical `compute_node_id(parent, kind.tag(),
    // …)` call — the canonical `compute_node_id` now requires `impl NodeTag`
    // (ADR/issue 3a).
    flux_syntax::compute_node_id(parent, ExprTag(kind.tag()), span, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_inputs_give_identical_ids() {
        let span = Span::new(3, 40, 52);
        assert_eq!(
            compute_node_id(0, NodeKind::Component, span, None),
            compute_node_id(0, NodeKind::Component, span, None),
        );
    }

    #[test]
    fn different_kind_changes_id() {
        let span = Span::new(3, 40, 52);
        assert_ne!(
            compute_node_id(0, NodeKind::Component, span, None),
            compute_node_id(0, NodeKind::Primitive, span, None),
        );
    }

    #[test]
    fn different_parent_changes_id() {
        let span = Span::new(3, 40, 52);
        assert_ne!(
            compute_node_id(0, NodeKind::Component, span, None),
            compute_node_id(7, NodeKind::Component, span, None),
        );
    }

    #[test]
    fn different_span_changes_id() {
        let a = Span::new(3, 40, 52);
        let b = Span::new(3, 40, 53);
        assert_ne!(
            compute_node_id(0, NodeKind::Component, a, None),
            compute_node_id(0, NodeKind::Component, b, None),
        );
    }

    #[test]
    fn key_distinguishes_for_each_children() {
        let span = Span::new(3, 40, 52);
        let base = compute_node_id(0, NodeKind::Component, span, None);
        let with_key = compute_node_id(0, NodeKind::Component, span, Some(99));
        assert_ne!(base, with_key);
        // Two children of a ForEach at the same source span differ by key only.
        assert_ne!(
            compute_node_id(0, NodeKind::Component, span, Some(1)),
            compute_node_id(0, NodeKind::Component, span, Some(2)),
        );
    }

    #[test]
    fn key_presence_is_part_of_the_id() {
        let span = Span::new(3, 40, 52);
        assert_ne!(
            compute_node_id(0, NodeKind::Component, span, None),
            compute_node_id(0, NodeKind::Component, span, Some(0)),
        );
    }

    // Bridge test (ADR-0027): `flux-ir`'s derivation must be byte-identical to
    // the canonical `flux-syntax::compute_node_id` so lowering can join IDs.
    #[test]
    fn delegates_to_flux_syntax_canonical() {
        let span = Span::new(3, 40, 52);
        for kind in [NodeKind::Component, NodeKind::Primitive, NodeKind::ForEach] {
            assert_eq!(
                compute_node_id(0, kind, span, None),
                flux_syntax::compute_node_id(0, ExprTag(kind.tag()), span, None),
            );
            assert_eq!(
                compute_node_id(7, kind, span, Some(99)),
                flux_syntax::compute_node_id(7, ExprTag(kind.tag()), span, Some(99)),
            );
        }
    }
}
