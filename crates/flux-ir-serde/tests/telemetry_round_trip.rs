//! Tests for the DevTools `Telemetry` / `DebugCommand` wire codec (ADR-0039).
//!
//! Two layers are exercised:
//! 1. **Round-trip** — every event/command encodes and decodes back to an
//!    equal value (the encode/decode are inverses).
//! 2. **Conformance** — the exact byte layout (magic, version, kind byte, field
//!    offsets, tags) is asserted, because a green round-trip does NOT prove the
//!    bytes match the normative Appendix D §D.12 layout (AGENTS.md §3.3, the
//!    flux-rust-crate-dev skill's "GREEN ≠ CONFORMANT" gate).

use flux_ir_serde::{
    DebugCommand, DebugCommandFrame, EnrichedTelemetryEvent, FRAME_DEBUG_COMMAND, FRAME_TELEMETRY,
    Rect, Registers, TelemetryEvent, TelemetryFrame,
};
use flux_syntax::{EffectId, NodeId, SignalId, Span, Value};

fn regs(v: Value) -> Registers {
    Box::new(core::array::from_fn(|_| v.clone()))
}

// ── TelemetryEvent round trips ─────────────────────────────────────────────

fn sample_events() -> Vec<TelemetryEvent> {
    vec![
        TelemetryEvent::VmStep {
            bytecode_offset: 42,
            opcode: 0x10,
            registers: regs(Value::Int(0)),
            gas_remaining: 99000,
        },
        TelemetryEvent::SignalWrite {
            signal_id: SignalId::from(3u32),
            old_value: Value::Int(0),
            new_value: Value::Int(1),
            triggered_effect_ids: vec![EffectId::from(1u32), EffectId::from(2u32)],
        },
        TelemetryEvent::ViewMutation {
            node_id: NodeId::from(7u32),
            native_view_id: 0xDEAD_BEEF,
            parent_id: NodeId::from(2u32),
            mutation_kind: 3, // Layout
            frame: Some(Rect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            }),
        },
        TelemetryEvent::ViewMutation {
            node_id: NodeId::from(8u32),
            native_view_id: 0x1,
            parent_id: NodeId::from(2u32),
            mutation_kind: 1, // Remove
            frame: None,
        },
        TelemetryEvent::HandlerInvocation {
            handler_id: 5,
            is_start: true,
            gas_used: None,
        },
        TelemetryEvent::HandlerInvocation {
            handler_id: 5,
            is_start: false,
            gas_used: Some(12),
        },
    ]
}

#[test]
fn telemetry_event_round_trips() {
    for event in sample_events() {
        // Encode a single event in isolation through a 1-event frame.
        let frame = TelemetryFrame {
            version: flux_ir_serde::PROTOCOL_VERSION,
            event_count: 1,
            events: vec![event.clone()],
        };
        let buf = frame.to_bytes();
        let decoded = TelemetryFrame::from_bytes(&buf).expect("telemetry frame decodes");
        assert_eq!(decoded.event_count, 1);
        assert_eq!(decoded.events.len(), 1);
        assert_eq!(decoded.events[0], event, "event round-trip mismatch");
    }
}

#[test]
fn telemetry_frame_multi_event_round_trip() {
    let events = sample_events();
    let frame = TelemetryFrame {
        version: flux_ir_serde::PROTOCOL_VERSION,
        event_count: events.len() as u16,
        events: events.clone(),
    };
    let bytes = frame.to_bytes();
    let decoded = TelemetryFrame::from_bytes(&bytes).expect("multi-event frame decodes");
    assert_eq!(decoded.event_count, events.len() as u16);
    assert_eq!(decoded.events, events);
}

#[test]
fn telemetry_rejects_non_telemetry_kind() {
    // A Delta frame's bytes must not decode as Telemetry.
    let delta =
        flux_ir_serde::Frame::delta(0, 0, &[flux_syntax::Patch::Remove { id: 1 }], &[], &[], &[]);
    let bytes = delta.to_bytes();
    assert!(TelemetryFrame::from_bytes(&bytes).is_none());
}

// ── DebugCommand round trips ───────────────────────────────────────────────

#[test]
fn debug_command_round_trips() {
    let commands = vec![
        DebugCommand::Pause,
        DebugCommand::Resume,
        DebugCommand::Step,
        DebugCommand::SetBreakpoint {
            bytecode_offset: 256,
        },
        DebugCommand::ClearBreakpoint {
            bytecode_offset: 256,
        },
        DebugCommand::RequestSnapshot,
    ];
    for command in commands {
        let frame = DebugCommandFrame::new(7, command.clone());
        let bytes = frame.to_bytes();
        let decoded = DebugCommandFrame::from_bytes(&bytes).expect("debug command decodes");
        assert_eq!(decoded.command_id, 7);
        assert_eq!(decoded.command, command, "command round-trip mismatch");
    }
}

#[test]
fn debug_command_rejects_non_debug_kind() {
    let delta =
        flux_ir_serde::Frame::delta(0, 0, &[flux_syntax::Patch::Remove { id: 1 }], &[], &[], &[]);
    let bytes = delta.to_bytes();
    assert!(DebugCommandFrame::from_bytes(&bytes).is_none());
}

// ── Conformance: exact Appendix D §D.12 byte layout ────────────────────────

fn magic_bytes() -> [u8; 4] {
    flux_ir_serde::MAGIC.to_le_bytes()
}

