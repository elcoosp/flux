//! ADR-0027 signal-graph metadata codec (FA-IRWIRE, T13/T14).

use flux_syntax::{ClosureRef, NodeId, SignalId};

use super::WireError;
use super::closure_ref::decode_closure_ref;
use super::cursor::Reader;

/// One node's ADR-0027 Phase 2/3 signal-graph metadata on the wire
/// (Appendix D — gated by `FLAG_NODE_HAS_SIGNAL_DEPS`).
///
/// `deps` is the distinct, ascending `READ_SIGNAL` ids the node's prop and
/// control expressions read (`signal_deps`, T13). `thunk` is the optional
/// compiled prop-thunk `ClosureRef` (`prop_thunk`, T14); `None` for
/// control-only nodes. `layout` is the `prop_layout` — `record-field position →
/// prop index` mapping (T14). When `thunk` is `Some` its `captured_signals`
/// must equal `deps` (the thunk is the single source of truth); a decoder that
/// sees `thunk` without `deps` must reject the frame.
#[derive(Clone, Debug)]
pub struct NodeSignalMeta {
    /// The node this metadata describes.
    pub node_id: NodeId,
    /// Distinct, ascending signal ids the node reads.
    pub deps: Vec<SignalId>,
    /// Optional compiled prop thunk; `None` for control-only nodes.
    pub thunk: Option<ClosureRef>,
    /// `prop_layout`: record-field position → prop index.
    pub layout: Vec<u16>,
    /// For a `ForEach` node, the dedicated per-element `item` signal slot the
    /// body's row thunks read. The host allocates a fresh per-row signal seeded
    /// with `list[i]` and rewrites each row thunk's `READ_SIGNAL item_slot` to
    /// it when expanding the list (FLUX-072 / ADR-0050). `None` for every other
    /// node kind.
    pub item_slot: Option<SignalId>,
}

/// Encodes a `NodeSignalMeta` entry (Appendix D, ADR-0027 section).
///
/// Layout: `node_id(u32) | deps_count(u16) | deps(u32)* | thunk_present(u8)
/// | thunk(ClosureRef)? | layout_count(u16) | layout(u16)*`.
pub(crate) fn encode_signal_meta(w: &mut super::cursor::Writer, meta: &NodeSignalMeta) {
    w.u32(meta.node_id);
    w.u16(meta.deps.len() as u16);
    for &signal in &meta.deps {
        w.u32(signal);
    }
    match &meta.thunk {
        Some(closure) => {
            w.u8(1);
            super::closure_ref::encode_closure_ref(w, closure);
        }
        None => w.u8(0),
    }
    w.u16(meta.layout.len() as u16);
    for &idx in &meta.layout {
        w.u16(idx);
    }
    match meta.item_slot {
        Some(slot) => {
            w.u8(1);
            w.u32(slot);
        }
        None => w.u8(0),
    }
}

/// Decodes a `NodeSignalMeta` entry (Appendix D, ADR-0027 section).
///
/// Returns an error if a thunk is present but `deps` is empty — a thunk
/// without dependency data is unusable for pruning and must be rejected
/// (Appendix D §T13; `FLAG_NODE_HAS_SIGNAL_DEPS` gate).
pub(crate) fn decode_signal_meta(r: &mut Reader<'_>) -> Result<NodeSignalMeta, WireError> {
    let node_id = NodeId::from(r.u32("signal_meta.node")?);
    let dep_count = r.u16("signal_meta.deps.count")?;
    r.ensure_capacity(dep_count as usize, "signal_meta.deps")?;
    let mut deps = Vec::with_capacity(dep_count as usize);
    for _ in 0..dep_count {
        deps.push(SignalId::from(r.u32("signal_meta.deps.signal")?));
    }
    let thunk_present = r.u8("signal_meta.thunk.present")?;
    let thunk = if thunk_present != 0 {
        Some(decode_closure_ref(r)?)
    } else {
        None
    };
    // INV-1 (FA-IRWIRE): a thunk without dependency data is unusable for
    // pruning — reject the frame rather than ship a silent no-op.
    if thunk.is_some() && deps.is_empty() {
        return Err(WireError::InvalidTag {
            tag: thunk_present,
            context: "signal_meta.thunk_without_deps",
            at: r.pos(),
        });
    }
    let layout_count = r.u16("signal_meta.layout.count")?;
    r.ensure_capacity(layout_count as usize, "signal_meta.layout")?;
    let mut layout = Vec::with_capacity(layout_count as usize);
    for _ in 0..layout_count {
        layout.push(r.u16("signal_meta.layout.idx")?);
    }
    let item_slot_present = r.u8("signal_meta.item_slot.present")?;
    let item_slot = if item_slot_present != 0 {
        Some(SignalId::from(r.u32("signal_meta.item_slot.id")?))
    } else {
        None
    };
    Ok(NodeSignalMeta {
        node_id,
        deps,
        thunk,
        layout,
        item_slot,
    })
}

/// Encodes a `Vec<NodeSignalMeta>` section: a `u16` count followed by entries.
pub(crate) fn encode_signal_meta_section(w: &mut super::cursor::Writer, metas: &[NodeSignalMeta]) {
    w.u16(metas.len() as u16);
    for meta in metas {
        encode_signal_meta(w, meta);
    }
}

/// Decodes a `Vec<NodeSignalMeta>` section: a `u16` count followed by entries.
pub(crate) fn decode_signal_meta_section(
    r: &mut Reader<'_>,
) -> Result<Vec<NodeSignalMeta>, WireError> {
    let count = r.u16("signal_meta.section.count")?;
    r.ensure_capacity(count as usize, "signal_meta.section")?;
    let mut metas = Vec::with_capacity(count as usize);
    for _ in 0..count {
        metas.push(decode_signal_meta(r)?);
    }
    Ok(metas)
}
