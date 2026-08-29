//! Time-travel state reconstruction (ADR-0042).
//!
//! Given a base snapshot and the telemetry events since it, [`reconstruct_state`]
//! replays the events through a pure, allocation-light simulator to rebuild the
//! full DevTools state at any point in the timeline. Replay is deterministic so
//! scrubbing always yields the same result for a given index.

use flux_ir_serde::{EnrichedTelemetryEvent, Rect};
use flux_syntax::{EffectId, NodeId, SignalId, Value};

/// The reconstructed VM register view at a point in time.
pub type Registers = Box<[Value; 16]>;

/// A fully reconstructed DevTools state snapshot.
///
/// Produced by replaying telemetry events from a base snapshot (ADR-0042
/// §2). It is the model the gpui views render when scrubbing the timeline.
/// A native view node in the live component tree.
///
/// Carries enough to rebuild the parent/child hierarchy on the DevTools side:
/// the node id, its parent's id, and the optional layout [`Rect`] (the host may
/// know the node exists but be unable to measure geometry — see ADR-0048).
#[derive(Clone, Debug, PartialEq)]
pub struct ViewFrame {
    /// IR node backing the native view.
    pub node_id: NodeId,
    /// Parent IR node id (`0` for the tree root).
    pub parent_id: NodeId,
    /// Layout rectangle, or `None` when the host cannot measure it.
    pub frame: Option<Rect>,
}

/// Reconstructed DevTools state for a single timeline position.
#[derive(Clone, Debug, PartialEq)]
pub struct ReconstructedState {
    /// VM instruction pointer (bytecode offset), if known.
    pub bytecode_offset: Option<u32>,
    /// Current opcode at the instruction pointer.
    pub opcode: Option<u8>,
    /// VM register bank (r0–r15).
    pub registers: Registers,
    /// Remaining gas.
    pub gas_remaining: Option<u32>,
    /// Signal cell values keyed by [`SignalId`].
    pub signals: Vec<(SignalId, Value)>,
    /// Native view nodes in the live component tree. The rect is `None` when the
    /// host knows the node exists but cannot measure its geometry (the host crate
    /// is Android-free and drives in-memory adapter views, so pixel rects are
    /// only available in the platform shell — see ADR-0048). We still record node
    /// presence so the component tree renders the live node graph.
    pub view_frames: Vec<ViewFrame>,
    /// Reactive dependency edges: for each written signal, the effect IDs that
    /// re-run when it changes (PRD-P user story 2 — "what reads" a signal). The
    /// signal-graph view renders these so a developer can see reactivity the way
    /// the VM actually works.
    pub signal_edges: Vec<(SignalId, Vec<EffectId>)>,
    /// Whether the VM is currently paused.
    pub paused: bool,
}

impl ReconstructedState {
    /// The base (empty) state the timeline anchors to.
    #[must_use]
    pub fn base() -> Self {
        Self {
            bytecode_offset: None,
            opcode: None,
            registers: Box::new(std::array::from_fn(|_| Value::Null)),
            gas_remaining: None,
            signals: Vec::new(),
            view_frames: Vec::new(),
            signal_edges: Vec::new(),
            paused: false,
        }
    }
}

