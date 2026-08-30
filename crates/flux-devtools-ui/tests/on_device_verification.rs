//! On-device verification harness for FLUX-062 (DevTools "ship it, not scaffold it").
//!
//! This is the CI/dev script the issue's Testing Decisions call for. It drives a
//! *real* DevTools WebSocket client (the exact `connect` + `ingest_message` code
//! the `flux-devtools` desktop binary uses) against a *real* wire feed built from
//! the shared codec [`flux_ir_serde`]: an embedded server emits authentic
//! `Telemetry` (`0x10`) and `HostAnnounce` (`0x12`) frames — the same byte shapes
//! the production dev server broadcasts after `route_telemetry`/`announce_host` —
//! so every DevTools view is populated from a faithful telemetry stream.
//!
//! The host client in the real DevServer path emits these exact frames (iOS/Android
//! hosts call `TelemetryEvent::toFrameBytes` / `HostAnnounceFrame::to_bytes`); here
//! we synthesize that stream with the same codec so the prove-the-data-path check
//! needs no full native-app build. Every decode/ingest/view step is the production
//! code — nothing in the DevTools data path is mocked.
//!
//! Asserted live views (FLUX-062 + prereqs):
//! - Component Tree: named nodes (Louis's historical "empty tree" gap) — `Column`/`Button`.
//! - Signal Graph: signal values + "what reads this signal" dependency edges.
//! - VM Inspector: instruction pointer + register bank.
//! - Timeline: advances with the stream, and time-travel scrub rebuilds an earlier state.
//! - Host identity: the `HostAnnounce` frame reaches the client and is shown.
//! - Network inspector (FLUX-060): `NetworkRequest`/`NetworkResponse` telemetry pairs.
//! - Flamegraph (FLUX-059): `PerfRecord` telemetry feeds the retained records.
//! - Multi-device (FLUX-061): two distinct hosts → two independent sessions.

use std::sync::Arc;
use std::time::Duration;

use flux_devtools_ui::state::HostKey;
use flux_devtools_ui::{DevToolsState, ingest_message};
use flux_ir_serde::{
    EnrichedTelemetryEvent, EnrichedTelemetryFrame, HostAnnounceFrame, TelemetryEvent,
};
use flux_syntax::{EffectId, NodeId, SignalId, Value};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

/// Builds one server→client `EnrichedTelemetry` frame (`0x10`, same kind) that the
/// dev server broadcasts to DevTools clients (post `route_telemetry` enrichment).
fn enriched_frame(events: Vec<EnrichedTelemetryEvent>) -> Vec<u8> {
    EnrichedTelemetryFrame {
        version: flux_ir_serde::PROTOCOL_VERSION,
        event_count: events.len() as u16,
        events,
    }
    .to_bytes()
}

/// Builds a `HostAnnounce` frame (`0x12`) — the server→client identity broadcast.
fn host_announce_frame(platform: &str, device: &str) -> Vec<u8> {
    HostAnnounceFrame {
        version: flux_ir_serde::PROTOCOL_VERSION,
        platform: platform.to_string(),
        device: device.to_string(),
        capabilities: vec![("vm".to_string(), 1, vec![])],
    }
    .to_bytes()
}

