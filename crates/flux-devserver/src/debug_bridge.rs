//! DevTools debug bridge (spec §4): the bidirectional telemetry router.
//!
//! The bridge runs alongside the host patch channel and exposes a second
//! WebSocket endpoint (`:7333`) for the gpui DevTools app. It:
//! 1. receives `Telemetry` frames from the host (raw IDs),
//! 2. enriches them with `.flux` source spans via the compiled [`LoweredIr`]
//!    ([`SourceMap`]), and
//! 3. forwards the enriched events to every connected DevTools client.
//!
//! `DebugCommand` frames from DevTools are forwarded back to the host.
//!
//! The enrichment and routing logic here is pure and unit-tested; the network
//! accept loop ([`serve_devtools`]) is the only I/O path.

use std::collections::HashMap;

use flux_ir::LoweredIr;
use flux_ir_serde::{
    DebugCommand, EnrichedTelemetryEvent, EnrichedTelemetryFrame, HostAnnounceFrame,
    TelemetryEvent, TelemetryFrame,
};
use flux_syntax::{NodeId, Span};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
///
/// Built once per successful compile from the [`LoweredIr`] so telemetry can be
/// enriched without re-lowering on every event (spec §4.2).
#[derive(Clone, Debug, Default)]
pub struct SourceMap {
    /// `NodeId` → source span (from the packed arena).
    node_spans: HashMap<NodeId, Span>,
    /// Contiguous bytecode ranges → source span, one entry per closure laid out
    /// sequentially in the shared handler blob (matching Appendix D §D.12).
    closure_spans: Vec<(u32, u32, Span)>,
}

impl SourceMap {
    /// Builds a source map from a compiled [`LoweredIr`].
    #[must_use]
    pub fn from_lowered(ir: &LoweredIr) -> Self {
        let mut node_spans = HashMap::new();
        for id in ir.arena.all_ids() {
            if let Some(span) = ir.arena.span_for_node_id(id) {
                node_spans.insert(id, span);
            }
        }
        // Lay out each closure's bytecode sequentially to model the shared blob
        // offset space, recording the [start, end) window → closure span.
        let mut closure_spans = Vec::new();
        let mut cursor: u32 = 0;
        for closure in ir.closures.values() {
            let start = cursor;
            let end = cursor + closure.bytecode.len() as u32;
            closure_spans.push((start, end, closure.span));
            cursor = end;
        }
        Self {
            node_spans,
            closure_spans,
        }
    }

    /// Resolves a node's source span, if known.
    #[must_use]
    pub fn span_for_node_id(&self, id: NodeId) -> Option<Span> {
        self.node_spans.get(&id).copied()
    }

    /// Resolves a bytecode offset to the span of the closure that contains it.
    #[must_use]
    pub fn span_for_bytecode_offset(&self, offset: u32) -> Option<Span> {
        self.closure_spans
            .iter()
            .find(|(start, end, _)| offset >= *start && offset < *end)
            .map(|(_, _, span)| *span)
    }
}

/// Enriches a raw host [`TelemetryEvent`] with source spans from `map`.
///
/// Variants whose IDs resolve to a span get `Some(span)`; unresolved IDs carry
/// `None` so the DevTools UI can show "no source mapping".
#[must_use]
pub fn enrich(event: &TelemetryEvent, map: &SourceMap) -> EnrichedTelemetryEvent {
    let span = match event {
        TelemetryEvent::VmStep {
            bytecode_offset, ..
        } => map.span_for_bytecode_offset(*bytecode_offset),
        TelemetryEvent::ViewMutation { node_id, .. } => map.span_for_node_id(*node_id),
        TelemetryEvent::SignalWrite { .. } | TelemetryEvent::HandlerInvocation { .. } => None,
        #[allow(unreachable_patterns)]
        _ => None,
    };
    flux_ir_serde::enrich_with_span(event.clone(), span)
}

use tokio::sync::mpsc;

/// In-process router for DevTools telemetry/commands (spec §4.3).
///
/// `DevToolsRouter` is the pure routing core: telemetry from the host is
/// enriched and broadcast to every subscribed DevTools client; commands from a
/// DevTools client are forwarded to the host. It holds no network state, so it
/// is unit-tested directly. A separate channel carries [`HostAnnounceFrame`]s so
/// the DevTools UI learns which device is streaming (the server learns this from
/// the host `Hello` and re-broadcasts it to every connected DevTools client).
#[derive(Debug)]
pub struct DevToolsRouter {
    source_map: SourceMap,
    /// One sender per connected DevTools client (telemetry stream).
    devtools: Vec<mpsc::UnboundedSender<EnrichedTelemetryEvent>>,
    /// One sender per connected DevTools client (host-identity stream).
    host_announce: Vec<mpsc::UnboundedSender<HostAnnounceFrame>>,
    /// Where host-bound `DebugCommand`s are forwarded.
    host_command: mpsc::UnboundedSender<DebugCommand>,
}

