//! Pairing-token handshake gate (Appendix D §D.12.1, `flux dev --token`).
//!
//! A server started with `with_auth_token` must reject a host whose `Hello`
//! presents no token or the wrong token, and must accept a host presenting the
//! exact token. A server started without a token accepts any host (the open
//! localhost dev loop).

use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use flux_devserver::{DevServer, RunningServer, ServerConfig};
use flux_ir_serde::{FRAME_ERROR, FRAME_HELLO, FRAME_INIT, Frame, MAGIC, PROTOCOL_VERSION};
use tokio_tungstenite::tungstenite::stream::MaybeTlsStream;
use tokio_tungstenite::tungstenite::{Message, WebSocket, connect};

const GOOD_SOURCE: &str =
    "compo Hello\n  state count: Int = 0\n  Button(text: \"tap\")\n  Text(text: \"{count}\")\n";

fn scratch_dir(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = std::env::temp_dir().join(format!("flux-authtok-{tag}-{nanos}"));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    std::fs::write(dir.join("main.flux"), GOOD_SOURCE).expect("write source");
    dir
}

async fn start_with_token(root: &Path, token: Option<&str>) -> RunningServer {
    let config = ServerConfig::new(root)
        .with_ws_addr("127.0.0.1:0".parse::<SocketAddr>().expect("addr"))
        .with_http_addr("127.0.0.1:0".parse::<SocketAddr>().expect("addr"))
        .with_debounce(Duration::from_millis(20));
    let config = match token {
        Some(t) => config.with_auth_token(t),
        None => config,
    };
    DevServer::start(config).await.expect("server starts")
}

type Client = WebSocket<MaybeTlsStream<std::net::TcpStream>>;

fn connect_client(addr: SocketAddr) -> Client {
    let (socket, _response) = connect(format!("ws://{addr}/")).expect("ws connect");
    socket
}

/// Builds a raw `Hello` frame (MAGIC | version | kind | platform | device |
/// cap_count=0 | token) so the test does not depend on `Frame::hello` gaining a
/// token parameter. Mirrors `flux_ir_serde::HelloFrame::to_bytes`.
fn hello_bytes(platform: &str, device: &str, token: Option<&str>) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&MAGIC.to_le_bytes());
    buf.push(PROTOCOL_VERSION);
    buf.push(FRAME_HELLO);
    let enc = |buf: &mut Vec<u8>, s: &str| {
        let b = s.as_bytes();
        buf.push((b.len() & 0xFF) as u8);
        buf.push(((b.len() >> 8) & 0xFF) as u8);
        buf.extend_from_slice(b);
    };
    enc(&mut buf, platform);
    enc(&mut buf, device);
    buf.push(0); // cap_count (u16 LE) = 0
    buf.push(0);
    match token {
        Some(t) => enc(&mut buf, t),
        None => {
            buf.push(0);
            buf.push(0);
        }
    }
    buf
}

fn frame_kind(bytes: &[u8]) -> Option<u8> {
    bytes.get(5).copied()
}

fn read_frame(client: &mut Client, timeout: Duration) -> Option<Vec<u8>> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if std::time::Instant::now() >= deadline {
            return None;
        }
        match client.read_message() {
            Ok(Message::Binary(b)) => return Some(b.to_vec()),
            Ok(_) => continue,
            Err(_) => return None,
        }
    }
}

#[tokio::test]
async fn server_without_token_accepts_any_host() {
    let root = scratch_dir("open");
    let server = start_with_token(&root, None).await;
    let mut client = connect_client(server.ws_addr());
    client
        .send(Message::Binary(hello_bytes("ios", "test", None).into()))
        .expect("send hello");
    let frame = read_frame(&mut client, Duration::from_secs(5)).expect("init");
    assert_eq!(
        frame_kind(&frame),
        Some(FRAME_INIT),
        "open server accepts host"
    );
}

#[tokio::test]
async fn server_with_token_rejects_missing_token() {
    let root = scratch_dir("missing");
    let server = start_with_token(&root, Some("secret")).await;
    let mut client = connect_client(server.ws_addr());
    client
        .send(Message::Binary(hello_bytes("ios", "test", None).into()))
        .expect("send hello");
    let frame = read_frame(&mut client, Duration::from_secs(5)).expect("error frame");
    assert_eq!(
        frame_kind(&frame),
        Some(FRAME_ERROR),
        "missing token must be rejected with an Error frame"
    );
}

#[tokio::test]
async fn server_with_token_rejects_wrong_token() {
    let root = scratch_dir("wrong");
    let server = start_with_token(&root, Some("secret")).await;
    let mut client = connect_client(server.ws_addr());
    client
        .send(Message::Binary(
            hello_bytes("ios", "test", Some("nope")).into(),
        ))
        .expect("send hello");
    let frame = read_frame(&mut client, Duration::from_secs(5)).expect("error frame");
    assert_eq!(
        frame_kind(&frame),
        Some(FRAME_ERROR),
        "wrong token must be rejected with an Error frame"
    );
}

#[tokio::test]
async fn server_with_token_accepts_matching_token() {
    let root = scratch_dir("match");
    let server = start_with_token(&root, Some("secret")).await;
    let mut client = connect_client(server.ws_addr());
    client
        .send(Message::Binary(
            hello_bytes("ios", "test", Some("secret")).into(),
        ))
        .expect("send hello");
    let frame = read_frame(&mut client, Duration::from_secs(5)).expect("init");
    assert_eq!(
        frame_kind(&frame),
        Some(FRAME_INIT),
        "matching token must be accepted"
    );
    // `Frame::hello` (no token) must still decode a frame that carried no token,
    // proving the decoder tolerates the absent trailing field (backward compat).
    let roundtrip = Frame::from_hello_bytes(&hello_bytes("ios", "test", None));
    assert!(roundtrip.is_some(), "decoder tolerates absent token field");
}