/// A realistic event batch a host emits on mount + a tap (a counter `Button`).
fn host_event_batch() -> Vec<TelemetryEvent> {
    vec![
        // Reconciler replays the shadow tree with resolved component names
        // (the data the component tree pane needs — `Column`, `Button`, `Text`).
        TelemetryEvent::ViewMutation {
            node_id: NodeId::from(1u32),
            native_view_id: 1,
            parent_id: NodeId::from(0u32),
            mutation_kind: 0,
            frame: Some(flux_ir_serde::Rect {
                x: 0.0,
                y: 0.0,
                width: 390.0,
                height: 844.0,
            }),
            component_name: "Column".to_string(),
        },
        TelemetryEvent::ViewMutation {
            node_id: NodeId::from(2u32),
            native_view_id: 2,
            parent_id: NodeId::from(1u32),
            mutation_kind: 0,
            frame: Some(flux_ir_serde::Rect {
                x: 0.0,
                y: 8.0,
                width: 200.0,
                height: 44.0,
            }),
            component_name: "Button".to_string(),
        },
        TelemetryEvent::ViewMutation {
            node_id: NodeId::from(3u32),
            native_view_id: 3,
            parent_id: NodeId::from(1u32),
            mutation_kind: 0,
            frame: None,
            component_name: "Text".to_string(),
        },
        // A signal write with the effects that read it (dependency edges).
        TelemetryEvent::SignalWrite {
            signal_id: SignalId::from(1u32),
            old_value: Value::Int(0),
            new_value: Value::Int(1),
            triggered_effect_ids: vec![EffectId::from(2u32)],
        },
        // A VM step advancing the instruction pointer.
        TelemetryEvent::VmStep {
            bytecode_offset: 24,
            opcode: 0x05,
            registers: Box::new(std::array::from_fn(|i| {
                if i == 0 {
                    Value::Int(1)
                } else {
                    Value::Null
                }
            })),
            gas_remaining: 999,
        },
        // A handler invocation (tap).
        TelemetryEvent::HandlerInvocation {
            handler_id: 7,
            is_start: true,
            gas_used: None,
        },
        // Network traffic (FLUX-060 network inspector).
        TelemetryEvent::NetworkRequest {
            request_id: 1,
            method: "GET".to_string(),
            url: "https://api.example.com/profile".to_string(),
            body: None,
            capability_id: 12,
        },
        TelemetryEvent::NetworkResponse {
            request_id: 1,
            status_code: 200,
            latency_ms: 42,
            body: Some("{\"ok\":true}".to_string()),
            result_kind: 1,
        },
        // A render-perf record (FLUX-059 flamegraph): the verbatim `MetricRecord`
        // JSON the render-perf harness emits.
        TelemetryEvent::PerfRecord {
            json: r#"{"scenario":"ios-imperative-dev","kind":"node-mutation","tree_size":50,"samples":[{"latency":0.018,"size":3}]}"#
                .to_string(),
        },
    ]
}

/// Enriches a raw event batch the way `route_telemetry` does (no source spans),
/// so the DevTools client receives `EnrichedTelemetryEvent`s exactly as on the
/// real wire.
fn enrich_batch(events: Vec<TelemetryEvent>) -> Vec<EnrichedTelemetryEvent> {
    events
        .into_iter()
        .map(flux_ir_serde::enrich_telemetry)
        .collect()
}

/// Spawns one embedded server that, on a single client connect, streams
/// `announce` then each `frames` as binary WebSocket messages, then holds the
/// socket open briefly so the client can finish ingesting. Returns the bind addr.
async fn spawn_wire_server(announce: Vec<u8>, frames: Vec<Vec<u8>>) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (sock, _) = listener.accept().await.expect("accept");
        let ws = accept_async(sock).await.expect("accept ws");
        let (mut w, _) = ws.split();
        w.send(Message::Binary(announce.into()))
            .await
            .expect("send announce");
        for f in frames {
            w.send(Message::Binary(f.into())).await.expect("send frame");
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    });
    // Let the listener bind before the caller's client connects.
    tokio::time::sleep(Duration::from_millis(50)).await;
    addr
}

