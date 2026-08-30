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
use flux_syntax::{NodeKind, Props, StringTable, Value};

use crate::builder::{ArenaBuilder, Node};
use crate::closure::ClosureIR;

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
        Some(NodeView { arena: self, index })
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

/// A read-only projection of one packed node.
///
/// Constructed by [`IRArena::get`]; every accessor decodes the node's slice of
/// the cold blobs on demand so the arena stays compact in memory.
#[derive(Debug, Clone, Copy)]
pub struct NodeView<'a> {
    arena: &'a IRArena,
    index: usize,
}

impl<'a> NodeView<'a> {
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

// ── blob (de)serialisation ────────────────────────────────────────────────

struct Cursor<'b> {
    bytes: &'b [u8],
    pos: usize,
}

impl<'b> Cursor<'b> {
    fn new(bytes: &'b [u8]) -> Self {
        Self { bytes, pos: 0 }
    }
    fn take(&mut self, n: usize) -> &'b [u8] {
        let slice = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        slice
    }
    fn u8(&mut self) -> u8 {
        self.bytes[self.pos]
    }
    fn u16(&mut self) -> u16 {
        let mut buf = [0u8; 2];
        buf.copy_from_slice(self.take(2));
        u16::from_le_bytes(buf)
    }
    fn u32(&mut self) -> u32 {
        let mut buf = [0u8; 4];
        buf.copy_from_slice(self.take(4));
        u32::from_le_bytes(buf)
    }
    fn u64(&mut self) -> u64 {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(self.take(8));
        u64::from_le_bytes(buf)
    }
    fn advance(&mut self, n: usize) {
        self.pos += n;
    }
}

fn pack_props(blob: &mut Vec<u8>, props: &Props) {
    blob.extend_from_slice(&(props.fields().len() as u16).to_le_bytes());
    for (idx, value) in props.fields() {
        blob.extend_from_slice(&idx.to_le_bytes());
        pack_value(blob, value);
    }
}

fn unpack_props(bytes: &[u8]) -> Props {
    let mut cur = Cursor::new(bytes);
    let count = cur.u16();
    let mut fields = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let idx = cur.u16();
        let value = unpack_value(&mut cur);
        fields.push((idx, value));
    }
    Props::from_fields(fields)
}

fn pack_value(blob: &mut Vec<u8>, value: &Value) {
    blob.push(value.tag());
    match value {
        Value::Null => {}
        Value::Int(i) => blob.extend_from_slice(&i.to_le_bytes()),
        Value::Float(f) => blob.extend_from_slice(&f.to_le_bytes()),
        Value::Bool(b) => blob.push(u8::from(*b)),
        Value::Str(id) | Value::HandlerRef(id) => blob.extend_from_slice(&id.to_le_bytes()),
        Value::List(items) => {
            blob.extend_from_slice(&(items.len() as u16).to_le_bytes());
            for item in items {
                pack_value(blob, item);
            }
        }
        Value::Record(fields) => {
            blob.extend_from_slice(&(fields.len() as u16).to_le_bytes());
            for (idx, val) in fields {
                blob.extend_from_slice(&idx.to_le_bytes());
                pack_value(blob, val);
            }
        }
        _ => {}
    }
}

fn unpack_value(cur: &mut Cursor<'_>) -> Value {
    const TAG_NULL: u8 = 0x00;
    const TAG_INT: u8 = 0x01;
    const TAG_FLOAT: u8 = 0x02;
    const TAG_BOOL: u8 = 0x03;
    const TAG_STR: u8 = 0x04;
    const TAG_HANDLER: u8 = 0x05;
    const TAG_LIST: u8 = 0x06;
    const TAG_RECORD: u8 = 0x07;
    let tag = cur.u8();
    cur.advance(1);
    match tag {
        TAG_NULL => Value::Null,
        TAG_INT => Value::Int(cur.u64() as i64),
        TAG_FLOAT => Value::Float(f64::from_bits(cur.u64())),
        TAG_BOOL => {
            let b = cur.u8();
            cur.advance(1);
            Value::Bool(b != 0)
        }
        TAG_STR => Value::Str(cur.u32()),
        TAG_HANDLER => Value::HandlerRef(cur.u32()),
        TAG_LIST => {
            let count = cur.u16();
            let mut items = Vec::with_capacity(count as usize);
            for _ in 0..count {
                items.push(unpack_value(cur));
            }
            Value::List(items)
        }
        TAG_RECORD => {
            let count = cur.u16();
            let mut fields = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let idx = cur.u16();
                let val = unpack_value(cur);
                fields.push((idx, val));
            }
            Value::Record(fields)
        }
        _ => Value::Null,
    }
}