#[test]
fn telemetry_frame_header_matches_d12() {
    // D.12 Telemetry: MAGIC(4) version(1) kind(0x10) event_count(2) [events].
    let frame = TelemetryFrame {
        version: flux_ir_serde::PROTOCOL_VERSION,
        event_count: 1,
        events: vec![TelemetryEvent::VmStep {
            bytecode_offset: 0,
            opcode: 0x01,
            registers: regs(Value::Null),
            gas_remaining: 0,
        }],
    };
    let bytes = frame.to_bytes();
    assert_eq!(&bytes[0..4], &magic_bytes());
    assert_eq!(bytes[4], flux_ir_serde::PROTOCOL_VERSION);
    assert_eq!(bytes[5], FRAME_TELEMETRY);
    // event_count u16 at offset 6.
    assert_eq!(u16::from_le_bytes([bytes[6], bytes[7]]), 1, "event_count");
    // First event: length-prefixed union. length u32 at offset 8.
    let event_len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    // After the length prefix, a VmStep tag 0x01.
    assert_eq!(bytes[12], 0x01, "VmStep event tag");
    // VmStep body: bytecode_offset u32, opcode u8, 16×Value, gas u32.
    // bytecode_offset at offset 13.
    let offset = u32::from_le_bytes([bytes[13], bytes[14], bytes[15], bytes[16]]);
    assert_eq!(offset, 0);
    assert_eq!(bytes[17], 0x01, "opcode");
    // The length prefix counts the body *after* it (tag + fields), not itself:
    // 1 (tag) + 4 (offset) + 1 (opcode) + 16×1 (Null regs) + 4 (gas) = 26.
    let expected = 1 + 4 + 1 + 16 + 4;
    assert_eq!(event_len, expected, "VmStep wire length");
}

#[test]
fn telemetry_frame_event_is_length_prefixed() {
    // Two events: the second starts exactly `first_len` bytes after the
    // first length-prefix, proving the decoder can locate each event.
    let events = vec![
        TelemetryEvent::VmStep {
            bytecode_offset: 1,
            opcode: 0x02,
            registers: regs(Value::Int(7)),
            gas_remaining: 100,
        },
        TelemetryEvent::SignalWrite {
            signal_id: SignalId::from(9u32),
            old_value: Value::Null,
            new_value: Value::Bool(true),
            triggered_effect_ids: vec![],
        },
    ];
    let frame = TelemetryFrame {
        version: flux_ir_serde::PROTOCOL_VERSION,
        event_count: 2,
        events,
    };
    let bytes = frame.to_bytes();
    let first_len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    // Second event length-prefix begins after the first event's 4-byte length
    // prefix plus its `first_len` body: offset 8 + 4 + first_len.
    let second_start = 8 + 4 + first_len;
    let second_len = u32::from_le_bytes([
        bytes[second_start],
        bytes[second_start + 1],
        bytes[second_start + 2],
        bytes[second_start + 3],
    ]) as usize;
    // SignalWrite tag 0x02 immediately follows its length prefix.
    assert_eq!(bytes[second_start + 4], 0x02, "SignalWrite event tag");
    assert!(second_len > 0, "second event has a non-zero body");
}

#[test]
fn debug_command_header_matches_d12() {
    // D.12 DebugCommand: MAGIC(4) version(1) kind(0x11) command_id(4)
    // payload_len(2) payload.
    let frame = DebugCommandFrame::new(
        0xCAFE,
        DebugCommand::SetBreakpoint {
            bytecode_offset: 1234,
        },
    );
    let bytes = frame.to_bytes();
    assert_eq!(&bytes[0..4], &magic_bytes());
    assert_eq!(bytes[4], flux_ir_serde::PROTOCOL_VERSION);
    assert_eq!(bytes[5], FRAME_DEBUG_COMMAND);
    assert_eq!(
        u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]),
        0xCAFE,
        "command_id"
    );
    // payload_len u16 at offset 10.
    let payload_len = u16::from_le_bytes([bytes[10], bytes[11]]) as usize;
    // Payload: tag 0x04 (SetBreakpoint) + u32 offset = 5 bytes.
    assert_eq!(payload_len, 5, "SetBreakpoint payload length");
    assert_eq!(bytes[12], 0x04, "SetBreakpoint tag");
    let bp = u32::from_le_bytes([bytes[13], bytes[14], bytes[15], bytes[16]]);
    assert_eq!(bp, 1234);
}

#[test]
fn debug_command_request_snapshot_payload_is_minimal() {
    let frame = DebugCommandFrame::new(1, DebugCommand::RequestSnapshot);
    let bytes = frame.to_bytes();
    let payload_len = u16::from_le_bytes([bytes[10], bytes[11]]) as usize;
    // RequestSnapshot is just the tag byte.
    assert_eq!(payload_len, 1);
    assert_eq!(bytes[12], 0x06, "RequestSnapshot tag");
}

// ── EnrichedTelemetryEvent is a value type (used by Phase 3/6) ──────────────

#[test]
fn enriched_event_carries_source_span() {
    let event = EnrichedTelemetryEvent::VmStep {
        bytecode_offset: 4,
        opcode: 0x09,
        registers: regs(Value::Null),
        gas_remaining: 1,
        source_span: Some(Span::new(0, 10, 20)),
    };
    match event {
        EnrichedTelemetryEvent::VmStep { source_span, .. } => {
            assert_eq!(source_span, Some(Span::new(0, 10, 20)));
        }
        _ => panic!("expected VmStep"),
    }
    // Events with no resolvable span carry None.
    let no_span = EnrichedTelemetryEvent::SignalWrite {
        signal_id: SignalId::from(0u32),
        old_value: Value::Null,
        new_value: Value::Null,
        triggered_effect_ids: vec![],
        source_span: None,
    };
    assert!(matches!(
        no_span,
        EnrichedTelemetryEvent::SignalWrite {
            source_span: None,
            ..
        }
    ));
}
