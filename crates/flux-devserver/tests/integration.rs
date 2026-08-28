//! Acceptance tests for the `flux-devserver` WebSocket dev channel (FLUX-019).
//!
//! Each test drives the real server over a real TCP socket with a
//! `tokio-tungstenite`/`tungstenite` client, exactly as an iOS/Android host
//! would: `Hello` → `Init`, then a file save → `Delta`.

use std::fs;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flux_devserver::{DevServer, RunningServer, ServerConfig};
use flux_ir_serde::{FRAME_DELTA, FRAME_ERROR, FRAME_INIT, Frame};
use tokio_tungstenite::tungstenite::stream::MaybeTlsStream;
use tokio_tungstenite::tungstenite::{Message, WebSocket, connect};

const GOOD_SOURCE: &str = "compo Hello\n  state count: Int = 0\n  Button(text: \"tap\")\n";
const GOOD_SOURCE_EDITED: &str = "compo Hello\n  state count: Int = 0\n  Button(text: \"tap!\")\n";
const MALFORMED_SOURCE: &str = "compo Hello Button(text: ";

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

/// Sends a `Hello` handshake frame and returns the frame bytes of the reply,
/// skipping heartbeats.
fn handshake(client: &mut Client) -> Vec<u8> {
    let hello = Frame::hello("ios", "test-harness", &[]).to_bytes();
    client
        .send(Message::Binary(hello.into()))
        .expect("send hello");
    next_frame(client, Duration::from_secs(5)).expect("init frame")
}

/// Connects a client and completes the handshake, returning the live client.
async fn connected(addr: SocketAddr) -> Client {
    tokio::task::spawn_blocking(move || {
        let mut client = connect_client(addr);
        assert_eq!(frame_type(&handshake(&mut client)), Some(FRAME_INIT));
        client
    })
    .await
    .expect("handshake task")
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
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
            {
                continue;
            }
            Err(_) => return None,
        }
    }
    None
}

fn frame_type(bytes: &[u8]) -> Option<u8> {
    bytes.get(5).copied()
}

#[tokio::test]
async fn handshake_hello_returns_init_frame_quickly() {
    let root = scratch_dir("handshake");
    fs::write(root.join("hello.flux"), GOOD_SOURCE).expect("write source");
    let server = start(&root).await;
    let addr = server.ws_addr();

    let elapsed = tokio::task::spawn_blocking(move || {
        let mut client = connect_client(addr);
        let started = Instant::now();
        let frame = handshake(&mut client);
        let elapsed = started.elapsed();
        assert_eq!(frame_type(&frame), Some(FRAME_INIT), "reply must be Init");
        let init = Frame::from_init_bytes(&frame).expect("decodes as Init");
        assert!(
            !init.string_table.is_empty(),
            "Init carries the populated string table (Gap G3)"
        );
        elapsed
    })
    .await
    .expect("client task");

    // Acceptance budget: Hello → Init in under 10 ms (the tree is compiled at
    // start-up, so the handshake is a lookup plus one frame encode).
    assert!(
        elapsed < Duration::from_millis(10),
        "handshake round trip took {elapsed:?}, budget is 10ms"
    );
    server.shutdown();
}

#[tokio::test]
async fn save_emits_delta_frame() {
    let root = scratch_dir("save");
    let file = root.join("hello.flux");
    fs::write(&file, GOOD_SOURCE).expect("write source");
    let server = start(&root).await;
    let addr = server.ws_addr();

    let mut client = connected(addr).await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    fs::write(&file, GOOD_SOURCE_EDITED).expect("save edit");

    let frame =
        tokio::task::spawn_blocking(move || next_frame(&mut client, Duration::from_secs(5)))
            .await
            .expect("read task")
            .expect("delta frame arrives");
    assert_eq!(
        frame_type(&frame),
        Some(FRAME_DELTA),
        "save must ship Delta"
    );
    let delta = Frame::from_delta_bytes(&frame).expect("decodes as Delta");
    assert!(
        !delta.patches.is_empty(),
        "a text edit must produce at least one patch"
    );
    server.shutdown();
}