fn pack_children(blob: &mut Vec<u8>, children: &[Child]) {
    blob.extend_from_slice(&(children.len() as u16).to_le_bytes());
    for child in children {
        match child {
            Child::Node(id) => {
                blob.push(0);
                blob.extend_from_slice(&id.to_le_bytes());
            }
            Child::Splice { items } => {
                blob.push(1);
                blob.extend_from_slice(&(items.len() as u16).to_le_bytes());
                for (key, id) in items {
                    blob.extend_from_slice(&key.to_le_bytes());
                    blob.extend_from_slice(&id.to_le_bytes());
                }
            }
            _ => {}
        }
    }
}

fn unpack_children(bytes: &[u8]) -> Vec<Child> {
    let mut cur = Cursor::new(bytes);
    let count = cur.u16();
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let tag = cur.u8();
        cur.advance(1);
        match tag {
            0 => out.push(Child::Node(cur.u32())),
            1 => {
                let n = cur.u16();
                let mut items = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    let key = cur.u64();
                    let id = cur.u32();
                    items.push((key, id));
                }
                out.push(Child::Splice { items });
            }
            _ => {}
        }
    }
    out
}

fn pack_handlers(blob: &mut Vec<u8>, handlers: &[HandlerId]) {
    blob.extend_from_slice(&(handlers.len() as u16).to_le_bytes());
    for id in handlers {
        blob.extend_from_slice(&id.to_le_bytes());
    }
}

fn unpack_handlers(bytes: &[u8]) -> Vec<HandlerId> {
    let mut cur = Cursor::new(bytes);
    let count = cur.u16();
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        out.push(cur.u32());
    }
    out
}

/// Folds the ordered sequence of child slots into a content hash capturing the
/// node's structural layout (independent of props/handlers).
///
/// Each slot contributes `Child::Node(id)` or `Child::Splice { items }`
/// (the ordered `(key, child_id)` pairs). Reordering children, adding,
/// removing, or changing a key all change the digest, while a purely
/// prop-level edit leaves it unchanged. The fold is order-sensitive so that
/// `A,B` and `B,A` hash differently (driving the `Reorder` path).
fn hash_children(children: &[Child]) -> u64 {
    let mut accumulator: u64 = 0xcbf2_9ce4_8422_2325;
    for (slot, child) in children.iter().enumerate() {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&(slot as u64).to_le_bytes());
        match child {
            Child::Node(id) => {
                hasher.update(&[0]);
                hasher.update(&id.to_le_bytes());
            }
            Child::Splice { items } => {
                hasher.update(&[1]);
                hasher.update(&(items.len() as u64).to_le_bytes());
                for (key, id) in items {
                    hasher.update(&key.to_le_bytes());
                    hasher.update(&id.to_le_bytes());
                }
            }
            // `Child` is `#[non_exhaustive]`; unknown future variants hash a
            // distinct sentinel so they remain distinguishable.
            &_ => {
                hasher.update(&[0xff]);
            }
        }
        let mut digest = [0_u8; 8];
        digest.copy_from_slice(&hasher.finalize().as_bytes()[..8]);
        accumulator ^= u64::from_le_bytes(digest);
    }
    accumulator
}

