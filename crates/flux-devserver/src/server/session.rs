//! One host connection: the `Hello` handshake and the frame fan-out loop (FLUX-019).
//!
//! Each accepted socket is upgraded with [`tokio_tungstenite::accept_async`] and
//! driven by its own `tokio::spawn`ed task running [`serve_client`]. The socket's
//! read half and the session's outbound queue are polled together with
//! `tokio::select!`, so neither a silent client nor a burst of broadcast frames
//! can block the other — and no thread is parked per connection (brittleness
//! issue 6).
//!
//! Compile work reached from this loop (the `Hello` → `Init` reply) runs on
//! Tokio's blocking pool via `spawn_blocking`, keeping the I/O reactor free.

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tokio_tungstenite::{WebSocketStream, accept_async};

use crate::dispatch::FRAME_DISPATCH_REPORT;
use crate::error::Diagnostic;
use crate::server::Shared;

/// The upgraded WebSocket stream a session is driven over.
type HostSocket = WebSocketStream<TcpStream>;

/// Drives one host connection: `Hello` handshake, `Init` reply, then frame fan-out.
///
/// The loop terminates on a clean close, on server shutdown, or when the
/// session's broadcast queue is dropped.
///
/// # Errors
///
/// Returns the underlying `tungstenite` error when the upgrade or a socket write
/// fails. A clean close is `Ok(())`.
pub(crate) async fn serve_client(stream: TcpStream, shared: Arc<Shared>) -> Result<(), WsError> {
    // Nagle off: patch frames are small and latency-sensitive (spec §3.7).
    stream.set_nodelay(true).map_err(WsError::Io)?;
    let socket = accept_async(stream).await?;
    let queue = shared.register();
    run_session(socket, queue, shared).await
}

/// The read/write select loop for one upgraded connection.
async fn run_session(
    socket: HostSocket,
    mut queue: UnboundedReceiver<Vec<u8>>,
    shared: Arc<Shared>,
) -> Result<(), WsError> {
    let (mut writer, mut reader) = socket.split();
    let mut handshook = false;
    while !shared.is_shutdown() {
        tokio::select! {
            incoming = reader.next() => match incoming {
                Some(Ok(Message::Binary(bytes))) => {
                    for frame in handle_host_frame(&bytes, &shared).await {
                        writer.send(Message::Binary(frame.into())).await?;
                    }
                    if is_hello(&bytes) {
                        handshook = true;
                    }
                }
                Some(Ok(Message::Close(_))) | None => return Ok(()),
                Some(Ok(_)) => {}
                Some(Err(WsError::ConnectionClosed | WsError::AlreadyClosed)) => return Ok(()),
                Some(Err(error)) => return Err(error),
            },
            queued = queue.recv(), if handshook => match queued {
                Some(frame) => writer.send(Message::Binary(frame.into())).await?,
                None => return Ok(()),
            },
        }
    }
    Ok(())
}

/// Whether `bytes` is a `Hello` handshake frame (spec §D.12.1).
fn is_hello(bytes: &[u8]) -> bool {
    bytes.get(5).copied() == Some(flux_ir_serde::FRAME_HELLO)
}

/// Handles one host→server frame, returning the frames to write back (possibly
/// none).
async fn handle_host_frame(bytes: &[u8], shared: &Arc<Shared>) -> Vec<Vec<u8>> {
    // A host dispatch report is a separate frame type from `Hello`; route it
    // before the handshake check because a host may report dispatches at any
    // time after connecting.
    match bytes.get(5).copied() {
        Some(FRAME_DISPATCH_REPORT) => {
            handle_dispatch_report(bytes, shared);
            Vec::new()
        }
        // Brittleness 4a: the host asks for a canonical `StringId` instead of
        // synthesising one locally.
        Some(flux_ir_serde::FRAME_INTERN_STRING) => {
            handle_intern_string(bytes, shared).into_iter().collect()
        }
        Some(flux_ir_serde::FRAME_HELLO) => handle_hello(bytes, shared).await.into_iter().collect(),
        // Heartbeats and unknown host frames are ignored; the host drives the
        // channel with `Hello` only (spec §D.12).
        _ => Vec::new(),
    }
}

