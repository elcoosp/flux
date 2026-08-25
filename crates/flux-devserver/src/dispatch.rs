//! Signal-deps-aware minimal-patch emission (ADR-0027 Phase 2, server half).
//!
//! The dev server is the single place that decides *which* nodes a host must
//! re-materialise after a handler dispatch. When the lowered tree carries
//! `signal_deps` (the distinct `READ_SIGNAL` ids each node's prop/control
//! expressions read), the server can invert that into a reverse index and, on a
//! dispatch that wrote signal set `S`, emit `Update` patches addressed *only* to
//! `dirty = ⋃ dependents[s] for s ∈ S` — instead of re-shipping the whole frame.
//!
//! The index is server-side state: it is never placed on the wire and is
//! rebuilt from scratch whenever the tree is re-lowered (after an edit). In
//! ADR-0027 terms this is the server-side payoff of Phase 2, scoped to
//! `|dependents[S]| + structural diff size` (see `reconcile-counters-and-budgets.md`).
//!
//! # Degradation
//!
//! Before FA-IRWIRE lands `signal_deps` in the lowered IR (T13), the pipeline
//! carries no dependency information. In that configuration the index is
//! *inactive* and the caller falls back to the existing coarse-frame behaviour;
//! no partial state is ever shipped and nothing crashes.

use std::collections::{BTreeSet, HashMap};

use flux_syntax::{HandlerId, NodeId, Patch, PropDiff, SignalId};

/// A host→server dispatch report (ADR-0027 Phase 2 host obligation).
///
/// Sent by the host after a handler closure runs. The `handler_id` identifies
/// the closure; `written` is the set of signal ids the VM wrote during that
/// dispatch. The server derives `dirty` purely from `written` and its reverse
/// index — it never re-runs the VM and never reads signal *values*.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DispatchReport {
    /// The handler closure that ran.
    pub handler_id: HandlerId,
    /// Signal ids written by the handler (from `VmOutcome.signals`).
    pub written: SignalId,
}

impl DispatchReport {
    /// Encodes the report as the wire bytes the host sends (Appendix D §D.12.6).
    ///
    /// Layout: `magic(4) version(1) frame_type(1)=0x06 handler_id(4) signal(4)`.
    /// Only one written signal is carried per frame; a host that wrote several
    /// signals sends one frame per signal. This keeps the frame fixed-width and
    /// trivially forward-extensible to a vectorised form later.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(14);
        out.extend_from_slice(&Self::MAGIC.to_le_bytes());
        out.push(Self::VERSION);
        out.push(FRAME_DISPATCH_REPORT);
        out.extend_from_slice(&self.handler_id.to_le_bytes());
        out.extend_from_slice(&self.written.to_le_bytes());
        out
    }

    /// Decodes a host dispatch-report frame, or `None` when the bytes are not a
    /// well-formed report at this protocol version.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 14 {
            return None;
        }
        if u32::from_le_bytes(bytes[0..4].try_into().ok()?) != Self::MAGIC {
            return None;
        }
        if bytes[4] != Self::VERSION {
            return None;
        }
        if bytes[5] != FRAME_DISPATCH_REPORT {
            return None;
        }
        let handler_id = HandlerId::from(u32::from_le_bytes(bytes[6..10].try_into().ok()?));
        let written = SignalId::from(u32::from_le_bytes(bytes[10..14].try_into().ok()?));
        Some(Self {
            handler_id,
            written,
        })
    }

    const MAGIC: u32 = 0x5244_4658; // "XFDR" — Flux Dispatch Report
    const VERSION: u8 = 1;
}

/// `frame_type` byte for the host→server dispatch-report frame.
pub const FRAME_DISPATCH_REPORT: u8 = 0x06;

/// Per-node `signal_deps` as seen by the server: a node id and the distinct,
/// sorted signal ids its prop/control expressions read.
///
/// This is the unit the server consumes to build its reverse index. The real
/// source is `Node.signal_deps` in the lowered IR (FA-IRWIRE, T13); until that
/// lands, the pipeline injects fixtures via [`Pipeline::set_signal_deps`].
///
/// [`Pipeline::set_signal_deps`]: crate::pipeline::Pipeline::set_signal_deps
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeSignalDeps {
    /// The node these deps belong to.
    pub id: NodeId,
    /// Distinct, ascending `READ_SIGNAL` ids read by this node's expressions.
    pub signal_deps: Vec<SignalId>,
}