impl DevToolsRouter {
    /// Creates a router; `host_command` receives every forwarded `DebugCommand`.
    #[must_use]
    pub fn new(source_map: SourceMap, host_command: mpsc::UnboundedSender<DebugCommand>) -> Self {
        Self {
            source_map,
            devtools: Vec::new(),
            host_announce: Vec::new(),
            host_command,
        }
    }

    /// Registers a new DevTools client and returns its event receiver.
    pub fn subscribe_devtools(&mut self) -> mpsc::UnboundedReceiver<EnrichedTelemetryEvent> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.devtools.push(tx);
        rx
    }

    /// Registers a new DevTools client for the host-identity stream and returns
    /// its receiver.
    pub fn subscribe_host_announce(&mut self) -> mpsc::UnboundedReceiver<HostAnnounceFrame> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.host_announce.push(tx);
        rx
    }

    /// Broadcasts a host-identity frame to all subscribed DevTools clients.
    /// Returns the number of clients reached.
    pub fn announce_host(&mut self, announce: &HostAnnounceFrame) -> usize {
        let mut reached = 0;
        self.host_announce
            .retain(|tx| match tx.send(announce.clone()) {
                Ok(()) => {
                    reached += 1;
                    true
                }
                Err(_) => false,
            });
        tracing::debug!(reached, "announce_host: broadcast complete");
        reached
    }

    /// Enriches a host telemetry event and broadcasts it to all DevTools clients.
    /// Returns the number of clients reached.
    pub fn route_telemetry(&mut self, event: &TelemetryEvent) -> usize {
        let enriched = enrich(event, &self.source_map);
        let mut reached = 0;
        self.devtools.retain(|tx| match tx.send(enriched.clone()) {
            Ok(()) => {
                reached += 1;
                true
            }
            Err(_) => false, // drop disconnected clients
        });
        tracing::debug!(reached, "route_telemetry: broadcast complete");
        reached
    }

    /// Forwards a `DebugCommand` to the host. Returns `false` if the host is gone.
    #[must_use]
    pub fn route_command(&self, command: DebugCommand) -> bool {
        self.host_command.send(command).is_ok()
    }
}

/// Default DevTools WebSocket port (spec §4.1).
pub const DEFAULT_DEVTOOLS_PORT: u16 = 7333;

