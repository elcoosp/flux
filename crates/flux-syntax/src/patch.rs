//! Patches and closure references — the unit of hot-swap
//! (Appendix C §C.1, Appendix D §D.2).

use core::ops::Range;

use crate::ids::{HandlerId, NodeId, PropIdx, SignalId, Span};
use crate::node::NodeRef;
use crate::value::Value;

/// A single structural or behavioural change to apply to the shadow tree.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Patch {
    /// Replace the node at `id` wholesale.
    Replace {
        /// Node being replaced.
        id: NodeId,
        /// Its replacement.
        node: NodeRef,
    },
    /// Apply a prop delta to the node at `id`.
    Update {
        /// Node being updated.
        id: NodeId,
        /// Props to set and remove.
        props_diff: PropDiff,
    },
    /// Insert `node` as a child of `parent` at `index`.
    Insert {
        /// Parent node.
        parent: NodeId,
        /// Insertion index among the parent's children.
        index: u16,
        /// Node to insert.
        node: NodeRef,
    },
    /// Remove the node at `id` and its subtree.
    Remove {
        /// Node to remove.
        id: NodeId,
    },
    /// Reorder `parent`'s children to match `keys`.
    Reorder {
        /// Parent node.
        parent: NodeId,
        /// Child node IDs in their new order.
        keys: Vec<NodeId>,
    },
    /// Hot-swap the body of a handler closure.
    Handler {
        /// Handler being swapped.
        id: HandlerId,
        /// Its new body.
        closure: ClosureRef,
    },
}

impl Patch {
    /// Wire tag for this patch as specified by Appendix D §D.2.
    #[must_use]
    pub const fn tag(&self) -> u8 {
        match self {
            Self::Replace { .. } => 0x01,
            Self::Update { .. } => 0x02,
            Self::Insert { .. } => 0x03,
            Self::Remove { .. } => 0x04,
            Self::Reorder { .. } => 0x05,
            Self::Handler { .. } => 0x06,
        }
    }

    /// Returns `true` when the patch changes only handler bodies, and so
    /// preserves all component state (`NFR-RELI-001`).
    #[must_use]
    pub const fn is_state_preserving(&self) -> bool {
        matches!(self, Self::Handler { .. } | Self::Update { .. })
    }
}

/// A delta over a node's prop map.
#[derive(Clone, Debug, Default)]
pub struct PropDiff {
    /// Props to set or overwrite.
    pub changes: Vec<(PropIdx, Value)>,
    /// Props to unset.
    pub removals: Vec<PropIdx>,
}

impl PropDiff {
    /// Returns `true` when the delta would change nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty() && self.removals.is_empty()
    }
}

/// A content-addressed reference to a handler body in the frame's bytecode
/// blob.
#[derive(Clone, Debug)]
pub struct ClosureRef {
    /// BLAKE3-derived digest of the bytecode, used for interning.
    pub hash: u64,
    /// Byte offset of the body within the frame's bytecode blob.
    pub bytecode_offset: u32,
    /// Length of the body in bytes.
    pub bytecode_len: u16,
    /// Signal cells this closure reads or writes.
    pub captured_signals: Vec<SignalId>,
    /// Source span of the handler body.
    pub span: Span,
}

impl ClosureRef {
    /// Returns the closure's byte range within the frame's bytecode blob.
    #[must_use]
    pub fn bytecode_range(&self) -> Range<u32> {
        self.bytecode_offset..self.bytecode_offset + u32::from(self.bytecode_len)
    }
}