/// Reverse index `SignalId → {NodeId}` plus a sibling `NodeId → {SignalId}`.
///
/// Built once per re-lowered tree from every [`NodeSignalDeps`]. The reverse
/// `dependents` map is what makes minimal-patch emission O(|written|) rather
/// than O(tree): given the written signals, `dirty` is a handful of set lookups
/// and unions.
///
/// The index is *inactive* (and reports no dependents) until at least one node
/// carries a non-empty `signal_deps`. That is the degradation guard: when the
/// lowered tree has no dependency data, the server must not emit minimal
/// patches and must fall back to coarse frames.
#[derive(Clone, Debug, Default)]
pub struct DependencyIndex {
    /// `SignalId →` node ids that read it.
    dependents: HashMap<SignalId, BTreeSet<NodeId>>,
    /// `NodeId →` signal ids it reads (the forward direction the reverse map
    /// is derived from). Retained so a node can be unregistered cheaply.
    node_deps: HashMap<NodeId, Vec<SignalId>>,
    /// Whether any node contributed dependency data this rebuild.
    active: bool,
}

impl DependencyIndex {
    /// Number of signal→nodes edges currently indexed.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.dependents.values().map(BTreeSet::len).sum()
    }

    /// Whether the last rebuild carried any dependency data.
    ///
    /// When `false`, the server must degrade to coarse-frame behaviour because
    /// there is nothing to scope patches against.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Rebuilds the index from `nodes`, discarding any previous state.
    ///
    /// Rebuilding wholesale per re-lowered tree (rather than diffing) keeps the
    /// index a pure function of the tree: a node that vanished with its signal
    /// deps leaves no stale entries. `nodes` need not be sorted; duplicates are
    /// merged.
    pub fn rebuild(&mut self, nodes: &[NodeSignalDeps]) {
        self.dependents.clear();
        self.node_deps.clear();
        self.active = false;
        for node in nodes {
            if node.signal_deps.is_empty() {
                continue;
            }
            self.active = true;
            // Record the forward mapping once.
            self.node_deps
                .entry(node.id)
                .or_insert_with(|| node.signal_deps.clone());
            for &signal in &node.signal_deps {
                self.dependents.entry(signal).or_default().insert(node.id);
            }
        }
    }

    /// The node ids that read any of `signals`, ascending and de-duplicated.
    ///
    /// This is `dirty` from the ADR-0027 dispatch algorithm: the only nodes
    /// whose rendered output may change after the signals in `written` changed.
    #[must_use]
    pub fn dependents_of(&self, signals: &[SignalId]) -> BTreeSet<NodeId> {
        let mut dirty = BTreeSet::new();
        for &signal in signals {
            if let Some(nodes) = self.dependents.get(&signal) {
                dirty.extend(nodes);
            }
        }
        dirty
    }

    /// The signal ids a single node reads, if it is indexed.
    #[must_use]
    pub fn deps_of(&self, id: NodeId) -> Option<&[SignalId]> {
        self.node_deps.get(&id).map(Vec::as_slice)
    }

    /// Iterates the `(signal, node)` edges, for diagnostics/tests.
    #[must_use]
    pub fn edges(&self) -> Vec<(SignalId, NodeId)> {
        let mut out = Vec::new();
        for (signal, nodes) in &self.dependents {
            for node in nodes {
                out.push((*signal, *node));
            }
        }
        out.sort();
        out
    }
}