/// Accepts DevTools WebSocket clients on `addr`, enriching host telemetry and
/// forwarding it to every client while relaying `DebugCommand`s to `host_sink`.
///
/// Each accepted `Telemetry` frame is enriched via `source_map` and broadcast;
/// each `DebugCommand` frame is sent to `host_sink`. The returned task runs
/// until the listener errors. This is the only I/O path in the bridge.
pub async fn serve_devtools(
    addr: std::net::SocketAddr,
    router: std::sync::Arc<parking_lot::Mutex<DevToolsRouter>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    // One router shared by every connection so host telemetry is broadcast to
    // all subscribed DevTools clients (not just the sender's own connection).
    // The router is owned by the server and shared with the host patch-channel
    // accept loop, which routes telemetry the host sends over `:7331`.
    loop {
        let (stream, _) = listener.accept().await?;
        let upgraded =
            tokio_tungstenite::accept_async_with_config(stream, Some(WebSocketConfig::default()))
                .await;
        let Ok(ws) = upgraded else {
            continue; // malformed handshake; skip this connection
        };
        let (mut writer, mut reader) = ws.split();
        // Subscribe this connection; the returned receiver drives its outbound
        // stream. Dropped connections are pruned by `route_telemetry`.
        let mut sub_rx = router.lock().subscribe_devtools();
        let mut host_rx = router.lock().subscribe_host_announce();
        // Outbound: enriched telemetry events AND host-identity frames share a
        // single sink (a `SplitSink` is not `Clone`, so we `select!` over both
        // receivers in one task that owns `writer`).
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    event = sub_rx.recv() => {
                        let Some(event) = event else { break };
                        let frame = EnrichedTelemetryFrame {
                            version: flux_ir_serde::PROTOCOL_VERSION,
                            event_count: 1,
                            events: vec![event],
                        };
                        if writer
                            .send(tokio_tungstenite::tungstenite::Message::Binary(
                                frame.to_bytes().into(),
                            ))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    announce = host_rx.recv() => {
                        let Some(announce) = announce else { break };
                        if writer
                            .send(tokio_tungstenite::tungstenite::Message::Binary(
                                announce.to_bytes().into(),
                            ))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        });
        // Inbound: telemetry → enrich+broadcast; commands → host.
        let router_in = router.clone();
        tokio::spawn(async move {
            while let Some(msg) = reader.next().await {
                let Ok(msg) = msg else { continue };
                let Some(frame) = TelemetryFrame::from_bytes(match &msg {
                    tokio_tungstenite::tungstenite::Message::Binary(b) => b,
                    _ => continue,
                }) else {
                    tracing::debug!("serve_devtools: inbound frame failed to decode");
                    continue;
                };
                for event in frame.events {
                    router_in.lock().route_telemetry(&event);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flux_ir_serde::PROTOCOL_VERSION;

    fn sample_event() -> TelemetryEvent {
        TelemetryEvent::VmStep {
            bytecode_offset: 0,
            opcode: 0x01,
            registers: Box::new([const { flux_syntax::Value::Null }; 16]),
            gas_remaining: 1000,
        }
    }

    #[test]
    fn enrich_unknown_ids_yields_none() {
        let map = SourceMap::default();
        let enriched = enrich(&sample_event(), &map);
        match enriched {
            EnrichedTelemetryEvent::VmStep { source_span, .. } => assert_eq!(source_span, None),
            _ => panic!("expected VmStep"),
        }
    }

    #[test]
    fn router_broadcasts_to_subscribers() {
        let (host_tx, _host_rx) = mpsc::unbounded_channel();
        let mut router = DevToolsRouter::new(SourceMap::default(), host_tx);
        let mut rx = router.subscribe_devtools();
        assert_eq!(router.route_telemetry(&sample_event()), 1);
        // The subscriber received the enriched event.
        let got = rx.try_recv().expect("event should be delivered");
        match got {
            EnrichedTelemetryEvent::VmStep {
                bytecode_offset, ..
            } => assert_eq!(bytecode_offset, 0),
            _ => panic!("expected VmStep"),
        }
    }

    #[test]
    fn router_drops_disconnected_clients() {
        let (host_tx, _host_rx) = mpsc::unbounded_channel();
        let mut router = DevToolsRouter::new(SourceMap::default(), host_tx);
        let _rx = router.subscribe_devtools();
        drop(_rx); // simulate disconnect
        assert_eq!(router.route_telemetry(&sample_event()), 0);
    }

    #[test]
    fn source_map_from_empty_ir_is_empty() {
        // A freshly defaulted map resolves nothing.
        let map = SourceMap::default();
        assert_eq!(map.span_for_node_id(flux_syntax::NodeId::from(1u32)), None);
        assert_eq!(map.span_for_bytecode_offset(0), None);
        let _ = PROTOCOL_VERSION;
    }

    // Proves the full host -> server -> DevTools-client data path over a real
    // WebSocket: a host client sends a raw Telemetry frame, the server enriches
    // it, and a subscribed DevTools client receives the enriched frame. This is
    // the wire contract the iOS/Android bridges rely on (without needing the
    // gpui desktop UI, which is nightly-gated).
    #[tokio::test]
    async fn host_telemetry_reaches_devtools_client() {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        use tokio_tungstenite::{connect_async, tungstenite::Message};

        // Fixed ephemeral-range port for the test endpoint. A probe/rebind race
        // with `serve_devtools` (which binds the same addr itself) would hang
        // the clients, so we use a stable port and let the server own the bind.
        let addr: std::net::SocketAddr = "127.0.0.1:17399".parse().unwrap();

        let (host_tx, _host_rx) = mpsc::unbounded_channel();
        let source_map = SourceMap::default();
        let test_router = std::sync::Arc::new(parking_lot::Mutex::new(DevToolsRouter::new(
            source_map, host_tx,
        )));
        tokio::spawn(async move { serve_devtools(addr, test_router).await });
        // Give the accept loop a moment to bind.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let url = format!("ws://{addr}/devtools");

        // DevTools client MUST subscribe before the host sends: the server
        // broadcasts telemetry live to currently-connected DevTools clients.
        let devtools_req = url.clone().into_client_request().unwrap();
        let (mut devtools_ws, _) = connect_async(devtools_req).await.unwrap();

        // Host client: sends a raw Telemetry frame.
        let host_req = url.into_client_request().unwrap();
        let (mut host_ws, _) = connect_async(host_req).await.unwrap();
        let frame = TelemetryFrame {
            version: PROTOCOL_VERSION,
            event_count: 1,
            events: vec![sample_event()],
        };
        host_ws
            .send(Message::Binary(frame.to_bytes().into()))
            .await
            .unwrap();

        // DevTools client: receives the enriched frame.
        let msg = devtools_ws.next().await.unwrap().unwrap();
        let bytes = match msg {
            Message::Binary(b) => b,
            _ => panic!("expected binary telemetry frame"),
        };
        let enriched = EnrichedTelemetryFrame::from_bytes(&bytes).expect("decodes enriched frame");
        assert_eq!(enriched.event_count, 1);
        assert_eq!(enriched.events.len(), 1);
        match &enriched.events[0] {
            EnrichedTelemetryEvent::VmStep {
                bytecode_offset, ..
            } => assert_eq!(*bytecode_offset, 0),
            other => panic!("expected enriched VmStep, got {other:?}"),
        }
    }
}
