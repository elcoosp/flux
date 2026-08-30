//! Read-only projection of one packed node (`NodeView`).
//!
//! Constructed by `IRArena::get`; every accessor decodes the node's slice of
//! the cold blobs on demand so the arena stays compact.
use super::IRArena;
use flux_syntax::{
    Child, ClosureRef, ComponentId, HandlerId, NodeId, NodeKind, Props, SignalId, Span,
};
/// A read-only projection of one packed node.
#[derive(Debug, Clone, Copy)]
pub struct NodeView<'a> {
    arena: &'a IRArena,
    index: usize,
}

impl<'a> NodeView<'a> {
    /// Constructs a `NodeView` over `arena` at `index`.
    ///
    /// `pub(crate)` so the parent `arena` module can build one from
    /// `IRArena::get` without exposing the private fields crate-wide.
    pub(crate) fn new(arena: &'a IRArena, index: usize) -> Self {
        Self { arena, index }
    }

    /// The node's stable ID.
    #[must_use]
    pub fn id(&self) -> NodeId {
        self.arena.ids[self.index]
    }

    /// The node kind.
    #[must_use]
    pub fn kind(&self) -> NodeKind {
        self.arena.kinds[self.index]
    }

    /// The interned component/primitive name.
    #[must_use]
    pub fn component_id(&self) -> ComponentId {
        self.arena.component_ids[self.index]
    }

    /// The source span this node was lowered from.
    #[must_use]
    pub fn span(&self) -> Span {
        self.arena.spans[self.index]
    }

    /// The node's prop map (unpacked from the cold blob).
    #[must_use]
    pub fn props(&self) -> Props {
        self.arena.props_of(self.index)
    }

    /// The node's child slots (unpacked from the cold blob).
    #[must_use]
    pub fn children(&self) -> Vec<Child> {
        self.arena.children_of(self.index)
    }

    /// The handlers bound by this node (unpacked from the cold blob).
    #[must_use]
    pub fn handlers(&self) -> Vec<HandlerId> {
        self.arena.handlers_of(self.index)
    }

    /// The pre-computed content hash of this node's props.
    ///
    /// Equal to `self.props().hash()`; exposed so the differ can compare two
    /// nodes' props with a single `u64` read instead of unpacking each blob.
    #[must_use]
    pub fn props_hash(&self) -> u64 {
        self.arena.props_hashes[self.index]
    }

    /// The pre-computed layout hash of this node's children.
    ///
    /// Equal to the fold of the node's ordered child slots; changes when a
    /// child is added, removed, or reordered.
    #[must_use]
    pub fn children_hash(&self) -> u64 {
        self.arena.children_hashes[self.index]
    }

    /// The distinct `READ_SIGNAL` ids this node's prop/control expressions
    /// read, sorted ascending (ADR-0027 T13). Empty when the node reads none.
    #[must_use]
    pub fn signal_deps(&self) -> &[SignalId] {
        self.arena.signal_deps_of(self.id())
    }

    /// The prop thunk closure reference for this node, if one was emitted
    /// (ADR-0027 T14). `None` when the node has no props to materialise.
    #[must_use]
    pub fn prop_thunk(&self) -> Option<&ClosureRef> {
        self.arena.prop_thunk_of(self.id())
    }

    /// The record-field → prop-index layout for this node's prop thunk
    /// (ADR-0027 T14).
    #[must_use]
    pub fn prop_layout(&self) -> &[u16] {
        self.arena.prop_layout_of(self.id())
    }
}
