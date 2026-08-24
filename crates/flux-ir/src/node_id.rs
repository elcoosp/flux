//! Stable node-ID derivation (ADR-0013).
//!
//! Node IDs are `u32` digests of `(parent_id, parent node kind, source span,
//! optional key)`. They are stable across edits: inserting a sibling or editing
//! a handler body does not shift any other node's ID, which is what makes
//! keyed reconciliation and state preservation work.
//!
//! The digest is BLAKE3 over a canonical little-endian byte layout, then
//! truncated to 32 bits. BLAKE3 is already a workspace dependency (via
//! `flux-syntax`) and is used for every other content address in Flux, so the
//! IR stays consistent with the wire protocol and the prop hash.

use blake3::Hasher;
use flux_syntax::NodeKind;
use flux_syntax::{Key, NodeId, Span};

/// Derives the stable [`NodeId`] for a node.
///
/// # Arguments
///
/// * `parent` — the ID of the enclosing node (`0` for a tree root).
/// * `kind` — the [`NodeKind`] of the node being identified.
/// * `span` — the source span this node was lowered from.
/// * `key` — the `ForEach` iteration key, when the node is a dynamically
///   generated child; `None` for statically-known nodes.
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
    let mut hasher = Hasher::new();
    hasher.update(&parent.to_le_bytes());
    hasher.update(&[kind.tag()]);
    hasher.update(&span.file_id.to_le_bytes());
    hasher.update(&span.start.to_le_bytes());
    hasher.update(&span.end.to_le_bytes());
    match key {
        Some(k) => hasher.update(&k.to_le_bytes()),
        None => hasher.update(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
    };
    let mut digest = [0_u8; 4];
    digest.copy_from_slice(&hasher.finalize().as_bytes()[..4]);
    u32::from_le_bytes(digest)
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
}
