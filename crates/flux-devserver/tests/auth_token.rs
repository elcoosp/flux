//! Pairing-token handshake gate (Appendix D §D.12.1, `flux dev --token`).
//!
//! A server started with `with_auth_token` must reject a host whose `Hello`
//! presents no token or the wrong token, and must accept a host presenting the
//! exact token. A server started without a token accepts any host (the open
//! localhost dev loop).
//!
//! Mirrors the harness in `minimal_patch.rs`: the blocking `tungstenite`
//! connect/read run inside `tokio::task::spawn_blocking` so they do not stall the
//! server's accept task on the async reactor (brittleness 6).

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flux_devserver::{DevServer, RunningServer, ServerConfig};
use flux_ir_serde::{FRAME_ERROR, FRAME_INIT, Frame};
use tokio_tungstenite::tungstenite::stream::MaybeTlsStream;
use tokio_tungstenite::tungstenite::{Message, WebSocket, connect};

const GOOD_SOURCE: &str =
    "compo Hello\n  state count: Int = 0\n  Button(text: \"tap\")\n  Text(text: \"{count}\")\n";

fn scratch_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = std::env::temp_dir().join(format!("flux-authtok-{tag}-{nanos}"));
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("main.flux"), GOOD_SOURCE).expect("write source");
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

fn handshake_frame(client: &mut Client, token: Option<&str>) -> u8 {
    let hello = match token {
        Some(t) => Frame::hello_with_token("ios", "test", &[], t).to_bytes(),
        None => Frame::hello("ios", "test", &[]).to_bytes(),
    };
    client
        .send(Message::Binary(hello.into()))
        .expect("send hello");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    if let MaybeTlsStream::Plain(stream) = client.get_ref() {
        stream
            .set_read_timeout(Some(Duration::from_millis(50)))
            .expect("read timeout");
    }
    loop {
        if std::time::Instant::now() >= deadline {
            panic!("handshake timed out");
        }
        match client.read_message() {
            Ok(Message::Binary(b)) => return *b.get(5).expect("frame kind"),
            Ok(_) => continue,
            Err(_) => {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
        }
    }
}

/// Connects + handshakes inside `spawn_blocking` (so the blocking WS call does
/// not stall the server's accept task), returning the reply frame kind.
async fn handshake(addr: SocketAddr, token: Option<String>) -> u8 {
    tokio::task::spawn_blocking(move || {
        let mut client = connect_client(addr);
        handshake_frame(&mut client, token.as_deref())
    })
    .await
    .expect("handshake task")
}

#[tokio::test]
async fn server_without_token_accepts_any_host() {
    let root = scratch_dir("open");
    let server = start_with_token(&root, None).await;
    let kind = handshake(server.ws_addr(), None).await;
    assert_eq!(kind, FRAME_INIT, "open server accepts host");
    server.shutdown();
}

#[tokio::test]
async fn server_with_token_rejects_missing_token() {
    let root = scratch_dir("missing");
    let server = start_with_token(&root, Some("secret")).await;
    let kind = handshake(server.ws_addr(), None).await;
    assert_eq!(kind, FRAME_ERROR, "missing token rejected with Error");
    server.shutdown();
}

#[tokio::test]
async fn server_with_token_rejects_wrong_token() {
    let root = scratch_dir("wrong");
    let server = start_with_token(&root, Some("secret")).await;
    let kind = handshake(server.ws_addr(), Some("nope".to_owned())).await;
    assert_eq!(kind, FRAME_ERROR, "wrong token rejected with Error");
    server.shutdown();
}

#[tokio::test]
async fn server_with_token_accepts_matching_token() {
    let root = scratch_dir("match");
    let server = start_with_token(&root, Some("secret")).await;
    let kind = handshake(server.ws_addr(), Some("secret".to_owned())).await;
    assert_eq!(kind, FRAME_INIT, "matching token accepted");
    // Decoder tolerates the absent trailing token field (backward compat) and
    // recovers a presented one.
    let none = Frame::from_hello_bytes(&Frame::hello("ios", "test", &[]).to_bytes());
    assert!(none.is_some(), "decoder tolerates absent token");
    let some =
        Frame::from_hello_bytes(&Frame::hello_with_token("ios", "test", &[], "secret").to_bytes());
    assert_eq!(
        some.and_then(|h| h.token),
        Some("secret".to_owned()),
        "decoder recovers the presented token"
    );
    server.shutdown();
}
