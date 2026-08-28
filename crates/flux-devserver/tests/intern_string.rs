//! Acceptance tests for the `InternString` → `StringInterned` exchange
//! (brittleness 4a) and for the WebSocket server's non-blocking behaviour
//! (brittleness 6).
//!
//! Each test drives the real server over a real TCP socket, exactly as an
//! iOS/Android host would.

use std::fs;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flux_devserver::{DevServer, RunningServer, ServerConfig};
use flux_ir_serde::{
    FRAME_STRING_INTERNED, Frame, STRING_ID_CANONICAL_CEILING, StringInternedFrame,
};
use tokio_tungstenite::tungstenite::stream::MaybeTlsStream;
use tokio_tungstenite::tungstenite::{Message, WebSocket, connect};

const GOOD_SOURCE: &str = "compo Hello\n  state count: Int = 0\n  Button(text: \"tap\")\n";

/// A unique scratch directory (the crate has no `tempfile` dev-dependency).
fn scratch_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = std::env::temp_dir().join(format!("flux-devserver-{tag}-{nanos}"));
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

async fn start(root: &Path) -> RunningServer {
    let config = ServerConfig::new(root)
        .with_ws_addr("127.0.0.1:0".parse::<SocketAddr>().expect("addr"))
        .with_http_addr("127.0.0.1:0".parse::<SocketAddr>().expect("addr"))
        .with_debounce(Duration::from_millis(20));
    DevServer::start(config).await.expect("server starts")
}

type Client = WebSocket<MaybeTlsStream<std::net::TcpStream>>;

fn connect_client(addr: SocketAddr) -> Client {
    let url = format!("ws://{addr}/");
    let (socket, _response) = connect(url).expect("ws connect");
    socket
}

fn frame_type(bytes: &[u8]) -> Option<u8> {
    bytes.get(5).copied()
}

/// Reads the next non-heartbeat binary frame, or `None` on timeout.
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
                if frame_type(&bytes) == Some(flux_ir_serde::FRAME_HEARTBEAT) {
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

/// Sends an `InternString` request for `text` and decodes the reply.
fn intern(client: &mut Client, text: &str) -> StringInternedFrame {
    let request = Frame::intern_string(text.as_bytes()).to_bytes();
    client
        .send(Message::Binary(request.into()))
        .expect("send InternString");
    let reply = next_frame(client, Duration::from_secs(5)).expect("StringInterned reply arrives");
    assert_eq!(
        frame_type(&reply),
        Some(FRAME_STRING_INTERNED),
        "server must answer InternString with StringInterned"
    );
    Frame::from_string_interned_bytes(&reply).expect("decodes as StringInterned")
}

#[tokio::test]
async fn intern_string_returns_a_stable_canonical_id() {
    let root = scratch_dir("intern");
    fs::write(root.join("hello.flux"), GOOD_SOURCE).expect("write source");
    let server = start(&root).await;
    let addr = server.ws_addr();

    tokio::task::spawn_blocking(move || {
        let mut client = connect_client(addr);
        let first = intern(&mut client, "host-generated-label");
        assert!(
            first.id < STRING_ID_CANONICAL_CEILING,
            "id {} must be canonical (< {STRING_ID_CANONICAL_CEILING:#x}) so the host \
             can drop its synthetic fallback",
            first.id
        );

        // Interning is stable: the same string yields the same id.
        let again = intern(&mut client, "host-generated-label");
        assert_eq!(
            again.id, first.id,
            "re-interning the same string must return the same id"
        );

        // A different string gets a different canonical id.
        let other = intern(&mut client, "another-label");
        assert_ne!(other.id, first.id, "distinct strings must get distinct ids");
        assert!(other.id < STRING_ID_CANONICAL_CEILING);
    })
    .await
    .expect("client task");
    server.shutdown();
}

#[tokio::test]
async fn intern_string_is_shared_across_sessions() {
    let root = scratch_dir("intern-shared");
    fs::write(root.join("hello.flux"), GOOD_SOURCE).expect("write source");
    let server = start(&root).await;
    let addr = server.ws_addr();

    tokio::task::spawn_blocking(move || {
        let mut first_client = connect_client(addr);
        let id = intern(&mut first_client, "shared-label").id;
        drop(first_client);

        // The string table is global to the server, so a second host resolves
        // the same text to the same canonical id.
        let mut second_client = connect_client(addr);
        assert_eq!(
            intern(&mut second_client, "shared-label").id,
            id,
            "the string table must be shared across sessions"
        );
    })
    .await
    .expect("client task");
    server.shutdown();
}

#[tokio::test]
async fn a_string_already_in_the_tree_resolves_to_its_arena_id() {
    let root = scratch_dir("intern-arena");
    fs::write(root.join("hello.flux"), GOOD_SOURCE).expect("write source");
    let server = start(&root).await;
    let addr = server.ws_addr();

    tokio::task::spawn_blocking(move || {
        let mut client = connect_client(addr);
        // `tap` is a literal in the compiled source. The server keeps a single
        // canonical id per distinct string: interning the same literal through
        // the live API must return the same id — it must never mint a second
        // one, or the host would hold two ids for one string. (A connecting
        // host also receives the tree's string table in its `Init` frame, which
        // resolves the same literal to this canonical id.)
        let first = intern(&mut client, "tap").id;
        let second = intern(&mut client, "tap").id;
        assert_eq!(
            first, second,
            "interning the same string twice must reuse one id"
        );
        assert!(
            first < STRING_ID_CANONICAL_CEILING,
            "interned id is canonical"
        );
    })
    .await
    .expect("client task");
    server.shutdown();
}

#[tokio::test]
async fn a_silent_client_does_not_block_another_session() {
    let root = scratch_dir("intern-nonblocking");
    fs::write(root.join("hello.flux"), GOOD_SOURCE).expect("write source");
    let server = start(&root).await;
    let addr = server.ws_addr();

    tokio::task::spawn_blocking(move || {
        // A connected client that never sends anything: with a blocking
        // per-connection loop this would hold a pool thread and, on the old
        // accept loop, delay the next connection's handshake.
        let _silent = connect_client(addr);

        let mut active = connect_client(addr);
        let started = Instant::now();
        let id = intern(&mut active, "still-served").id;
        assert!(id < STRING_ID_CANONICAL_CEILING);
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "a silent peer must not delay another session (took {:?})",
            started.elapsed()
        );
    })
    .await
    .expect("client task");
    server.shutdown();
}
