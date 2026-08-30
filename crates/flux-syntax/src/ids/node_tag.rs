#![allow(clippy::module_inception)] // submodule named for its single responsibility (NodeTag lives in 'node_tag').
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