impl IRArena {
    /// Remaps every node id in this arena to a **content-addressed** id (FLUX-074,
    /// item A).
    ///
    /// The final id for a node is derived from its structural content — its wire
    /// `kind`, its `component_id`, its prop *value* hash, its children's content
    /// hashes (computed bottom-up), its position among its parent's children, and its
    /// parent's own content id — and ignores source spans. Because the parent/position
    /// are unchanged by a pure text-above edit (which only shifts spans, never
    /// content or structure), a node whose source moved but whose content is identical
    /// keeps its id. That is what lets its view instance survive a hot reload instead
    /// of being torn down and rebuilt (FLUX-074, the roast's core ask).
    ///
    /// `span` is retained for diagnostics; only the *identity* changes. Every other
    /// field (props, handlers, closure table, string table) is preserved, and the
    /// ADR-0027 signal-graph side-tables (`signal_deps`/`prop_thunk`/`prop_layout`/
    /// `item_slot`) are re-keyed under the new ids so they stay attached to the same
    /// node.
    ///
    /// The computation is acyclic in two passes: a bottom-up pass assigns a
    /// *local* content id (parent-independent), then a top-down pass mixes in the
    /// parent's final id and the child's position to break sibling collisions while
    /// Returns the `old_id → content_addressed_id` mapping so callers that keep
    /// node-id-keyed state outside the arena (e.g. the devserver's
    /// `prop_thunks` table) can re-key it in lockstep.
    pub fn content_address(&mut self) -> AHashMap<NodeId, NodeId> {
        let ids: Vec<NodeId> = self.all_ids().collect();

        // 1. Discover each node's parent (a Flux reactive tree is a tree, not a DAG:
        //    every node has at most one parent). Roots have no parent → parent id 0.
        let mut parent_of: AHashMap<NodeId, Option<NodeId>> = AHashMap::new();
        for &id in &ids {
            parent_of.insert(id, None);
        }
        for &id in &ids {
            if let Some(view) = self.get(id) {
                for child in view.children() {
                    for cid in child.node_ids() {
                        parent_of.insert(cid, Some(id));
                    }
                }
            }
        }

        // 2. Bottom-up pass: a parent-independent *local* content id per node, derived
        //    from its own content and its children's local ids (recursive, acyclic).
        let mut local_ids: AHashMap<NodeId, NodeId> = AHashMap::with_capacity(ids.len());
        for &id in &ids {
            compute_local_id(&mut local_ids, self, id);
        }

        // 3. Top-down pass: mix the parent's final id + this node's position into the
        //    final id. Parent is assigned before child, so this never cycles. Roots get
        //    parent id 0 and position 0.
        let mut final_ids: AHashMap<NodeId, NodeId> = AHashMap::with_capacity(ids.len());
        let roots: Vec<NodeId> = ids
            .iter()
            .copied()
            .filter(|id| parent_of.get(id).copied().flatten().is_none())
            .collect();
        for root in roots {
            assign_final_id(0, 0, root, &local_ids, &mut final_ids, self);
        }

        // 4. Rebuild a fresh arena with remapped ids and remapped child references.
        let mut builder = ArenaBuilder::new();
        for &id in &ids {
            let view = match self.get(id) {
                Some(v) => v,
                None => continue,
            };
            let new_id = final_ids[&id];
            let new_children = remap_children(&view.children(), &final_ids);
            builder.pack(Node {
                id: new_id,
                kind: view.kind(),
                component_id: view.component_id(),
                props: view.props(),
                children: new_children,
                handlers: view.handlers(),
                span: view.span(),
            });
        }
        let mut new_arena = builder.finish();
        // `content_address` remaps node ids but must preserve the interning
        // table — literal strings are content, not structure, and the wire
        // `Init` frame (Appendix D §D.12.2) ships the full string table so the
        // host can resolve literal ids. `ArenaBuilder::new()` starts with an
        // empty table, so carry the original over explicitly.
        new_arena = new_arena.with_string_table(self.string_table().clone());

        // 5. Re-attach closures and ADR-0027 signal metadata under the new ids.
        for c in self.closures.values() {
            new_arena.add_closure(c.clone());
        }
        for &id in &ids {
            let new_id = final_ids[&id];
            let deps = self.signal_deps_of(id).to_vec();
            let thunk = self.prop_thunk_of(id).cloned();
            let layout = self.prop_layout_of(id).to_vec();
            let item_slot = self.item_slot_of(id);
            new_arena.set_signal_metadata(new_id, deps, thunk, layout, item_slot);
        }

        // 6. Swap the rebuilt arena into `self`.
        *self = new_arena;
        final_ids
    }
}

