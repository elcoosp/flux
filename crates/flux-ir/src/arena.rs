//! The reactive-tree arena (Appendix C §C.1).
//!
//! Nodes are packed into a struct-of-arrays layout for cache-linear diff
//! scanning: hot, fixed-width fields live in parallel [`Vec`]s, while
//! variable-width cold data (props, children, handlers) is serialised into
//! length-prefixed blobs addressed by per-node offset vectors.
//!
//! Two deliberate deviations from the illustrative `IRArena` in the appendices:
//! 1. `kinds` is `Vec<NodeKind>`, not `Vec<u8>`. `NodeKind` is a fieldless
//!    `#[repr(u8)]` enum, so it is layout-identical to `u8`, but reading it back
//!    needs no `unsafe` transmute or fallible tag decode (ADR-0002 / AGENTS.md).
//! 2. `spans` are stored inline as `Vec<Span>` (fixed 12 bytes, on the diff hot
//!    path) rather than in a blob, avoiding an offset indirection for hot reads.

use ahash::AHashMap;
use flux_syntax::{Child, ClosureRef, ComponentId, HandlerId, NodeId, SignalId, Span};
use flux_syntax::{NodeKind, Props, StringTable};

use crate::builder::Node;
use crate::closure::ClosureIR;
use blob::{
    pack_children, pack_handlers, pack_props, unpack_children, unpack_handlers, unpack_props,
};

/// A packed reactive tree.
///
/// Build it with [`ArenaBuilder`](crate::builder::ArenaBuilder), pack nodes with
/// [`IRArena::pack`], and read them back through [`NodeView`].
#[derive(Debug, Default, Clone)]
pub struct IRArena {
    // Struct-of-arrays for diff-hot fields.
    ids: Vec<NodeId>,
    kinds: Vec<NodeKind>,
    component_ids: Vec<ComponentId>,
    spans: Vec<Span>,

    // Offsets into the cold blobs (start of node `i`; end is the next offset,
    // or the blob length for the final node).
    props_offsets: Vec<u32>,
    children_offsets: Vec<u32>,
    handler_offsets: Vec<u32>,

    // Variable-width cold data.
    props_blob: Vec<u8>,
    children_blob: Vec<u8>,
    handlers_blob: Vec<u8>,

    // NodeId → arena slot.
    node_index: AHashMap<NodeId, usize>,

    // Shared interning table.
    string_table: StringTable,

    // Handler bytecode table.
    closures: AHashMap<HandlerId, ClosureIR>,

    // ── ADR-0027 Phase 2/3 per-node signal-graph metadata ──────────────────
    // These side-tables mirror the wire `signal_deps` / `prop_thunk` /
    // `prop_layout` sections (docs/spec/wire-signal-deps-and-thunks.md) without
    // changing the `builder::Node` field set, so off-limits consumers that
    // construct `Node { .. }` literals (e.g. `flux-differ`) keep compiling.
    // Each map is keyed by the node's `NodeId`; entries are populated by the
    // lowering pass right after `pack`ing the node.
    /// Distinct `READ_SIGNAL` ids read by a node's prop and control
    /// expressions, sorted ascending (T13). Empty when the node reads none.
    signal_deps_map: AHashMap<NodeId, Vec<SignalId>>,
    /// Per-node prop thunk closure reference (T14). `None` means no thunk was
    /// emitted for this node (e.g. a node with no props).
    prop_thunk_map: AHashMap<NodeId, Option<ClosureRef>>,
    /// Record-field position → prop index, in the order the thunk fills the
    /// `ALLOC_RECORD` (T14). Empty when the node has no thunk.
    prop_layout_map: AHashMap<NodeId, Vec<u16>>,
    /// Per-`ForEach` node: the dedicated per-element `item` signal slot its row
    /// thunks read (FLUX-072 / ADR-0050). `None` for every other node kind.
    item_slot_map: AHashMap<NodeId, Option<SignalId>>,

    // Pre-computed content hashes for O(1) differ comparisons (FLUX-014 P3).
    // `props_hashes[i]` mirrors `props_of(i).hash()`; `children_hashes[i]`
    // mirrors `hash_children(children_of(i))`. Both are populated at `pack`
    // time and never change afterwards.
    props_hashes: Vec<u64>,
    children_hashes: Vec<u64>,
}

