//! Identifier aliases and source spans (Appendix C §C.1).

use serde::{Deserialize, Serialize};

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

/// Derives the stable [`NodeId`] for a node — the **single source of truth**
/// for node-ID computation across the whole toolchain.
///
/// This is the canonical implementation that every crate delegates to (see
/// `docs/adr/ir-node-id-bridge.md`). It mirrors the layout historically used
/// by `flux-ir` so existing IR/differ/wire hashes stay stable:
///
/// ```text
/// BLAKE3( parent
///       | kind_tag
///       | span.file_id
///       | span.start
///       | span.end
///       | key-or-0xFF*8 )  -> truncate to 32 bits
/// ```
///
/// The digest is BLAKE3 (already a `flux-syntax` dependency) truncated to 32
/// bits, consistent with every other content address in Flux (prop hash,
/// closure hash, wire interning). Node IDs are stable across edits: a sibling
/// insertion or handler-body edit does not shift any other node's ID, which is
/// what makes keyed reconciliation and state preservation work.
///
/// # Examples
///
/// ```
/// use flux_syntax::{compute_node_id, Key, Span};
///
/// let span = Span::new(1, 0, 10);
/// let a = compute_node_id(0, 7, span, None);
/// let b = compute_node_id(1, 7, span, None);
/// assert_ne!(a, b);
/// // Identical inputs always produce the identical ID.
/// assert_eq!(a, compute_node_id(0, 7, span, None));
/// ```
#[must_use]
pub fn compute_node_id(parent: NodeId, kind_tag: u8, span: Span, key: Option<Key>) -> NodeId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&parent.to_le_bytes());
    hasher.update(&[kind_tag]);
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