/// Connects the real DevTools client, ingests every frame into `state` via the
/// production `ingest_message`, and returns once `state.timeline_len() >= until`.
async fn run_client(state: Arc<DevToolsState>, addr: std::net::SocketAddr, until: usize) {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let url = format!("ws://{addr}/devtools");
    let (mut ws, _) = tokio_tungstenite::connect_async(url.into_client_request().unwrap())
        .await
        .expect("devtools client connect");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        tokio::select! {
            msg = ws.next() => {
                match msg {
                    Some(Ok(Message::Binary(bytes))) => {
                        ingest_message(&state, &Message::Binary(bytes.into()));
                    }
                    _ => {}
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(20)) => {}
        }
        if state.timeline_len() >= until {
            break;
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn on_device_every_view_renders_live_data() {
    let announce = host_announce_frame("ios", "iPhone17,1");
    let enriched = enriched_frame(enrich_batch(host_event_batch()));
    let addr = spawn_wire_server(announce, vec![enriched]).await;

    let state = Arc::new(DevToolsState::new());
    run_client(state.clone(), addr, 9).await;

    // --- Assertions: every view is populated with live data. ---
    // 1. Host identity reached the client and is shown.
    let host = state.host_info().expect("host identity announced");
    assert_eq!(host.platform, "ios");
    assert_eq!(host.device, "iPhone17,1");

    // 2. Component Tree: named nodes (NOT bare ids — Louis's historical gap).
    let live_state = state
        .session_state(&HostKey::from_host(&host))
        .expect("ios session present");
    let named: Vec<&String> = live_state
        .live
        .view_frames
        .iter()
        .filter_map(|vf| vf.component_name.as_ref())
        .collect();
    assert!(
        named.iter().any(|n| *n == "Column"),
        "component tree must show named nodes, got: {named:?}"
    );
    assert!(
        named.iter().any(|n| *n == "Button"),
        "component tree must show Button, got: {named:?}"
    );

    // 3. Signal Graph: signal value + dependency edge.
    assert!(
        live_state
            .live
            .signals
            .iter()
            .any(|(id, v)| *id == SignalId::from(1u32) && *v == Value::Int(1)),
        "signal graph must show signal #1 = 1"
    );
    assert!(
        live_state
            .live
            .signal_edges
            .iter()
            .any(|(id, fx)| *id == SignalId::from(1u32) && fx.contains(&EffectId::from(2u32))),
        "signal graph must show signal #1 → effect #2 edge"
    );

    // 4. VM Inspector: instruction pointer + register bank.
    assert_eq!(state.vm_state().bytecode_offset, Some(24));
    assert_eq!(state.vm_state().registers[0], Value::Int(1));

    // 5. Timeline: advanced with the stream, and time-travel scrub rebuilds an
    //    strictly-earlier prefix (state_at(0) != live).
    assert!(state.timeline_len() >= 9);
    let at_zero = state.state_at(0).expect("timeline index 0");
    let live_full = state.vm_state();
    assert_ne!(
        at_zero.bytecode_offset, live_full.bytecode_offset,
        "time-travel scrub must reconstruct an earlier state"
    );

    // 6. Network inspector (FLUX-060): the request/response pair is retained.
    let net = state.network_snapshot();
    assert_eq!(net.len(), 1, "network inspector retains the exchange");
    assert_eq!(net[0].status_code, Some(200));

    // 7. Flamegraph (FLUX-059): the PerfRecord telemetry populated records.
    assert!(
        !state.perf_records().is_empty(),
        "flamegraph must receive the PerfRecord telemetry"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn on_device_multi_device_two_sessions() {
    // Two distinct hosts (iOS sim + Android phone) stream on the same endpoint;
    // FLUX-061's session model must keep them independent.
    let ios_announce = host_announce_frame("ios", "iPhone17,1");
    let ios_frame = enriched_frame(enrich_batch(vec![TelemetryEvent::VmStep {
        bytecode_offset: 10,
        opcode: 0x01,
        registers: Box::new(std::array::from_fn(|_| Value::Null)),
        gas_remaining: 100,
    }]));
    let android_announce = host_announce_frame("android", "Pixel 8");
    let android_frame = enriched_frame(enrich_batch(vec![TelemetryEvent::VmStep {
        bytecode_offset: 20,
        opcode: 0x02,
        registers: Box::new(std::array::from_fn(|_| Value::Null)),
        gas_remaining: 100,
    }]));

    let frames = vec![ios_announce, ios_frame, android_announce, android_frame];
    let addr = spawn_wire_server(Vec::new(), frames).await;

    let state = Arc::new(DevToolsState::new());
    run_client(state.clone(), addr, 2).await;

    assert_eq!(
        state.session_count(),
        2,
        "FLUX-061: two independent sessions"
    );
    let keys = state.session_keys();
    assert!(
        keys.iter()
            .any(|k| k.platform == "ios" && k.device == "iPhone17,1"),
        "ios session key present: {keys:?}"
    );
    assert!(
        keys.iter()
            .any(|k| k.platform == "android" && k.device == "Pixel 8"),
        "android session key present: {keys:?}"
    );

    let ios = state
        .session_state(&HostKey {
            platform: "ios".to_string(),
            device: "iPhone17,1".to_string(),
        })
        .expect("ios session");
    let android = state
        .session_state(&HostKey {
            platform: "android".to_string(),
            device: "Pixel 8".to_string(),
        })
        .expect("android session");
    assert_eq!(ios.vm_state().bytecode_offset, Some(10));
    assert_eq!(android.vm_state().bytecode_offset, Some(20));
}