impl IRArena {
    /// Creates an empty arena with a fresh string table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of packed nodes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Iterates over every packed [`NodeId`] in insertion order.
    pub fn all_ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.ids.iter().copied()
    }

    /// Returns `true` when no nodes have been packed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Looks up a node by [`NodeId`], returning a read-only [`NodeView`].
    #[must_use]
    pub fn get(&self, id: NodeId) -> Option<NodeView<'_>> {
        let index = *self.node_index.get(&id)?;
        Some(NodeView::new(self, index))
    }

    /// Returns the source [`Span`] a node was lowered from, if the node is in
    /// the arena. Used by the dev server to enrich telemetry with `.flux`
    /// source locations (DevTools spec §4.2).
    #[must_use]
    pub fn span_for_node_id(&self, id: NodeId) -> Option<Span> {
        let index = *self.node_index.get(&id)?;
        Some(self.spans[index])
    }

    /// Packs `node`, returning its stable [`NodeId`].
    ///
    /// The node's `id` is the source-derived ID from
    /// [`compute_node_id`](crate::compute_node_id); the arena neither mints nor
    /// remaps IDs.
    pub fn pack(&mut self, node: Node) -> NodeId {
        let index = self.ids.len();
        self.ids.push(node.id);
        self.kinds.push(node.kind);
        self.component_ids.push(node.component_id);
        self.spans.push(node.span);

        self.props_offsets.push(self.props_blob.len() as u32);
        pack_props(&mut self.props_blob, &node.props);

        self.children_offsets.push(self.children_blob.len() as u32);
        pack_children(&mut self.children_blob, &node.children);

        self.handler_offsets.push(self.handlers_blob.len() as u32);
        pack_handlers(&mut self.handlers_blob, &node.handlers);

        self.props_hashes.push(node.props.hash());
        self.children_hashes.push(hash_children(&node.children));

        self.node_index.insert(node.id, index);
        node.id
    }

    /// Inserts or replaces a closure in the table.
    pub fn add_closure(&mut self, closure: ClosureIR) {
        self.closures.insert(closure.id, closure);
    }

    /// Returns the closure for `id`, if registered.
    #[must_use]
    pub fn closure(&self, id: HandlerId) -> Option<&ClosureIR> {
        self.closures.get(&id)
    }

    /// Returns the pre-computed content hash of the props packed at `index`.
    ///
    /// The hash is computed once in [`pack`](Self::pack) and equals
    /// `self.props_of(index).hash()`, so callers can compare two nodes' props
    /// without unpacking their cold blobs. Panics if `index` is out of range.
    #[must_use]
    pub fn props_hash(&self, index: usize) -> u64 {
        self.props_hashes[index]
    }

    /// Returns the pre-computed layout hash of the children packed at `index`.
    ///
    /// The hash folds the ordered `(key, child_id)` sequence of each child slot
    /// (computed once in [`pack`](Self::pack)) and equals
    /// `hash_children(&self.children_of(index))`. It captures structural
    /// changes (add/remove/reorder) independent of props. Panics if `index` is
    /// out of range.
    #[must_use]
    pub fn children_hash(&self, index: usize) -> u64 {
        self.children_hashes[index]
    }

    /// Returns the shared string table.
    #[must_use]
    pub fn string_table(&self) -> &StringTable {
        &self.string_table
    }

    /// Replaces the arena's string table.
    ///
    /// Used by [`ArenaBuilder::finish`](crate::builder::ArenaBuilder::finish),
    /// which threads a caller-supplied interner into the packed tree so that
    /// every `Value::Str(id)` emitted during lowering resolves against the
    /// arena (Gap G3 — see `docs/adr/flux018-string-table-gap.md`).
    pub(crate) fn with_string_table(mut self, table: StringTable) -> Self {
        self.string_table = table;
        self
    }

    fn props_of(&self, index: usize) -> Props {
        let start = self.props_offsets[index] as usize;
        let end = self
            .props_offsets
            .get(index + 1)
            .copied()
            .map(|o| o as usize)
            .unwrap_or(self.props_blob.len());
        unpack_props(&self.props_blob[start..end])
    }

    fn children_of(&self, index: usize) -> Vec<Child> {
        let start = self.children_offsets[index] as usize;
        let end = self
            .children_offsets
            .get(index + 1)
            .copied()
            .map(|o| o as usize)
            .unwrap_or(self.children_blob.len());
        unpack_children(&self.children_blob[start..end])
    }

    fn handlers_of(&self, index: usize) -> Vec<HandlerId> {
        let start = self.handler_offsets[index] as usize;
        let end = self
            .handler_offsets
            .get(index + 1)
            .copied()
            .map(|o| o as usize)
            .unwrap_or(self.handlers_blob.len());
        unpack_handlers(&self.handlers_blob[start..end])
    }
}

/// Re-exported so downstream crates keep resolving `crate::arena::NodeView`
/// (the struct now lives in the `node_view` submodule).
pub use node_view::NodeView;

pub(crate) use blob::hash_children;

mod blob;
mod content_address;
mod metadata;
mod node_view;

#[cfg(test)]
mod tests;