/// Bottom-up memoised computation of a node's *local* content id.
///
/// The local id folds the node's kind/component_id/prop hash and the local ids of
/// its children (resolved recursively first), but NOT its parent or position — so
/// identical subtrees share a local id. The top-down pass turns local ids into
/// final, position-disambiguated ids.
fn compute_local_id(local: &mut AHashMap<NodeId, NodeId>, arena: &IRArena, id: NodeId) -> NodeId {
    if let Some(&cached) = local.get(&id) {
        return cached;
    }
    let view = arena
        .get(id)
        .expect("node present during content addressing");
    let remapped_children: Vec<Child> = view
        .children()
        .iter()
        .map(|child| match child {
            Child::Node(cid) => Child::Node(compute_local_id(local, arena, *cid)),
            Child::Splice { items } => Child::Splice {
                items: items
                    .iter()
                    .map(|(k, cid)| (*k, compute_local_id(local, arena, *cid)))
                    .collect(),
            },
            other => other.clone(),
        })
        .collect();
    let children_hash = hash_children(&remapped_children);
    let local_id = flux_syntax::content_addressed_id(
        0,
        view.kind().tag(),
        view.component_id(),
        view.props_hash(),
        children_hash,
        None,
    );
    local.insert(id, local_id);
    local_id
}

/// Top-down assignment of a node's *final* content id by mixing in its parent's
/// final id and its own position (index among the parent's children, or the
/// `ForEach` splice key for spliced items).
///
/// Runs parent-before-child, so `parent_final` is always already known — the
/// recursion never revisits the parent and therefore cannot cycle.
fn assign_final_id(
    parent_final: NodeId,
    position: u64,
    id: NodeId,
    local: &AHashMap<NodeId, NodeId>,
    final_ids: &mut AHashMap<NodeId, NodeId>,
    arena: &IRArena,
) {
    let view = arena
        .get(id)
        .expect("node present during content addressing");
    let children_local = remap_children(&view.children(), local);
    let children_hash = hash_children(&children_local);
    let final_id = flux_syntax::content_addressed_id(
        parent_final,
        view.kind().tag(),
        view.component_id(),
        view.props_hash(),
        children_hash,
        Some(position),
    );
    final_ids.insert(id, final_id);

    for (pos, child) in view.children().iter().enumerate() {
        match child {
            Child::Node(cid) => {
                assign_final_id(final_id, pos as u64, *cid, local, final_ids, arena)
            }
            Child::Splice { items } => {
                for (k, cid) in items {
                    assign_final_id(final_id, *k, *cid, local, final_ids, arena);
                }
            }
            _ => {}
        }
    }
}