/// Handles the `Hello` handshake, returning the `Init` (or `Error`) reply.
///
/// The compile the reply may need runs on the blocking pool: it is the only
/// CPU-bound step in the WebSocket path and must not stall the reactor.
async fn handle_hello(bytes: &[u8], shared: &Arc<Shared>) -> Option<Vec<u8>> {
    use flux_ir_serde::Frame;
    let Some(hello) = Frame::from_hello_bytes(bytes) else {
        let shared = Arc::clone(shared);
        return blocking(move || shared.pipeline.lock().error_frame(&malformed_hello())).await;
    };
    // Validate the host advertises every capability the compiled tree
    // CALL_CAPs (spec §D.12.1 / §24.4). A missing method surfaces as a clear
    // `Error` frame here, not as a silent VM fault at the first call.
    let shared_req = Arc::clone(shared);
    let required = blocking(move || shared_req.pipeline.lock().required_capabilities()).await;
    let missing = match required {
        Some(required) => missing_capabilities(&hello.capabilities, &required),
        None => return None,
    };
    if !missing.is_empty() {
        let diagnostic = capability_mismatch(&hello, &missing);
        tracing::warn!(
            platform = %hello.platform,
            device = %hello.device,
            missing = ?missing,
            "host handshake rejected: missing required capabilities"
        );
        let shared = Arc::clone(shared);
        return blocking(move || shared.pipeline.lock().error_frame(&diagnostic)).await;
    }
    tracing::info!(
        platform = %hello.platform,
        device = %hello.device,
        version = hello.version,
        capabilities = hello.capabilities.len(),
        "host handshake"
    );
    let shared = Arc::clone(shared);
    blocking(move || init_reply(&shared)).await.flatten()
}

/// The `(capability, method)` names a host fails to advertise but the tree
/// requires, in stable sorted order.
fn missing_capabilities(
    advertised: &[(String, u32, Vec<String>)],
    required: &[(u32, u16)],
) -> Vec<(String, String)> {
    let mut missing = Vec::new();
    for &(cap_id, method_id) in required {
        let Some((cap_name, method_name)) =
            crate::capability_manifest::names_for(cap_id, method_id)
        else {
            // Unknown id: cannot be satisfied by any host. Report it plainly so
            // the author learns the id is not part of the MLP manifest.
            missing.push(("unknown".to_owned(), format!("{cap_id}.{method_id}")));
            continue;
        };
        if !crate::capability_manifest::is_satisfied(advertised, cap_name, method_name) {
            missing.push((cap_name.to_owned(), method_name.to_owned()));
        }
    }
    missing.sort_unstable();
    missing
}

/// Builds the diagnostic for a host that omits required capabilities.
fn capability_mismatch(
    hello: &flux_ir_serde::HelloFrame,
    missing: &[(String, String)],
) -> crate::error::Diagnostic {
    let list: Vec<String> = missing
        .iter()
        .map(|(cap, method)| format!("{cap}.{method}"))
        .collect();
    crate::error::Diagnostic::new(
        format!(
            "host {} ({}) is missing required capabilities: {} — \
             hint: the app's .flux tree CALL_CAPs these, but the host only advertises {}. \
             Rebuild the host against the current stdlib/capabilities.flux, or add the \
             missing capability to the host build",
            hello.platform,
            hello.device,
            list.join(", "),
            advertised_summary(&hello.capabilities),
        ),
        None,
    )
}

/// One-line summary of what a host advertised, used in the mismatch hint.
fn advertised_summary(advertised: &[(String, u32, Vec<String>)]) -> String {
    if advertised.is_empty() {
        return "nothing".to_owned();
    }
    advertised
        .iter()
        .map(|(name, _v, feats)| {
            if feats.is_empty() {
                name.clone()
            } else {
                format!("{name}({})", feats.join(", "))
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Runs `work` on Tokio's blocking pool, returning `None` if the pool task was
/// cancelled (server shutdown).
async fn blocking<T, F>(work: F) -> Option<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    match tokio::task::spawn_blocking(work).await {
        Ok(value) => Some(value),
        Err(error) => {
            tracing::debug!(%error, "blocking pipeline task did not complete");
            None
        }
    }
}

/// Handles an `InternString` request (brittleness 4a).
///
/// Interns the payload into the server's canonical string table — shared across
/// every session through the pipeline mutex — and replies with a
/// `StringInterned` frame carrying an id below
/// [`flux_ir_serde::STRING_ID_CANONICAL_CEILING`]. A non-UTF-8 or over-long
/// payload is a protocol violation: it is logged and dropped rather than
/// answered with a bogus id.
fn handle_intern_string(bytes: &[u8], shared: &Arc<Shared>) -> Option<Vec<u8>> {
    use flux_ir_serde::{Frame, StringInternedFrame};
    let request = Frame::from_intern_string_bytes(bytes)?;
    let Some(text) = request.as_str() else {
        tracing::warn!(
            len = request.len,
            "dropped InternString frame with a non-UTF-8 payload"
        );
        return None;
    };
    let id = shared.pipeline.lock().intern_string(text);
    tracing::debug!(id, text, "interned host string");
    Some(StringInternedFrame::new(id).to_bytes())
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