/// Replays `events` (each an [`EnrichedTelemetryEvent`]) onto `base`, returning
/// the state at the end of the slice.
///
/// `base` is the nearest base snapshot at or before the target index (ADR-0042
/// §2). The fold is pure: it never touches I/O and allocates only the
/// reconstructed collections, so it is cheap to run on every scrub frame and is
/// unit-tested without a display.
#[must_use]
pub fn reconstruct_state(
    base: &ReconstructedState,
    events: &[EnrichedTelemetryEvent],
) -> ReconstructedState {
    let mut state = base.clone();
    for event in events {
        match event {
            EnrichedTelemetryEvent::VmStep {
                bytecode_offset,
                opcode,
                registers,
                gas_remaining,
                ..
            } => {
                state.bytecode_offset = Some(*bytecode_offset);
                state.opcode = Some(*opcode);
                state.registers = registers.clone();
                state.gas_remaining = Some(*gas_remaining);
            }
            EnrichedTelemetryEvent::SignalWrite {
                signal_id,
                new_value,
                triggered_effect_ids,
                ..
            } => {
                upsert(&mut state.signals, *signal_id, new_value.clone());
                // Record the reactivity edge: effects that re-run when this
                // signal changes (PRD-P user story 2).
                upsert(
                    &mut state.signal_edges,
                    *signal_id,
                    triggered_effect_ids.clone(),
                );
            }
            EnrichedTelemetryEvent::ViewMutation {
                node_id,
                parent_id,
                frame,
                mutation_kind,
                ..
            } => {
                if *mutation_kind == 1 {
                    // Remove (mutation_kind 1): drop the node if present.
                    state.view_frames.retain(|vf| vf.node_id != *node_id);
                } else {
                    // Create/update: record node presence (and its parent link so
                    // the DevTools can rebuild the hierarchy). The rect is `None`
                    // when the host cannot measure geometry (the host crate is
                    // Android-free and drives in-memory adapter views); we still
                    // track the node so the component tree shows the live graph.
                    let entry = ViewFrame {
                        node_id: *node_id,
                        parent_id: *parent_id,
                        frame: frame.clone(),
                    };
                    if let Some(existing) = state
                        .view_frames
                        .iter_mut()
                        .find(|vf| vf.node_id == *node_id)
                    {
                        *existing = entry;
                    } else {
                        state.view_frames.push(entry);
                    }
                }
            }
            EnrichedTelemetryEvent::HandlerInvocation { is_start: true, .. } => {
                // A running handler implies the VM is mid-execution; a finished
                // handler with no pending start leaves pause state unchanged.
                state.paused = false;
            }
            // Future (non-exhaustive) variants: ignored for reconstruction.
            _ => {}
        }
    }
    state
}

