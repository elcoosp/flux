//! WebSocket client for the DevTools channel (spec §5.3, §4.3).
//!
//! Connects to the dev server's `:7333` DevTools endpoint, decodes incoming
//! `Telemetry` frames into [`EnrichedTelemetryEvent`]s (enrichment happens
//! server-side, ADR-0039), and feeds them into [`DevToolsState`]. Outgoing
//! `DebugCommand` frames are serialized and sent to the server, which forwards
//! them to the host (spec §4.3).
//!
//! The decode/dispatch step ([`ingest_message`]) is pure and unit-tested; the
//! `connect` loop is the only async I/O path.

use std::sync::Arc;

use anyhow::Context as _;
use flux_ir_serde::{
    DebugCommand, DebugCommandFrame, EnrichedTelemetryEvent, EnrichedTelemetryFrame,
    enrich_telemetry,
};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

use crate::state::DevToolsState;

/// Default DevTools WebSocket port (spec §4.1).
pub const DEFAULT_DEVTOOLS_PORT: u16 = 7333;

/// Decodes one server message and applies it to `state`.
///
/// Only `Telemetry` frames advance the timeline; other message kinds (text
/// pings, close) are ignored. Malformed telemetry frames are logged and
/// skipped rather than crashing the client (AGENTS.md: never panic in prod).
///
/// The server enriches events with source spans before sending (Phase 3), so
/// the decoded payloads are [`EnrichedTelemetryEvent`]s. A raw `Telemetry`
/// frame (host → server shape) is also accepted and enriched client-side with
/// no span, so the same decoder handles both.
///
/// Returns the number of telemetry events ingested.
pub fn ingest_message(state: &DevToolsState, message: &Message) -> usize {
    let Message::Binary(bytes) = message else {
        return 0;
    };
    // Try the enriched (server → DevTools) frame first, then fall back to a raw
    // host-style frame enriched client-side.
    let events: Vec<EnrichedTelemetryEvent> = match EnrichedTelemetryFrame::from_bytes(bytes) {
        Some(frame) => frame.events,
        None => match flux_ir_serde::TelemetryFrame::from_bytes(bytes) {
            Some(frame) => frame.events.into_iter().map(enrich_telemetry).collect(),
            None => {
                // Not a telemetry frame: maybe the server is announcing the host
                // identity (platform/device) so the UI can show which device
                // streams. Apply it and bail (no timeline events this frame).
                if let Some(announce) = flux_ir_serde::HostAnnounceFrame::from_bytes(bytes) {
                    state.set_host(crate::state::HostInfo {
                        platform: announce.platform,
                        device: announce.device,
                        capabilities: announce.capabilities,
                    });
                    return 0;
                }
                tracing::warn!(bytes = bytes.len(), "dropping unparseable telemetry frame");
                return 0;
            }
        },
    };
    let count = events.len();
    for event in events {
        state.handle_telemetry(event);
    }
    count
}

/// Serializes a [`DebugCommand`] into a `DebugCommand` frame for sending to the
/// server (which forwards it to the host). `command_id` is echoed by the host.
#[must_use]
pub fn encode_command(command_id: u32, command: DebugCommand) -> Vec<u8> {
    DebugCommandFrame::new(command_id, command).to_bytes()
}

/// Connects to the dev server's DevTools endpoint and returns the live
/// WebSocket stream. Callers drive [`ingest_message`] on each binary message
/// (or use [`run_ingest_loop`]) and send [`DebugCommand`]s via
/// [`encode_command`].
///
/// # Errors
///
/// Returns an error if the WebSocket handshake fails.
pub async fn connect(addr: &str) -> anyhow::Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
    let url = format!("ws://{addr}/devtools");
    let request = url
        .into_client_request()
        .context("building DevTools ws request")?;
    let (stream, _response) = connect_async(request)
        .await
        .context("DevTools websocket handshake")?;
    Ok(stream)
}

