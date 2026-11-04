//! Round-21 regression test: a handshake-REJECTED host must receive its `Error`
//! reply and then nothing else — never a broadcast frame.
//!
//! Root cause this guards: `handle_hello` returns `Some(reply)` for BOTH the
//! accepted (`Init`) and the rejected (`Error`) outcome, so `serve_client`'s
//! plain `if let Some(reply)` registered a rejected client into the broadcast
//! list too (`break shared.register()`), and it then received every Init/Delta
//! frame meant for accepted hosts. The fix registers a client only when the
//! reply's wire frame-kind byte (header offset 5, Appendix D §D.12) is
//! `FRAME_INIT`; a rejected client still receives the `Error` frame, then the
//! connection closes.
//!
//! Mirrors the harness in `watch_race.rs` / `auth_token.rs`: the blocking
//! `tungstenite` connect/read runs inside `tokio::task::spawn_blocking` so it
//! does not stall the server's accept task on the async reactor.

use std::fs;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flux_devserver::{DevServer, RunningServer, ServerConfig};
use flux_ir_serde::{FRAME_DELTA, FRAME_ERROR, FRAME_HEARTBEAT, FRAME_INIT};
use tokio_tungstenite::tungstenite::stream::MaybeTlsStream;
use tokio_tungstenite::tungstenite::{Message, WebSocket, connect};

const GOOD_SOURCE: &str =
    "compo Hello\n  state count: Int = 0\n  Button(text: \"tap\")\n  Text(text: \"{count}\")\n";

const EDITED_SOURCE: &str =
    "compo Hello\n  state count: Int = 0\n  Button(text: \"tap tap\")\n  Text(text: \"{count}\")\n";

fn scratch_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = std::env::temp_dir().join(format!("flux-reject-bcast-{tag}-{nanos}"));
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

async fn start_with_token(root: &Path, token: &str) -> RunningServer {
    let config = ServerConfig::new(root)
        .with_ws_addr("127.0.0.1:0".parse::<SocketAddr>().expect("addr"))
        .with_http_addr("127.0.0.1:0".parse::<SocketAddr>().expect("addr"))
        .with_debounce(Duration::from_millis(20))
        .with_auth_token(token);
    DevServer::start(config).await.expect("server starts")
}

type Client = WebSocket<MaybeTlsStream<std::net::TcpStream>>;

fn connect_client(addr: SocketAddr) -> Client {
    let (socket, _response) = connect(format!("ws://{addr}/")).expect("ws connect");
    socket
}

fn frame_type(bytes: &[u8]) -> Option<u8> {
    bytes.get(5).copied()
}

/// Sends `Hello` with `token` and returns the kind byte of the reply frame.
fn handshake(client: &mut Client, token: &str) -> u8 {
    let hello = flux_ir_serde::Frame::hello_with_token("ios", "test", &[], token).to_bytes();
    client
        .send(Message::Binary(hello.into()))
        .expect("send hello");
    next_frame(client, Duration::from_secs(5))
        .and_then(|f| frame_type(&f))
        .expect("handshake reply")
}

/// Reads the next non-heartbeat binary frame, or `None` on timeout / close.
fn next_frame(client: &mut Client, timeout: Duration) -> Option<Vec<u8>> {
    let deadline = Instant::now() + timeout;
    if let MaybeTlsStream::Plain(stream) = client.get_ref() {
        stream
            .set_read_timeout(Some(Duration::from_millis(50)))
            .expect("read timeout");
    }
    while Instant::now() < deadline {
        match client.read() {
            Ok(Message::Binary(bytes)) => {
                let bytes = bytes.to_vec();
                if frame_type(&bytes) == Some(FRAME_HEARTBEAT) {
                    continue;
                }
                return Some(bytes);
            }
            Ok(_) => continue,
            Err(tokio_tungstenite::tungstenite::Error::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => return None,
        }
    }
    None
}

/// The rejected client gets exactly one frame (the `Error` reply) and then no
/// broadcast traffic, while an accepted client does receive the broadcast.
#[tokio::test]
async fn rejected_handshake_gets_error_then_no_broadcast_frames() {
    let root = scratch_dir("main");
    fs::write(root.join("main.flux"), GOOD_SOURCE).expect("write source");
    let server = start_with_token(&root, "secret").await;
    let addr = server.ws_addr();

    // Accepted client: correct token → Init reply and broadcast registration.
    let (mut accepted, accepted_kind) = tokio::task::spawn_blocking(move || {
        let mut client = connect_client(addr);
        let kind = handshake(&mut client, "secret");
        (client, kind)
    })
    .await
    .expect("accepted handshake task");
    assert_eq!(accepted_kind, FRAME_INIT, "correct token is accepted");

    // Rejected client: wrong token → Error reply.
    let (mut rejected, rejected_kind) = tokio::task::spawn_blocking(move || {
        let mut client = connect_client(addr);
        let kind = handshake(&mut client, "nope");
        (client, kind)
    })
    .await
    .expect("rejected handshake task");
    assert_eq!(
        rejected_kind, FRAME_ERROR,
        "wrong token is rejected with an Error frame"
    );

    // Trigger a real broadcast: save a changed source; the watcher reloads,
    // recompiles and ships the resulting frame to every registered client.
    let file = root.join("main.flux");
    tokio::task::spawn_blocking(move || {
        std::thread::sleep(Duration::from_millis(30));
        fs::write(&file, EDITED_SOURCE).expect("save edit");
    })
    .await
    .expect("edit task");

    // The accepted client must observe the broadcast — this proves the
    // broadcast actually fired (the assertion below would otherwise pass
    // vacuously).
    let observed = tokio::task::spawn_blocking(move || {
        next_frame(&mut accepted, Duration::from_secs(5)).map(|f| frame_type(&f))
    })
    .await
    .expect("accepted read task");
    let observed_kind = observed
        .expect("accepted client must receive the broadcast frame")
        .expect("frame kind");
    assert!(
        observed_kind == FRAME_DELTA || observed_kind == FRAME_INIT,
        "accepted client must receive the broadcast Delta/Init, got {observed_kind:#04x}"
    );

    // The rejected client must receive NOTHING further: its socket closed
    // right after the Error reply, and no broadcast frame may reach it.
    let leaked =
        tokio::task::spawn_blocking(move || next_frame(&mut rejected, Duration::from_secs(2)))
            .await
            .expect("rejected read task");
    assert!(
        leaked.is_none(),
        "rejected client received a broadcast frame ({:#04x}) after its Error reply",
        leaked.as_ref().and_then(|f| frame_type(f)).unwrap_or(0)
    );

    server.shutdown();
    let _ = fs::remove_dir_all(&root);
}