/// Computes the minimal patch set for a dispatch.
///
/// Given the written signal `written` (from the [`DispatchReport`]), the current
/// `arena` (to read each dirty node's *current* props so the host can
/// re-materialise them), and the `index`, returns exactly one `Patch::Update`
/// per node in `dirty = dependents[written]`, addressed only to those nodes.
/// Nodes not in `dirty` receive nothing — this is the bounded-by-
/// `|dependents[S]|` guarantee in `reconcile-counters-and-budgets.md`.
///
/// The returned patches preserve the ADR-0027 invariant except for structural
/// control-prop changes: a node whose `if`/`ForEach` condition reads `written`
/// is already in `dirty` (its `signal_deps` captures the condition's reads), so
/// a plain `Update` carries its new computed props; the host fires the keyed
/// structural diff locally once it has the new prop values. The server's only
/// job here is to *scope* the update set, which `dirty` already does.
///
/// # Errors
///
/// Returns [`MinimalPatchError::IndexInactive`] when `index` has no dependency
/// data, signalling the caller to fall back to a coarse frame. Returns
/// [`MinimalPatchError::UnknownNode`] if a dirty node id is somehow absent from
/// `arena` (it shouldn't be — `dirty` is derived from the same tree), so the
/// caller can degrade rather than ship a dangling patch.
pub fn emit_minimal_updates(
    written: SignalId,
    arena: &flux_ir::IRArena,
    index: &DependencyIndex,
) -> Result<Vec<Patch>, MinimalPatchError> {
    if !index.is_active() {
        return Err(MinimalPatchError::IndexInactive);
    }
    let dirty = index.dependents_of(&[written]);
    let mut patches = Vec::with_capacity(dirty.len());
    for node_id in dirty {
        let view = arena
            .get(node_id)
            .ok_or(MinimalPatchError::UnknownNode(node_id))?;
        // Re-send the node's full current prop map so the host can
        // re-materialise it without a round-trip. `Update` is state-preserving
        // (see `Patch::is_state_preserving`), so component instances survive.
        let changes = view
            .props()
            .fields()
            .iter()
            .map(|(idx, value)| (*idx, value.clone()))
            .collect();
        patches.push(Patch::Update {
            id: node_id,
            props_diff: PropDiff {
                changes,
                removals: Vec::new(),
            },
        });
    }
    Ok(patches)
}

/// Why [`emit_minimal_updates`] declined to emit a minimal patch set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MinimalPatchError {
    /// The index carried no dependency data; the caller must ship a coarse frame.
    IndexInactive,
    /// A dirty node id was not present in the arena (should be unreachable).
    UnknownNode(NodeId),
}

impl MinimalPatchError {
    /// Whether this error means "degrade to coarse frame now".
    #[must_use]
    pub fn is_degradation(&self) -> bool {
        matches!(self, Self::IndexInactive)
    }
}

// ── tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use flux_ir::ArenaBuilder;
    use flux_syntax::{ComponentId, NodeKind, Props, Span, Value};

    fn pack_node(builder: &mut ArenaBuilder, id: u32, props: Vec<(u16, Value)>) {
        builder.pack(flux_ir::Node {
            id: NodeId::from(id),
            kind: NodeKind::Primitive,
            component_id: ComponentId::from(0u32),
            props: Props::from_fields(
                props
                    .into_iter()
                    .map(|(i, v)| (flux_syntax::PropIdx::from(i), v))
                    .collect(),
            ),
            children: vec![],
            handlers: vec![],
            span: Span::new(0, 0, 0),
        });
    }

    fn fixture_arena() -> flux_ir::IRArena {
        let mut b = ArenaBuilder::new();
        // Text reading `count`.
        pack_node(&mut b, 1, vec![(0, Value::Str(10))]); // text: "tapped N times"
        // Button (no signal read).
        pack_node(&mut b, 2, vec![(0, Value::Str(11))]); // text: "Increment"
        // Another text reading `count` too (independent of node 1).
        pack_node(&mut b, 3, vec![(0, Value::Str(12))]);
        // A label reading a different signal `theme`.
        pack_node(&mut b, 4, vec![(0, Value::Str(13))]);
        b.finish()
    }

    fn fixture_deps() -> Vec<NodeSignalDeps> {
        vec![
            NodeSignalDeps {
                id: NodeId::from(1u32),
                signal_deps: vec![SignalId::from(100u32)],
            },
            NodeSignalDeps {
                id: NodeId::from(2u32),
                signal_deps: vec![],
            },
            NodeSignalDeps {
                id: NodeId::from(3u32),
                signal_deps: vec![SignalId::from(100u32)],
            },
            NodeSignalDeps {
                id: NodeId::from(4u32),
                signal_deps: vec![SignalId::from(200u32)],
            },
        ]
    }

    #[test]
    fn dependents_index_maps_each_signal_to_readers() {
        let mut index = DependencyIndex::default();
        index.rebuild(&fixture_deps());

        assert!(index.is_active(), "index active once any node has deps");
        assert_eq!(index.edge_count(), 3, "two edges on 100, one on 200");
        assert_eq!(
            index.dependents_of(&[SignalId::from(100u32)]),
            BTreeSet::from([NodeId::from(1u32), NodeId::from(3u32)]),
        );
        assert_eq!(
            index.dependents_of(&[SignalId::from(200u32)]),
            BTreeSet::from([NodeId::from(4u32)]),
        );
        assert!(
            index.dependents_of(&[SignalId::from(999u32)]).is_empty(),
            "unknown signal hits nothing"
        );
    }

    #[test]
    fn minimal_patch_count_equals_dependents_without_injection() {
        let mut index = DependencyIndex::default();
        index.rebuild(&fixture_deps());
        let arena = fixture_arena();

        let patches = emit_minimal_updates(SignalId::from(100u32), &arena, &index)
            .expect("active index emits");

        // |dependents[count]| = 2 → exactly two Update patches, no others.
        assert_eq!(patches.len(), 2, "patch scope must equal |dependents[S]|");
        let ids: BTreeSet<NodeId> = patches
            .iter()
            .map(|p| match p {
                Patch::Update { id, .. } => *id,
                _ => panic!("only Update patches are emitted"),
            })
            .collect();
        assert_eq!(
            ids,
            BTreeSet::from([NodeId::from(1u32), NodeId::from(3u32)])
        );
        assert!(
            ids.iter().all(|id| id != &NodeId::from(2u32)),
            "the non-reading Button is never touched"
        );
    }

    #[test]
    fn minimal_patch_is_state_preserving_update() {
        let mut index = DependencyIndex::default();
        index.rebuild(&fixture_deps());
        let arena = fixture_arena();

        let patches = emit_minimal_updates(SignalId::from(100u32), &arena, &index).unwrap();
        for patch in &patches {
            assert!(
                patch.is_state_preserving(),
                "minimal patches must preserve component state"
            );
            if let Patch::Update { props_diff, .. } = patch {
                assert!(
                    !props_diff.changes.is_empty(),
                    "each dirty node's current props are re-sent"
                );
            }
        }
    }

    #[test]
    fn unrelated_signal_dirty_set_is_empty() {
        let mut index = DependencyIndex::default();
        index.rebuild(&fixture_deps());

        // Writing a signal nothing reads → no dirty nodes → no patches.
        let dirty = index.dependents_of(&[SignalId::from(999u32)]);
        assert!(dirty.is_empty());
    }

    #[test]
    fn degradation_when_no_signal_deps() {
        let mut index = DependencyIndex::default();
        // Rebuilt from nodes with *no* deps → inactive.
        index.rebuild(&[NodeSignalDeps {
            id: NodeId::from(1u32),
            signal_deps: vec![],
        }]);
        assert!(!index.is_active(), "empty deps → inactive → degrade");

        let arena = fixture_arena();
        let err = emit_minimal_updates(SignalId::from(100u32), &arena, &index)
            .expect_err("inactive index refuses to emit");
        assert!(
            err.is_degradation(),
            "caller must fall back to coarse frame"
        );
        assert_eq!(err, MinimalPatchError::IndexInactive);
    }

    #[test]
    fn dispatch_report_round_trips_over_wire() {
        let report = DispatchReport {
            handler_id: HandlerId::from(7u32),
            written: SignalId::from(100u32),
        };
        let bytes = report.to_bytes();
        let back = DispatchReport::from_bytes(&bytes).expect("decodes");
        assert_eq!(back, report);
        assert_eq!(bytes.len(), 14, "fixed-width frame");
        assert_eq!(bytes[5], FRAME_DISPATCH_REPORT);
    }

    #[test]
    fn dispatch_report_rejects_garbage() {
        assert!(DispatchReport::from_bytes(&[]).is_none());
        assert!(DispatchReport::from_bytes(b"not a frame at all").is_none());
        // Valid magic/version but wrong frame type must be rejected.
        let mut bytes = DispatchReport {
            handler_id: HandlerId::from(1u32),
            written: SignalId::from(2u32),
        }
        .to_bytes();
        bytes[5] = 0x99;
        assert!(DispatchReport::from_bytes(&bytes).is_none());
    }

    #[test]
    fn index_rebuild_replaces_state_not_merges() {
        let mut index = DependencyIndex::default();
        index.rebuild(&fixture_deps());
        assert_eq!(index.edge_count(), 3);
        // A second rebuild with a single node must drop the previous edges.
        index.rebuild(&[NodeSignalDeps {
            id: NodeId::from(9u32),
            signal_deps: vec![SignalId::from(300u32)],
        }]);
        assert_eq!(index.edge_count(), 1);
        assert_eq!(
            index.dependents_of(&[SignalId::from(300u32)]),
            BTreeSet::from([NodeId::from(9u32)])
        );
        assert!(
            index.dependents_of(&[SignalId::from(100u32)]).is_empty(),
            "stale edges from the prior tree are gone"
        );
    }
}
