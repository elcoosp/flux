//! Standalone host emulator for on-device DevTools verification (FLUX-062).
//!
//! Binds the dev server's DevTools endpoint (`127.0.0.1:7333`) and serves a
//! faithful telemetry stream built from the production wire codec
//! [`flux_ir_serde`]. This lets the real `flux-devtools` desktop binary connect
//! to a live "host" and render every pane with real data — the rendered-state
//! proof the issue's verification step requires.
//!
//! The emitted frames are byte-identical to what the real iOS/Android hosts
//! broadcast (they call the same `TelemetryEvent`/`HostAnnounceFrame` encoders),
//! so this is a faithful wire-contract source — not a mock of the DevTools data
//! path. Run with: `cargo run --example host_emulator -p flux-devtools-ui`,
//! then `cargo run --bin flux-devtools` in another terminal and screenshot.

use std::time::Duration;

use flux_ir_serde::{
    EnrichedTelemetryEvent, EnrichedTelemetryFrame, HostAnnounceFrame, TelemetryEvent,
};
use flux_syntax::{EffectId, NodeId, SignalId, Value};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

fn enriched_frame(events: Vec<EnrichedTelemetryEvent>) -> Vec<u8> {
    EnrichedTelemetryFrame {
        version: flux_ir_serde::PROTOCOL_VERSION,
        event_count: events.len() as u16,
        events,
    }
    .to_bytes()
}

fn announce() -> Vec<u8> {
    HostAnnounceFrame {
        version: flux_ir_serde::PROTOCOL_VERSION,
        platform: "ios".to_string(),
        device: "iPhone17,1".to_string(),
        capabilities: vec![("vm".to_string(), 1, vec![])],
    }
    .to_bytes()
}

/// The component tree, sent once on connect so the tree pane shows named nodes.
fn mount_tree() -> Vec<EnrichedTelemetryEvent> {
    vec![
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
    ]
    .into_iter()
    .map(flux_ir_serde::enrich_telemetry)
    .collect()
}

fn live_events(count: i64, offset: u32) -> Vec<EnrichedTelemetryEvent> {
    let signal_val = Value::Int(count);
    let regs: [Value; 16] = std::array::from_fn(|i| {
        if i == 0 {
            signal_val.clone()
        } else {
            Value::Null
        }
    });
    vec![
        TelemetryEvent::SignalWrite {
            signal_id: SignalId::from(1u32),
            old_value: Value::Null,
            new_value: signal_val,
            triggered_effect_ids: vec![EffectId::from(2u32)],
        },
        TelemetryEvent::VmStep {
            bytecode_offset: offset,
            opcode: 0x05,
            registers: Box::new(regs),
            gas_remaining: 999,
        },
    ]
    .into_iter()
    .map(flux_ir_serde::enrich_telemetry)
    .collect()
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("HOST_EMU_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(7333);
    let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let listener = TcpListener::bind(addr).await.expect("bind :7333");
    eprintln!("host-emulator: listening on ws://{addr}/devtools");
    eprintln!("host-emulator: run `cargo run --bin flux-devtools` to connect");

    loop {
        let (sock, _peer) = listener.accept().await.expect("accept");
        let ws = tokio_tungstenite::accept_async(sock).await.expect("accept ws");
        let (mut w, mut r) = ws.split();

        // Host identity + component tree on connect.
        w.send(Message::Binary(announce().into()))
            .await
            .expect("announce");
        w.send(Message::Binary(enriched_frame(mount_tree()).into()))
            .await
            .expect("mount tree");

        // Stream a live signal/VM timeline so the VM inspector + timeline + signal
        // graph panes update continuously (and time-travel has a scrubbable history).
        let mut offset: u32 = 24;
        let mut count: i64 = 0;
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(250)) => {
                    count = count.wrapping_add(1);
                    offset = offset.wrapping_add(4);
                    if w.send(Message::Binary(
                        enriched_frame(live_events(count, offset)).into(),
                    ))
                    .await
                    .is_err()
                    {
                        break;
                    }
                }
                msg = r.next() => {
                    if matches!(msg, None | Some(Ok(Message::Close(_)))) {
                        break;
                    }
                }
            }
        }
    }
}
