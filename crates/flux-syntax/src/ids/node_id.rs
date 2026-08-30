use super::fnv::*;
use super::node_tag::NodeTag;
use super::span::Span;

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

/// A node-kind tag used purely as the *content* family discriminator for
/// [`content_addressed_id`].
///
/// Distinct from the [`NodeTag`] families: it carries no declaration/expression
/// split (the structural `NodeKind` already disambiguates) and instead encodes
/// the wire `NodeKind` discriminant plus the node's `component_id` so that two
/// structurally-identical-but-differently-named primitives (e.g. `Text` vs
/// `Button`) address differently. See [`content_addressed_id`].
#[derive(Clone, Copy, Debug)]
struct ContentTag {
    kind: u8,
    component_id: u32,
}

impl ContentTag {
    /// Folds `kind` and `component_id` into a single `u8`-wide tag byte so the
    /// hash input stays fixed-width and cheap to build.
    fn as_byte(self) -> u8 {
        // `kind` is a 3-bit wire discriminant; `component_id` is folded in via a
        // small rotation so common low ids still perturb the tag meaningfully.
        (self.kind ^ (self.component_id.wrapping_mul(0x9E) as u8)) & 0x7F
    }
}

/// Derives a **content-addressed** [`NodeId`] for a node.
///
/// Unlike [`compute_node_id`] (which folds the source `Span` and therefore
/// flips when text *above* the node shifts), this id is derived purely from the
/// node's *structural content* and its already-content-addressed parent:
///
/// ```text
/// FNV1A32( parent                // content-addressed parent id
///        | content_tag(kind, component_id)
///        | props_hash  (u64 LE)  // prop *values*, not source offsets
///        | children_hash (u64 LE)// recursive child content ids, folded
///        | key-or-0xFF*8 )  -> u32
/// ```
///
/// Because children contribute their *content-addressed* ids (recursively), a
/// subtree whose source moved but whose content is identical keeps its id. That
/// is what lets a node survive a text-above edit at hot reload instead of being
/// torn down and rebuilt (FLUX-074, item A). The same FNV-1a-32 primitive and
/// seed as [`compute_node_id`] is used, so the two id spaces stay in the same
/// deterministic family.
///
/// `props_hash` and `children_hash` are the arena-stored content digests (see
/// `IRArena::props_hash` / `children_hash`); `children_hash` must already embed
/// the content-addressed ids of the node's children for the recursion to hold.
///
/// # Examples
///
/// ```rust
/// use flux_syntax::{content_addressed_id, Key};
///
/// // Same structural content -> identical id regardless of where it sits in source.
/// let a = content_addressed_id(0, 1, 7, 0x1111_2222_3333_4444, 0x5555_6666_7777_8888, None);
/// let b = content_addressed_id(0, 1, 7, 0x1111_2222_3333_4444, 0x5555_6666_7777_8888, None);
/// assert_eq!(a, b);
/// // Different props -> different id.
/// let c = content_addressed_id(0, 1, 7, 0x9999_9999_9999_9999, 0x5555_6666_7777_8888, None);
/// assert_ne!(a, c);
/// ```
#[must_use]
pub fn content_addressed_id(
    parent: NodeId,
    kind: u8,
    component_id: u32,
    props_hash: u64,
    children_hash: u64,
    key: Option<Key>,
) -> NodeId {
    let mut buf = [0_u8; 29];
    buf[0..4].copy_from_slice(&parent.to_le_bytes());
    buf[4] = ContentTag { kind, component_id }.as_byte();
    buf[5..13].copy_from_slice(&props_hash.to_le_bytes());
    buf[13..21].copy_from_slice(&children_hash.to_le_bytes());
    match key {
        Some(k) => buf[21..29].copy_from_slice(&k.to_le_bytes()),
        None => buf[21..29].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
    };
    fnv1a32(&buf)
}