/// Inserts or updates `value` for `key` in `vec`, preserving order.
fn upsert<K: PartialEq, V: Clone>(vec: &mut Vec<(K, V)>, key: K, value: V) {
    if let Some(slot) = vec.iter_mut().find(|(k, _)| *k == key) {
        slot.1 = value;
    } else {
        vec.push((key, value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flux_syntax::{EffectId, NodeId, SignalId};

    fn vm_step(offset: u32, reg0: i64) -> EnrichedTelemetryEvent {
        EnrichedTelemetryEvent::VmStep {
            bytecode_offset: offset,
            opcode: 0x02,
            registers: Box::new(std::array::from_fn(|i| {
                if i == 0 {
                    Value::Int(reg0)
                } else {
                    Value::Null
                }
            })),
            gas_remaining: 50,
            source_span: None,
        }
    }

    fn signal_write(id: u32, value: i64) -> EnrichedTelemetryEvent {
        EnrichedTelemetryEvent::SignalWrite {
            signal_id: SignalId::from(id),
            old_value: Value::Null,
            new_value: Value::Int(value),
            triggered_effect_ids: vec![EffectId::from(0u32)],
            source_span: None,
        }
    }

    fn signal_write_with_effects(id: u32, value: i64, effects: &[u32]) -> EnrichedTelemetryEvent {
        EnrichedTelemetryEvent::SignalWrite {
            signal_id: SignalId::from(id),
            old_value: Value::Null,
            new_value: Value::Int(value),
            triggered_effect_ids: effects.iter().map(|e| EffectId::from(*e)).collect(),
            source_span: None,
        }
    }

    fn view_layout(node: u32, w: f64) -> EnrichedTelemetryEvent {
        EnrichedTelemetryEvent::ViewMutation {
            node_id: NodeId::from(node),
            native_view_id: 0,
            parent_id: NodeId::from(0u32),
            mutation_kind: 3,
            frame: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: w,
                height: w,
            }),
            source_span: None,
        }
    }

    #[test]
    fn replay_updates_registers_and_ip() {
        let events = vec![vm_step(10, 7), vm_step(14, 9)];
        let state = reconstruct_state(&ReconstructedState::base(), &events);
        assert_eq!(state.bytecode_offset, Some(14));
        assert_eq!(state.opcode, Some(0x02));
        assert_eq!(state.gas_remaining, Some(50));
        assert_eq!(state.registers[0], Value::Int(9));
    }

    #[test]
    fn replay_accumulates_signals() {
        let events = vec![
            signal_write(1, 100),
            signal_write(2, 200),
            signal_write(1, 150),
        ];
        let state = reconstruct_state(&ReconstructedState::base(), &events);
        // Signal 1 was written twice; the last value wins.
        assert_eq!(state.signals.len(), 2);
        let s1 = state
            .signals
            .iter()
            .find(|(id, _)| *id == SignalId::from(1u32))
            .map(|(_, v)| v.clone())
            .unwrap();
        assert_eq!(s1, Value::Int(150));
    }

    #[test]
    fn replay_tracks_signal_dependency_edges() {
        let events = vec![
            signal_write(1, 100),
            signal_write(2, 200),
            // A write to signal 1 triggers effects 4 and 5 (they read signal 1).
            signal_write_with_effects(1, 150, &[4, 5]),
        ];
        let state = reconstruct_state(&ReconstructedState::base(), &events);
        let edges_1 = state
            .signal_edges
            .iter()
            .find(|(id, _)| *id == SignalId::from(1u32))
            .map(|(_, e)| e.clone())
            .unwrap_or_default();
        assert_eq!(edges_1, vec![EffectId::from(4u32), EffectId::from(5u32)]);
        // Signal 2 was written by the `signal_write` helper, which carries one
        // dependent effect (id 0).
        assert!(
            state
                .signal_edges
                .iter()
                .find(|(id, _)| *id == SignalId::from(2u32))
                .map(|(_, e)| e == &vec![EffectId::from(0u32)])
                .unwrap_or(false)
        );
    }

    #[test]
    fn replay_keeps_per_signal_reader_sets() {
        // Each signal is its own graph node: its reader set must not bleed into
        // another signal's edge. Pins the "node per signal, edge per dependency"
        // contract from FLUX-058 user story 2.
        let events = vec![
            signal_write_with_effects(1, 10, &[1, 2]),
            signal_write_with_effects(3, 30, &[7, 8, 9]),
            signal_write_with_effects(1, 11, &[1, 2, 3]),
        ];
        let state = reconstruct_state(&ReconstructedState::base(), &events);
        // Latest write to signal 1 carries readers [1,2,3].
        let readers_1 = state
            .signal_edges
            .iter()
            .find(|(id, _)| *id == SignalId::from(1u32))
            .map(|(_, e)| e.clone())
            .unwrap();
        assert_eq!(
            readers_1,
            vec![
                EffectId::from(1u32),
                EffectId::from(2u32),
                EffectId::from(3u32)
            ]
        );
        // Signal 3 keeps its own, independent reader set.
        let readers_3 = state
            .signal_edges
            .iter()
            .find(|(id, _)| *id == SignalId::from(3u32))
            .map(|(_, e)| e.clone())
            .unwrap();
        assert_eq!(
            readers_3,
            vec![
                EffectId::from(7u32),
                EffectId::from(8u32),
                EffectId::from(9u32)
            ]
        );
    }

    #[test]
    fn replay_tracks_view_frames_and_removal() {
        let mut events = vec![view_layout(5, 10.0), view_layout(6, 20.0)];
        events.push(EnrichedTelemetryEvent::ViewMutation {
            node_id: NodeId::from(5u32),
            native_view_id: 0,
            parent_id: NodeId::from(0u32),
            mutation_kind: 1, // Remove
            frame: None,
            source_span: None,
        });
        let state = reconstruct_state(&ReconstructedState::base(), &events);
        assert_eq!(state.view_frames.len(), 1);
        assert!(
            state
                .view_frames
                .iter()
                .all(|vf| vf.node_id != NodeId::from(5u32))
        );
    }

    #[test]
    fn replay_is_deterministic() {
        let events = vec![vm_step(2, 3), signal_write(1, 42)];
        let a = reconstruct_state(&ReconstructedState::base(), &events);
        let b = reconstruct_state(&ReconstructedState::base(), &events);
        assert_eq!(a, b);
    }

    #[test]
    fn partial_replay_from_base_is_stable() {
        // Reconstructing up to index 1 from base must equal replaying only the
        // first event.
        let events = [vm_step(2, 3), signal_write(1, 42), vm_step(8, 9)];
        let full = reconstruct_state(&ReconstructedState::base(), &events[..2]);
        let partial = reconstruct_state(&ReconstructedState::base(), &events[..1]);
        assert_eq!(partial.bytecode_offset, Some(2));
        assert_eq!(full.bytecode_offset, Some(2));
        assert_eq!(full.signals.len(), 1);
    }
}