/// Runs the telemetry ingest loop on `stream` until the connection closes,
/// applying every decoded frame to `state`. Returns when the socket ends.
///
/// # Errors
///
/// Returns an error if a read fails fatally (the loop otherwise tolerates
/// individual malformed frames via [`ingest_message`]).
pub async fn run_ingest_loop(
    mut stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    state: Arc<DevToolsState>,
) -> anyhow::Result<()> {
    use futures_util::StreamExt;
    while let Some(msg) = stream.next().await {
        match msg {
            Ok(message) => {
                let n = ingest_message(&state, &message);
                if n > 0 {
                    tracing::debug!(events = n, "ingested telemetry");
                }
            }
            Err(e) => {
                tracing::warn!(%e, "DevTools ws read error");
                break;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flux_ir_serde::TelemetryEvent;
    use flux_syntax::{NodeId, SignalId, Value};

    fn sample_frame() -> Vec<u8> {
        let events = vec![
            TelemetryEvent::VmStep {
                bytecode_offset: 12,
                opcode: 0x05,
                registers: Box::new([const { Value::Null }; 16]),
                gas_remaining: 100,
            },
            TelemetryEvent::SignalWrite {
                signal_id: SignalId::from(1u32),
                old_value: Value::Null,
                new_value: Value::Int(7),
                triggered_effect_ids: vec![],
            },
            TelemetryEvent::ViewMutation {
                node_id: NodeId::from(3u32),
                native_view_id: 0,
                mutation_kind: 0,
                frame: None,
            },
        ];
        let enriched: Vec<EnrichedTelemetryEvent> =
            events.into_iter().map(enrich_telemetry).collect();
        EnrichedTelemetryFrame {
            version: flux_ir_serde::PROTOCOL_VERSION,
            event_count: enriched.len() as u16,
            events: enriched,
        }
        .to_bytes()
    }

    #[test]
    fn ingest_message_feeds_state() {
        let state = DevToolsState::new();
        let bytes = sample_frame();
        let msg = Message::Binary(bytes.into());
        let n = ingest_message(&state, &msg);
        assert_eq!(n, 3);
        assert_eq!(state.timeline_len(), 3);
        assert_eq!(state.vm_state().bytecode_offset, Some(12));
    }

    #[test]
    fn ingest_ignores_non_binary() {
        let state = DevToolsState::new();
        assert_eq!(ingest_message(&state, &Message::Text("ping".into())), 0);
        assert_eq!(state.timeline_len(), 0);
    }

    #[test]
    fn encode_command_is_decodable() {
        let bytes = encode_command(
            9,
            DebugCommand::SetBreakpoint {
                bytecode_offset: 512,
            },
        );
        let decoded = DebugCommandFrame::from_bytes(&bytes).expect("command decodes");
        assert_eq!(decoded.command_id, 9);
        assert_eq!(
            decoded.command,
            DebugCommand::SetBreakpoint {
                bytecode_offset: 512
            }
        );
    }

    /// Proves the DevTools data path end-to-end without a display: a local
    /// WebSocket server emits one enriched telemetry frame; the client connects
    /// and the ingest loop feeds it into `DevToolsState`. This is the headless
    /// equivalent of launching the app against a running dev server (PRD-P
    /// "ship it, not scaffold it" — the connect + ingest path is verified).
    #[tokio::test]
    async fn ingest_loop_pulls_from_live_server() {
        use futures_util::SinkExt;
        use futures_util::StreamExt;
        use tokio::net::TcpListener;
        use tokio_tungstenite::accept_async;

        // Build one enriched telemetry frame (mirrors what the dev server sends).
        let events = vec![TelemetryEvent::SignalWrite {
            signal_id: SignalId::from(7u32),
            old_value: Value::Null,
            new_value: Value::Int(3),
            triggered_effect_ids: vec![],
        }];
        let enriched: Vec<EnrichedTelemetryEvent> =
            events.into_iter().map(enrich_telemetry).collect();
        let frame = EnrichedTelemetryFrame {
            version: flux_ir_serde::PROTOCOL_VERSION,
            event_count: enriched.len() as u16,
            events: enriched,
        }
        .to_bytes();

        // Start a server that sends the frame then closes.
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (sock, _) = listener.accept().await.expect("accept");
            let ws = accept_async(sock).await.expect("accept ws");
            let (mut w, _) = ws.split();
            w.send(tokio_tungstenite::tungstenite::Message::Binary(
                frame.into(),
            ))
            .await
            .expect("send frame");
        });

        // Connect the real client and run the ingest loop.
        let state = Arc::new(DevToolsState::new());
        let stream = connect(&addr.to_string()).await.expect("client connect");
        let ingest = state.clone();
        let handle = tokio::spawn(async move {
            let _ = run_ingest_loop(stream, ingest).await;
        });
        // Give the loop a moment to receive and apply the frame.
        for _ in 0..50 {
            if state.timeline_len() >= 1 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        handle.abort();
        assert!(
            state.timeline_len() >= 1,
            "DevTools must ingest the frame the server sent"
        );
        assert_eq!(state.vm_state().bytecode_offset, None);
    }
}
