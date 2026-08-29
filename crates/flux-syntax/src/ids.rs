//! Identifier aliases and source spans (Appendix C §C.1).

use serde::{Deserialize, Serialize};

/// FNV-1a-32 (Fowler–Noll–Vo) with the standard 32-bit offset basis and prime.
///
/// Chosen over a cryptographic hash (`blake3`, the historical choice) for node-ID
/// derivation because node IDs are a non-security content address: they need to be
/// deterministic and well-distributed across the `(parent, tag, span, key)` tuple,
/// not collision-resistant against an adversary. FNV-1a-32 is dependency-free,
/// has no per-process randomization (unlike `ahash`'s `AHasher`), and is markedly
/// faster than blake3 for the tiny fixed-size inputs here — which keeps
/// [`compute_node_id`] cheap on the per-save lowering hot path (FLUX-071).
///
/// The same primitive derives wire prop indices (`flux_ir::lower::prop_index_for_name`),
/// so both ID spaces now share one deterministic convention.
const FNV_OFFSET_BASIS: u32 = 0x811C_9DC5;
const FNV_PRIME: u32 = 0x0100_0193;

/// Folds `bytes` into a 32-bit FNV-1a digest.
#[must_use]
fn fnv1a32(bytes: &[u8]) -> u32 {
    let mut hash = FNV_OFFSET_BASIS;
    for &byte in bytes {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Stable identity of an IR node, derived from source structure.
pub type NodeId = u32;
/// Index into the host app's closure table.
pub type HandlerId = u32;
/// Index of a cell in the reactive signal graph.
pub type SignalId = u32;
/// Index of an effect owned by a component instance.
pub type EffectId = u32;
/// Interned component name.
pub type ComponentId = u32;
/// Interned string, resolved through a [`crate::StringTable`].
pub type StringId = u32;
/// Identity of a source file.
pub type FileId = u32;
/// Interned type.
pub type TypeId = u32;
/// Index of a prop field within a component's prop layout.
pub type PropIdx = u16;
/// Identity of a live component instance in the host app.
pub type InstanceId = u32;
/// Hash of a `ForEach` item key, used for keyed reconciliation.
pub type Key = u64;

/// A half-open byte range within a single source file.
///
/// `start` is inclusive and `end` is exclusive, matching the convention used by
/// Rust-style diagnostics and by Appendix C §C.1.
///
/// # Examples
///
/// ```
/// use flux_syntax::Span;
///
/// let span = Span::new(0, 10, 20);
/// assert!(span.contains(10));
/// assert!(!span.contains(20));
/// assert_eq!(span.len(), 10);
/// ```
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Span {
    /// Source file this span points into.
    pub file_id: FileId,
    /// Inclusive start byte offset.
    pub start: u32,
    /// Exclusive end byte offset.
    pub end: u32,
}

impl Span {
    /// Creates a span covering `start..end` in `file_id`.
    #[must_use]
    pub const fn new(file_id: FileId, start: u32, end: u32) -> Self {
        Self {
            file_id,
            start,
            end,
        }
    }

    /// Returns the length of the span in bytes, saturating at zero for
    /// malformed (inverted) spans.
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    /// Returns `true` when the span covers no bytes.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns `true` when `offset` lies within `start..end`.
    #[must_use]
    pub const fn contains(&self, offset: u32) -> bool {
        offset >= self.start && offset < self.end
    }

    /// Returns the smallest span covering both `self` and `other`.
    ///
    /// The file of `self` wins when the two spans come from different files;
    /// callers join spans only within a single file during parsing, so a
    /// mismatch means a bug upstream rather than a recoverable condition.
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        Self {
            file_id: self.file_id,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

/// High bit set on a declaration tag so its [`NodeTag::into_u8`] byte can
/// never overlap an [`ExprTag`] byte.
///
/// The two families map to disjoint `u8` ranges (`ExprTag` ∈ 0..=127,
/// `DeclTag` ∈ 128..=255), so a node-ID computed from `ExprTag(t)` is always
/// distinct from one computed from `DeclTag(t)` — even for the same `t`.
const DECL_TAG_FAMILY_BIT: u8 = 0x80;

/// A node-kind tag for an **expression** node.
///
/// Expression tags and declaration tags share the same underlying `u8`
/// discriminant space — the `NodeKind` wire tags defined in Appendix D §D.3 —
/// but mixing the two at a call site is a classic source of silent node-ID
/// collisions. [`NodeTag`] makes the family explicit so the compiler rejects
/// passing a declaration tag where an expression tag is required, and vice
/// versa.
///
/// The `u8` payload is public so downstream crates can build a tag directly
/// from a [`NodeKind`](crate::NodeKind) discriminant. Valid discriminants are
/// exactly the `NodeKind` tags (`Component` = 0 … `Screen` = 6). [`NodeTag::into_u8`]
/// returns the raw discriminant unchanged for `ExprTag`, so call sites that
/// wrap a `NodeKind` tag in `ExprTag` keep producing the same [`NodeId`] the
/// historical `kind_tag: u8` API produced — existing hashes stay stable.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct ExprTag(pub u8);

/// A node-kind tag for a **declaration** node.
///
/// See [`ExprTag`] for the rationale. Declaration tags name the structural
/// tree nodes (components, primitives, screens, routers) as opposed to the
/// expression-flow nodes tagged by [`ExprTag`]. Valid discriminants are the
/// `NodeKind` wire tags (Appendix D §D.3). [`NodeTag::into_u8`] sets
/// the declaration family bit (`0x80`) on the discriminant, so a declaration tag and an
/// expression tag carrying the same numeric value always hash to distinct
/// [`NodeId`]s — the families can never be confused at the ID level.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct DeclTag(pub u8);

/// A node-kind tag that can be folded into a stable [`NodeId`].
///
/// Implemented only by [`ExprTag`] and [`DeclTag`]; a private `Sealed`
/// supertrait keeps the set closed so external crates cannot invent a third
/// tag family and silently drift out of sync with the wire protocol.
pub trait NodeTag: sealed::Sealed {
    /// Returns the `u8` folded into the node-ID hash.
    ///
    /// The returned byte already encodes the tag family (declarations carry
    /// the `0x80` family bit), so two different families never collide even
    /// when their [`NodeKind`](crate::NodeKind) discriminant is identical.
    fn into_u8(self) -> u8;
}

impl NodeTag for ExprTag {
    fn into_u8(self) -> u8 {
        self.0
    }
}

impl NodeTag for DeclTag {
    fn into_u8(self) -> u8 {
        self.0 | DECL_TAG_FAMILY_BIT
    }
}

/// Closes [`NodeTag`] so only this crate can name new tag families.
///
/// Private on purpose: the wire protocol (Appendix D) defines a fixed tag
/// space, and allowing arbitrary external `NodeTag` implementations would let
/// a crate produce node IDs that disagree with every other toolchain member.
mod sealed {
    /// Marker supertrait required by [`crate::NodeTag`].
    ///
    /// The trait is `pub` (so `private_bounds` is satisfied) but lives in a
    /// private module, so it is unnameable from outside the crate. That is
    /// exactly what enforces the sealed-trait contract: only `flux-syntax`
    /// can name `Sealed`, therefore only it can implement [`crate::NodeTag`].
    pub trait Sealed {}

    impl Sealed for crate::ExprTag {}
    impl Sealed for crate::DeclTag {}
}

/// Derives the stable [`NodeId`] for a node — the **single source of truth**
/// for node-ID computation across the whole toolchain.
///
/// This is the canonical implementation that every crate delegates to (see
/// `docs/adr/ir-node-id-bridge.md`). It mirrors the layout historically used
/// by `flux-ir` so existing IR/differ/wire hashes stay stable:
///
/// ```text
/// FNV1A32( parent
///        | kind_tag
///        | span.file_id
///        | span.start
///        | span.end
///        | key-or-0xFF*8 )  -> u32
/// ```
///
/// The digest is FNV-1a-32 (FLUX-071): deterministic, dependency-free, and far
/// cheaper than the historical `blake3` for the tiny fixed-size inputs here,
/// while keeping identical collision-resistance for this non-security key space.
/// It is the same primitive used for wire prop indices, so both ID spaces share
/// one convention. Node IDs are stable across edits: a sibling insertion or
/// handler-body edit does not shift any other node's ID, which is what makes
/// keyed reconciliation and state preservation work.
///
/// `tag` is any [`NodeTag`] — either [`ExprTag`] or [`DeclTag`] — so an
/// expression tag and a declaration tag that happen to carry the same numeric
/// discriminant can never be swapped by accident, and they always hash to
/// distinct IDs.
///
/// # Examples
///
/// ```rust
/// use flux_syntax::{compute_node_id, DeclTag, ExprTag, Span};
///
/// let span = Span::new(1, 0, 10);
/// // Same numeric discriminant, different tag families -> distinct IDs.
/// let expr = compute_node_id(0, ExprTag(7), span, None);
/// let decl = compute_node_id(0, DeclTag(7), span, None);
/// assert_ne!(expr, decl);
/// // Identical inputs always produce the identical ID.
/// assert_eq!(expr, compute_node_id(0, ExprTag(7), span, None));
/// ```
#[must_use]
pub fn compute_node_id(parent: NodeId, tag: impl NodeTag, span: Span, key: Option<Key>) -> NodeId {
    let mut buf = [0_u8; 25];
    buf[0..4].copy_from_slice(&parent.to_le_bytes());
    buf[4] = tag.into_u8();
    buf[5..9].copy_from_slice(&span.file_id.to_le_bytes());
    buf[9..13].copy_from_slice(&span.start.to_le_bytes());
    buf[13..17].copy_from_slice(&span.end.to_le_bytes());
    match key {
        Some(k) => buf[17..25].copy_from_slice(&k.to_le_bytes()),
        None => buf[17..25].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
    };
    fnv1a32(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(start: u32, end: u32) -> Span {
        Span::new(1, start, end)
    }

    #[test]
    fn fnv1a32_is_deterministic() {
        // Same input must map to the same ID across calls (the stable-ID contract;
        // FNV is not randomized per process, unlike ahasher).
        let a = compute_node_id(0, ExprTag(7), span(0, 10), None);
        let b = compute_node_id(0, ExprTag(7), span(0, 10), None);
        assert_eq!(a, b);
    }

    #[test]
    fn expr_and_decl_families_are_disjoint() {
        // The DeclTag family bit (0x80) must keep an ExprTag and a DeclTag with the
        // same numeric discriminant producing distinct IDs.
        let expr = compute_node_id(0, ExprTag(7), span(0, 10), None);
        let decl = compute_node_id(0, DeclTag(7), span(0, 10), None);
        assert_ne!(expr, decl);
    }

    #[test]
    fn every_field_independently_affects_id() {
        // Changing parent, tag, file_id, start, end, or key must each change the ID.
        let base = compute_node_id(0, ExprTag(7), span(0, 10), None);
        assert_ne!(
            base,
            compute_node_id(1, ExprTag(7), span(0, 10), None),
            "parent change must change id"
        );
        assert_ne!(
            base,
            compute_node_id(0, ExprTag(8), span(0, 10), None),
            "tag change must change id"
        );
        assert_ne!(
            compute_node_id(0, ExprTag(7), Span::new(2, 0, 10), None),
            base,
            "file_id change must change id"
        );
        assert_ne!(
            base,
            compute_node_id(0, ExprTag(7), span(5, 10), None),
            "start change must change id"
        );
        assert_ne!(
            base,
            compute_node_id(0, ExprTag(7), span(0, 20), None),
            "end change must change id"
        );
        assert_ne!(
            base,
            compute_node_id(0, ExprTag(7), span(0, 10), Some(99)),
            "key change must change id"
        );
    }

    #[test]
    fn none_and_some_zero_are_distinct() {
        // The key sentinel differs between the no-key (0xFF*8) and key=0 cases.
        let none = compute_node_id(0, ExprTag(7), span(0, 10), None);
        let some_zero = compute_node_id(0, ExprTag(7), span(0, 10), Some(0));
        assert_ne!(none, some_zero);
    }
}
