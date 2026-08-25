//! One host connection: the `Hello` handshake and the frame fan-out loop (FLUX-019).
//!
//! Each accepted socket is driven by a blocking task running [`serve_client`].
//! The synchronous `tungstenite` state machine is used because this crate's
//! pre-wired dependency set carries no `futures-util`, so the async
//! `Sink`/`Stream` adapters on `WebSocketStream` cannot be named here.

use std::io::ErrorKind;
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use tokio_tungstenite::tungstenite::{
    Error as WsError, HandshakeError, Message, WebSocket, accept,
};

use crate::dispatch::FRAME_DISPATCH_REPORT;
use crate::error::Diagnostic;
use crate::server::Shared;

/// How long a client thread blocks on a socket read before checking its queue.
const CLIENT_POLL: Duration = Duration::from_millis(10);

/// Drives one host connection: `Hello` handshake, `Init` reply, then frame fan-out.
///
/// # Errors
///
/// Returns the underlying `tungstenite` error when the upgrade or a socket write
/// fails. A clean close is `Ok(())`.
pub(crate) fn serve_client(stream: TcpStream, shared: &Arc<Shared>) -> Result<(), WsError> {
    let mut socket = upgrade(stream)?;
    let queue = shared.register();
    let mut handshook = false;
    while !shared.is_shutdown() {
        match socket.read() {
            Ok(Message::Binary(bytes)) => {
                if handle_host_frame(&bytes, &mut socket, shared)? {
                    handshook = true;
                }
            }
            Ok(Message::Close(_)) => return Ok(()),
            Ok(_) => {}
            Err(WsError::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(WsError::ConnectionClosed) => return Ok(()),
            Err(error) => return Err(error),
        }
        if handshook && !drain_queue(&queue, &mut socket)? {
            return Ok(());
        }
    }
    Ok(())
}

/// Performs the WebSocket upgrade and switches the socket to polled reads.
fn upgrade(stream: TcpStream) -> Result<WebSocket<TcpStream>, WsError> {
    // Accepted sockets inherit the listener's non-blocking flag on macOS and
    // Linux; the HTTP upgrade must be read in blocking mode or `accept` reports
    // `HandshakeIncomplete`. The read timeout installed afterwards is what lets
    // the frame loop poll its outgoing queue.
    stream.set_nonblocking(false).map_err(WsError::Io)?;
    stream.set_nodelay(true).map_err(WsError::Io)?;
    let socket = accept(stream).map_err(|e| match e {
        HandshakeError::Failure(err) => err,
        HandshakeError::Interrupted(_) => WsError::ConnectionClosed,
    })?;
    socket
        .get_ref()
        .set_read_timeout(Some(CLIENT_POLL))
        .map_err(WsError::Io)?;
    Ok(socket)
}

/// Handles one host→server frame. Returns `true` once the handshake completed.
fn handle_host_frame(
    bytes: &[u8],
    socket: &mut WebSocket<TcpStream>,
    shared: &Arc<Shared>,
) -> Result<bool, WsError> {
    // A host dispatch report is a separate frame type from `Hello`; route it
    // before the handshake check because a host may report dispatches at any
    // time after connecting.
    if bytes.get(5).copied() == Some(FRAME_DISPATCH_REPORT) {
        handle_dispatch_report(bytes, shared);
        return Ok(false);
    }
    use flux_ir_serde::{FRAME_HELLO, Frame};
    if bytes.get(5).copied() != Some(FRAME_HELLO) {
        // Heartbeats and unknown host frames are ignored; the host drives the
        // channel with `Hello` only (spec §D.12).
        return Ok(false);
    }
    let reply = match Frame::from_hello_bytes(bytes) {
        Some(hello) => {
            tracing::info!(
                platform = %hello.platform,
                device = %hello.device,
                version = hello.version,
                capabilities = hello.capabilities.len(),
                "host handshake"
            );
            init_reply(shared)
        }
        None => Some(shared.pipeline.lock().error_frame(&malformed_hello())),
    };
    if let Some(frame) = reply {
        socket.send(Message::Binary(frame.into()))?;
    }
    Ok(true)
}

/// Handles a host→server dispatch report (ADR-0027 Phase 2).
///
/// Decodes the report and asks the pipeline for the minimal-patch `Delta`. When
/// the pipeline returns `None` (index inactive → degrade to coarse frame, or the
/// written signal has no dependents → `noop_dispatch`), nothing is shipped.
/// Otherwise the `Delta` is fanned out to every connected host.
fn handle_dispatch_report(bytes: &[u8], shared: &Arc<Shared>) {
    let report = match crate::dispatch::DispatchReport::from_bytes(bytes) {
        Some(report) => report,
        None => {
            tracing::warn!(bytes = bytes.len(), "dropped malformed dispatch report");
            return;
        }
    };
    let frame = shared.pipeline.lock().handle_dispatch_report(report);
    if let Some(frame) = frame {
        shared.broadcast(frame);
        tracing::debug!(handler = ?report.handler_id, "shipped minimal dispatch delta");
    }
}

/// Builds the handshake reply: the retained tree's `Init`, or an `Error` frame
/// when the project does not currently compile (spec §D.12.2 / §D.12.3).
fn init_reply(shared: &Arc<Shared>) -> Option<Vec<u8>> {
    let mut pipeline = shared.pipeline.lock();
    if !pipeline.has_tree() {
        if let Err(diagnostic) = pipeline.compile() {
            return Some(pipeline.error_frame(&diagnostic));
        }
    }
    pipeline.init_frame()
}

/// The diagnostic shipped when a host frame claims to be `Hello` but does not
/// decode at this protocol version.
fn malformed_hello() -> Diagnostic {
    Diagnostic::new(
        format!(
            "malformed handshake frame: expected a Hello frame at protocol version {} \
             — hint: rebuild the host app against this dev server",
            flux_ir_serde::PROTOCOL_VERSION
        ),
        None,
    )
}

/// Writes every queued frame to `socket`. Returns `false` when the queue closed.
fn drain_queue(
    queue: &Receiver<Vec<u8>>,
    socket: &mut WebSocket<TcpStream>,
) -> Result<bool, WsError> {
    loop {
        match queue.try_recv() {
            Ok(frame) => socket.send(Message::Binary(frame.into()))?,
            Err(TryRecvError::Empty) => return Ok(true),
            Err(TryRecvError::Disconnected) => return Ok(false),
        }
    }
}