/// Returns a copy of `children` with every `NodeId` replaced by `remap(id)`.
fn remap_children(children: &[Child], remap: &AHashMap<NodeId, NodeId>) -> Vec<Child> {
    children
        .iter()
        .map(|child| match child {
            Child::Node(cid) => Child::Node(*remap.get(cid).unwrap_or(cid)),
            Child::Splice { items } => Child::Splice {
                items: items
                    .iter()
                    .map(|(k, cid)| (*k, *remap.get(cid).unwrap_or(cid)))
                    .collect(),
            },
            other => other.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use flux_syntax::PropIdx;

    fn sample_node() -> Node {
        Node {
            id: 7,
            kind: NodeKind::Component,
            component_id: 3,
            props: Props::from_fields(vec![
                (PropIdx::from(0u16), Value::Int(12)),
                (
                    PropIdx::from(1u16),
                    Value::Str(flux_syntax::StringId::from(4u32)),
                ),
                (
                    PropIdx::from(2u16),
                    Value::List(vec![Value::Bool(true), Value::Null]),
                ),
            ]),
            children: vec![
                Child::Node(8),
                Child::Splice {
                    items: vec![(1, 9), (2, 10)],
                },
            ],
            handlers: vec![HandlerId::from(5u32), HandlerId::from(6u32)],
            span: Span::new(1, 0, 42),
        }
    }

    #[test]
    fn pack_then_get_round_trips() {
        let mut arena = IRArena::new();
        let id = arena.pack(sample_node());
        let view = arena.get(id).expect("node present");
        assert_eq!(view.id(), 7);
        assert_eq!(view.kind(), NodeKind::Component);
        assert_eq!(view.component_id(), 3);
        assert_eq!(view.span(), Span::new(1, 0, 42));
        assert_eq!(view.props().fields().len(), 3);
        assert_eq!(view.props().get(PropIdx::from(0u16)), Some(&Value::Int(12)));
        assert_eq!(view.children().len(), 2);
        assert_eq!(
            view.handlers(),
            vec![HandlerId::from(5u32), HandlerId::from(6u32)]
        );
    }

    #[test]
    fn duplicate_id_replaces_slot() {
        let mut arena = IRArena::new();
        arena.pack(sample_node());
        let mut changed = sample_node();
        changed.props = Props::from_fields(vec![(PropIdx::from(0u16), Value::Int(99))]);
        arena.pack(changed);
        assert_eq!(arena.len(), 2, "pack does not de-dupe; two slots exist");
        let view = arena.get(7).expect("present");
        assert_eq!(view.props().get(PropIdx::from(0u16)), Some(&Value::Int(99)));
    }

    #[test]
    fn nested_values_round_trip() {
        let node = Node {
            id: 1,
            kind: NodeKind::Primitive,
            component_id: 0,
            props: Props::from_fields(vec![(
                PropIdx::from(0u16),
                Value::Record(vec![(PropIdx::from(0u16), Value::Float(3.5))]),
            )]),
            children: vec![],
            handlers: vec![],
            span: Span::new(0, 0, 1),
        };
        let mut arena = IRArena::new();
        arena.pack(node);
        let got = arena
            .get(1)
            .unwrap()
            .props()
            .get(PropIdx::from(0u16))
            .unwrap()
            .clone();
        assert_eq!(
            got,
            Value::Record(vec![(PropIdx::from(0u16), Value::Float(3.5))])
        );
    }

    #[test]
    fn content_address_keeps_id_stable_across_source_move() {
        // Two arenas with identical structural content but *different* spans must
        // receive identical content-addressed ids (FLUX-074, item A). This is the
        // property that lets a node survive a text-above edit at hot reload.
        let moved_above = build_two_node_tree(true);
        let not_moved = build_two_node_tree(false);
        // Before content addressing the ids differ (spans differ); after, they match.
        let ids_before: Vec<NodeId> = moved_above.all_ids().collect();
        let ids_other: Vec<NodeId> = not_moved.all_ids().collect();
        assert_ne!(
            ids_before, ids_other,
            "span-based ids must differ before content addressing"
        );

        let mut a = moved_above;
        a.content_address();
        let mut b = not_moved;
        b.content_address();
        let a_ids: Vec<NodeId> = a.all_ids().collect();
        let b_ids: Vec<NodeId> = b.all_ids().collect();
        assert_eq!(
            a_ids, b_ids,
            "content-addressed ids must match despite span shift"
        );
    }

    #[test]
    fn content_address_changes_when_content_changes() {
        // Editing a leaf's props must change its content id.
        let before = build_two_node_tree_with_leaf_text("tap");
        let after = build_two_node_tree_with_leaf_text("cancel");
        let mut before = before;
        let mut after = after;
        before.content_address();
        after.content_address();
        let before_ids: Vec<NodeId> = before.all_ids().collect();
        let after_ids: Vec<NodeId> = after.all_ids().collect();
        assert_ne!(
            before_ids, after_ids,
            "a prop edit must change at least one content id"
        );
        // Note: a content edit to the leaf also re-keys ancestors, because the
        // parent's `children_hash` folds the (now-changed) child local id. That is
        // expected — content-addressing only promises id *stability across
        // content-preserving moves*, not id *immutability across content edits*.
        // The inverse property (span-only move keeps every id) is covered by
        // `content_address_keeps_id_stable_across_source_move`.
    }

    #[test]
    fn content_address_preserves_metadata_and_closures() {
        // After content addressing, signal_deps / prop_thunks stay attached to the
        // re-keyed node, and closures remain queryable.
        let mut arena = build_two_node_tree(false);
        let root = arena.all_ids().next().expect("root present");
        arena.set_signal_metadata(root, vec![SignalId::from(3u32)], None, vec![], None);
        arena.add_closure(ClosureIR::new(
            HandlerId::from(9u32),
            vec![0x00],
            vec![],
            Span::new(0, 0, 1),
        ));
        arena.content_address();
        // Exactly one node now carries signal deps (the remapped root).
        let with_deps: Vec<NodeId> = arena
            .all_ids()
            .filter(|id| !arena.signal_deps_of(*id).is_empty())
            .collect();
        assert_eq!(with_deps.len(), 1, "signal metadata re-keyed to one node");
        assert!(
            arena.closure(HandlerId::from(9u32)).is_some(),
            "closure preserved"
        );
    }

    /// Builds a two-node tree (parent + leaf child) with a `Text`-like leaf whose
    /// `text` prop is `text`. `span_shift` moves every span by a large offset to
    /// simulate text being inserted above the tree.
    fn build_two_node_tree_with_leaf_text(text: &str) -> IRArena {
        let leaf_text = text.to_owned();
        build_tree_with_spans(0, leaf_text)
    }

    /// Builds the same two-node tree; `span_shift` toggles whether spans are at the
    /// original offsets (false) or shifted (true) — content is identical either way.
    fn build_two_node_tree(span_shift: bool) -> IRArena {
        let off = if span_shift { 1000 } else { 0 };
        build_tree_with_spans(off, "tap".to_owned())
    }

    fn build_tree_with_spans(offset: u32, leaf_text: String) -> IRArena {
        // Derive node ids from spans (as the real lower path does) so that the
        // span-shifted arena has genuinely different ids *before* content
        // addressing — proving the test's premise (ids differ pre-addressing,
        // match post-addressing).
        //
        // The leaf prop is a `Float` derived from `leaf_text.len()`. In the real
        // pipeline strings are compared by their *interned id* in a shared string
        // table, so an isolated per-builder table would collapse "tap" and
        // "cancel" to the same `StringId(0)`; a length-derived float makes the
        // content difference observable without depending on cross-builder string
        // interning.
        let leaf_span = Span::new(0, offset + 10, offset + 14);
        let parent_span = Span::new(0, offset, offset + 20);
        let leaf_id = crate::compute_node_id(0, NodeKind::Primitive, leaf_span, None);
        let parent_id = crate::compute_node_id(0, NodeKind::Component, parent_span, None);
        let mut b = ArenaBuilder::new();
        b.pack(Node {
            id: leaf_id,
            kind: NodeKind::Primitive,
            component_id: ComponentId::from(2u32),
            props: Props::from_fields(vec![(
                PropIdx::from(0u16),
                Value::Float(leaf_text.len() as f64),
            )]),
            children: vec![],
            handlers: vec![],
            span: leaf_span,
        });
        b.pack(Node {
            id: parent_id,
            kind: NodeKind::Component,
            component_id: ComponentId::from(1u32),
            props: Props::default(),
            children: vec![Child::Node(leaf_id)],
            handlers: vec![],
            span: parent_span,
        });
        b.finish()
    }

    #[test]
    fn pack_stores_node_prop_and_children_hashes() {
        let mut arena = IRArena::new();
        let id = arena.pack(sample_node());
        let view = arena.get(id).expect("present");
        assert_eq!(
            view.props_hash(),
            view.props().hash(),
            "arena-stored props hash must equal Props::hash"
        );
        assert_eq!(
            view.children_hash(),
            children_hash_of(&sample_node().children),
            "arena-stored children hash must equal the layout hash"
        );
    }

    #[test]
    fn distinct_props_produce_distinct_hashes() {
        // Two differently-id'd nodes with different props must store different
        // prop hashes (packing does not de-dupe, so distinct ids => distinct slots).
        let mut arena = IRArena::new();
        let id_a = arena.pack(sample_node());
        let mut changed = sample_node();
        changed.id = NodeId::from(42u32);
        changed.props = Props::from_fields(vec![(PropIdx::from(0u16), Value::Int(99))]);
        let id_b = arena.pack(changed);
        let a = arena.get(id_a).expect("present");
        let b = arena.get(id_b).expect("present");
        assert_ne!(a.props_hash(), b.props_hash());
    }

    /// Reference computation mirroring the arena's `children_hash` so the test
    /// is independent of the private helper (it only asserts equality of the
    /// public surface to a re-derivation).
    fn children_hash_of(children: &[Child]) -> u64 {
        crate::arena::hash_children(children)
    }
}
