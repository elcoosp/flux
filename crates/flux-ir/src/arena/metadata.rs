//! ADR-0027 Phase 2/3 signal-graph metadata accessors for `IRArena`.
//!
//! `set_signal_metadata` attaches the per-node `signal_deps` / `prop_thunk` /
//! `prop_layout` / `item_slot` side-tables; the `*_of` getters read them back.
use super::IRArena;
use flux_syntax::{ClosureRef, NodeId, SignalId};

impl IRArena {
    /// Attaches the ADR-0027 Phase 2/3 signal-graph metadata for `id` (T13/T14).
    ///
    /// Called by the lowering pass immediately after `pack`ing a node. `deps`
    /// is the sorted, distinct set of `READ_SIGNAL` ids the node's prop and
    /// control expressions read; `thunk` is the optional prop-thunk closure
    /// reference; `layout` maps record-field position → prop index.
    ///
    /// # Panics
    ///
    /// Panics if `id` was not packed, which would be a lowering bug.
    pub fn set_signal_metadata(
        &mut self,
        id: NodeId,
        deps: Vec<SignalId>,
        thunk: Option<ClosureRef>,
        layout: Vec<u16>,
        item_slot: Option<SignalId>,
    ) {
        self.signal_deps_map.insert(id, deps);
        self.prop_thunk_map.insert(id, thunk);
        self.prop_layout_map.insert(id, layout);
        self.item_slot_map.insert(id, item_slot);
    }

    /// The per-element `item` signal slot for a `ForEach` node (FLUX-072 /
    /// ADR-0050), or `None` for any other node kind. The host allocates a fresh
    /// per-row signal seeded with `list[i]` and rewrites each row thunk's
    /// `READ_SIGNAL` to it when expanding the list.
    #[must_use]
    pub fn item_slot_of(&self, id: NodeId) -> Option<SignalId> {
        self.item_slot_map.get(&id).copied().flatten()
    }

    /// The distinct `READ_SIGNAL` ids `id`'s prop/control expressions read,
    /// sorted ascending (T13). Empty slice when the node reads none.
    #[must_use]
    pub fn signal_deps_of(&self, id: NodeId) -> &[SignalId] {
        static EMPTY: [SignalId; 0] = [];
        self.signal_deps_map
            .get(&id)
            .map(Vec::as_slice)
            .unwrap_or(&EMPTY)
    }

    /// The prop thunk closure reference for `id`, if one was emitted (T14).
    #[must_use]
    pub fn prop_thunk_of(&self, id: NodeId) -> Option<&ClosureRef> {
        self.prop_thunk_map.get(&id).and_then(Option::as_ref)
    }

    /// The record-field → prop-index layout for `id`'s prop thunk (T14).
    #[must_use]
    pub fn prop_layout_of(&self, id: NodeId) -> &[u16] {
        static EMPTY: [u16; 0] = [];
        self.prop_layout_map
            .get(&id)
            .map(Vec::as_slice)
            .unwrap_or(&EMPTY)
    }
}
