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
    DebugCommand, DebugCommandFrame, EnrichedTelemetryEvent, EnrichedTelemetryFrame,
    FRAME_DEBUG_COMMAND, FRAME_TELEMETRY, Rect, Registers, TelemetryEvent, TelemetryFrame,
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
            component_name: "Row".to_string(),
        },
        TelemetryEvent::ViewMutation {
            node_id: NodeId::from(8u32),
            native_view_id: 0x1,
            parent_id: NodeId::from(2u32),
            mutation_kind: 1, // Remove
            frame: None,
            component_name: "Button".to_string(),
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
        // FLUX-060 network inspector: an outbound GET plus its resolved response.
        TelemetryEvent::NetworkRequest {
            request_id: 1,
            method: "GET".to_string(),
            url: "https://api.example.com/users".to_string(),
            body: None,
            capability_id: 14,
        },
        TelemetryEvent::NetworkResponse {
            request_id: 1,
            status_code: 200,
            latency_ms: 42,
            body: Some("{\"ok\":true}".to_string()),
            result_kind: 1,
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

// ── FLUX-060 network inspector: NetworkRequest / NetworkResponse codec ──────

#[test]
fn network_events_carry_expected_wire_tags() {
    // D.12 telemetry event tags: VmStep=0x01 … HandlerInvocation=0x04,
    // NetworkRequest=0x05, NetworkResponse=0x06, PerfRecord=0x07. The tags are
    // the contract a host/DevTools pair must agree on, so assert them directly.
    let req = TelemetryFrame {
        version: flux_ir_serde::PROTOCOL_VERSION,
        event_count: 1,
        events: vec![TelemetryEvent::NetworkRequest {
            request_id: 7,
            method: "POST".to_string(),
            url: "https://x.test/v1".to_string(),
            body: Some("q=1".to_string()),
            capability_id: 14,
        }],
    };
    let bytes = req.to_bytes();
    // event starts at offset 8 (after header + event_count), length at 8..12,
    // tag at offset 12.
    assert_eq!(bytes[12], 0x05, "NetworkRequest tag");

    let resp = TelemetryFrame {
        version: flux_ir_serde::PROTOCOL_VERSION,
        event_count: 1,
        events: vec![TelemetryEvent::NetworkResponse {
            request_id: 7,
            status_code: 404,
            latency_ms: 9,
            body: None,
            result_kind: 2,
        }],
    };
    let bytes = resp.to_bytes();
    assert_eq!(bytes[12], 0x06, "NetworkResponse tag");
}

#[test]
fn network_events_round_trip_enriched() {
    // The DevTools consumes the *enriched* frame; the server attaches a span via
    // enrich_with_span, which must NOT corrupt the network fields.
    let raw = TelemetryEvent::NetworkRequest {
        request_id: 3,
        method: "GET".to_string(),
        url: "https://api.test/me".to_string(),
        body: None,
        capability_id: 14,
    };
    let enriched = flux_ir_serde::enrich_with_span(raw.clone(), Some(Span::new(0, 2, 9)));
    let frame = EnrichedTelemetryFrame {
        version: flux_ir_serde::PROTOCOL_VERSION,
        event_count: 1,
        events: vec![enriched],
    };
    let bytes = frame.to_bytes();
    let decoded = EnrichedTelemetryFrame::from_bytes(&bytes).expect("enriched network decodes");
    match &decoded.events[0] {
        EnrichedTelemetryEvent::NetworkRequest {
            request_id,
            method,
            url,
            capability_id,
            source_span,
            ..
        } => {
            assert_eq!(*request_id, 3);
            assert_eq!(method, "GET");
            assert_eq!(url, "https://api.test/me");
            assert_eq!(*capability_id, 14);
            assert_eq!(*source_span, Some(Span::new(0, 2, 9)));
        }
        other => panic!("expected NetworkRequest, got {other:?}"),
    }
}

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

/// FLUX-060 follow-up: the Android/iOS hosts emit `NetworkRequest`/`NetworkResponse`
/// around the `Http` capability. This test pins the **exact** byte sequence those
/// host encoders produce (generated from the identical `Telemetry.swift` /
/// `Telemetry.kt` encode arms) so the Rust decoder (`TelemetryFrame::from_bytes`)
/// agrees with the hosts byte-for-byte. If a host changes its encoding, this
/// canonical array must change in lockstep — it is the wire contract.
#[test]
fn host_network_telemetry_decodes_to_network_events() {
    // MAGIC(58 55 5c 46) version(02) kind(10) event_count(02 00)
    // Event 1 = NetworkRequest(requestId=7 GET https://api.example.com/users, no body, cap=14)
    // Event 2 = NetworkResponse(requestId=7 status=200 latency=42 body=`{"ok":true}` kind=1)
    let bytes: &[u8] = &[
        0x58, 0x55, 0x5c, 0x46, 0x02, 0x10, 0x02, 0x00, // header + event_count=2
        // NetworkRequest event
        0x2e, 0x00, 0x00, 0x00, // length prefix = 0x2e (46)
        0x05, // tag
        0x07, 0x00, 0x00, 0x00, // request_id = 7
        0x03, 0x00, 0x47, 0x45, 0x54, // method = "GET" (len 3)
        0x1d, 0x00, // url len = 29
        0x68, 0x74, 0x74, 0x70, 0x73, 0x3a, 0x2f, 0x2f, 0x61, 0x70, 0x69, 0x2e, 0x65, 0x78, 0x61,
        0x6d, 0x70, 0x6c, 0x65, 0x2e, 0x63, 0x6f, 0x6d, 0x2f, 0x75, 0x73, 0x65, 0x72, 0x73,
        0x00, // no body
        0x0e, 0x00, 0x00, 0x00, // capability_id = 14
        // NetworkResponse event
        0x1a, 0x00, 0x00, 0x00, // length prefix = 0x1a (26)
        0x06, // tag
        0x07, 0x00, 0x00, 0x00, // request_id = 7
        0xc8, 0x00, // status_code = 200
        0x2a, 0x00, 0x00, 0x00, // latency_ms = 42
        0x01, // body present
        0x0b, 0x00, // body len = 11
        0x7b, 0x22, 0x6f, 0x6b, 0x22, 0x3a, 0x74, 0x72, 0x75, 0x65,
        0x7d, // body = `{"ok":true}`
        0x01, // result_kind = 1 (Ready)
    ];
    let frame = TelemetryFrame::from_bytes(bytes).expect("host network frame decodes");
    assert_eq!(frame.event_count, 2);

    match &frame.events[0] {
        TelemetryEvent::NetworkRequest {
            request_id,
            method,
            url,
            body,
            capability_id,
        } => {
            assert_eq!(*request_id, 7);
            assert_eq!(method, "GET");
            assert_eq!(url, "https://api.example.com/users");
            assert_eq!(body, &None);
            assert_eq!(*capability_id, 14);
        }
        other => panic!("expected NetworkRequest, got {other:?}"),
    }

    match &frame.events[1] {
        TelemetryEvent::NetworkResponse {
            request_id,
            status_code,
            latency_ms,
            body,
            result_kind,
        } => {
            assert_eq!(*request_id, 7);
            assert_eq!(*status_code, 200);
            assert_eq!(*latency_ms, 42);
            assert_eq!(body.as_deref(), Some("{\"ok\":true}"));
            assert_eq!(*result_kind, 1);
        }
        other => panic!("expected NetworkResponse, got {other:?}"),
    }
}