#[tokio::test]
async fn reconnect_resends_init() {
    let root = scratch_dir("reconnect");
    fs::write(root.join("hello.flux"), GOOD_SOURCE).expect("write source");
    let server = start(&root).await;
    let addr = server.ws_addr();

    tokio::task::spawn_blocking(move || {
        let mut first = connect_client(addr);
        assert_eq!(frame_type(&handshake(&mut first)), Some(FRAME_INIT));
        drop(first);
        let mut second = connect_client(addr);
        assert_eq!(
            frame_type(&handshake(&mut second)),
            Some(FRAME_INIT),
            "reconnect must resend Init"
        );
    })
    .await
    .expect("client task");
    server.shutdown();
}

#[tokio::test]
async fn malformed_source_emits_error_frame() {
    let root = scratch_dir("malformed");
    let file = root.join("hello.flux");
    fs::write(&file, GOOD_SOURCE).expect("write source");
    let server = start(&root).await;
    let mut client = connected(server.ws_addr()).await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    fs::write(&file, MALFORMED_SOURCE).expect("save malformed");

    let frame =
        tokio::task::spawn_blocking(move || next_frame(&mut client, Duration::from_secs(5)))
            .await
            .expect("read task")
            .expect("error frame arrives");
    assert_eq!(
        frame_type(&frame),
        Some(FRAME_ERROR),
        "malformed source must ship Error, not Delta"
    );
    let err = Frame::from_error_bytes(&frame).expect("decodes as Error");
    assert!(!err.message.is_empty(), "Error frame carries a diagnostic");
    server.shutdown();
}

#[tokio::test]
async fn previous_good_tree_is_retained_across_a_malformed_save() {
    let root = scratch_dir("retained");
    let file = root.join("hello.flux");
    fs::write(&file, GOOD_SOURCE).expect("write source");
    let server = start(&root).await;
    let mut client = connected(server.ws_addr()).await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    fs::write(&file, MALFORMED_SOURCE).expect("save malformed");
    let mut client = tokio::task::spawn_blocking(move || {
        let frame = next_frame(&mut client, Duration::from_secs(5)).expect("error frame");
        assert_eq!(frame_type(&frame), Some(FRAME_ERROR));
        client
    })
    .await
    .expect("error task");

    // The failed compile did not clobber the tree: the next good save still
    // diffs against it and ships a Delta rather than a fresh Init.
    tokio::time::sleep(Duration::from_millis(50)).await;
    fs::write(&file, GOOD_SOURCE_EDITED).expect("save recovery");
    let frame =
        tokio::task::spawn_blocking(move || next_frame(&mut client, Duration::from_secs(5)))
            .await
            .expect("read task")
            .expect("delta after recovery");
    assert_eq!(
        frame_type(&frame),
        Some(FRAME_DELTA),
        "recovery diffs against the retained good tree"
    );
    server.shutdown();
}

#[tokio::test]
async fn asset_server_serves_files_under_the_project_root() {
    let root = scratch_dir("assets");
    fs::write(root.join("hello.flux"), GOOD_SOURCE).expect("write source");
    fs::write(root.join("logo.txt"), "flux").expect("write asset");
    let server = start(&root).await;
    let base = format!("http://{}", server.http_addr());

    // `#[tokio::test]` runs a current-thread runtime, so the blocking HTTP
    // client must run on the blocking pool or it starves the server task.
    let url = format!("{base}/assets/logo.txt");
    let body = tokio::task::spawn_blocking(move || http_get(&url))
        .await
        .expect("fetch task")
        .expect("asset fetch");
    assert!(
        body.contains("flux"),
        "asset body should carry the file contents, got: {body}"
    );

    let url = format!("{base}/assets/nope.txt");
    let missing = tokio::task::spawn_blocking(move || http_get(&url))
        .await
        .expect("fetch task")
        .expect("missing fetch");
    assert!(
        missing.starts_with("HTTP/1.1 404"),
        "unknown asset must 404, got: {missing}"
    );
    server.shutdown();
}

/// Minimal blocking HTTP/1.1 GET (the crate has no HTTP client dependency).
fn http_get(url: &str) -> Option<String> {
    use std::io::{Read, Write};
    let rest = url.strip_prefix("http://")?;
    let (authority, path) = rest.split_once('/')?;
    let mut stream = std::net::TcpStream::connect(authority).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    write!(
        stream,
        "GET /{path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    // `read_to_string` returns a timeout error once the server has finished
    // writing and gone quiet; the bytes read so far are still the response.
    let mut raw = Vec::new();
    let _ = stream.read_to_end(&mut raw);
    Some(String::from_utf8_lossy(&raw).into_owned())
}
