//! T-206 regression test: broadcast must not race the pipeline lock.
//!
//! Audit H16: previously `compile_and_broadcast` released the pipeline lock
//! before calling `shared.broadcast(frame)`. A Hello arriving in that window
//! could subscribe, receive `Init(new)` and then a `Delta(old→new)` derived
//! from the *previous* compile — corrupting its tree. The fix broadcasts
//! while the lock is still held.

use std::fs;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flux_devserver::{DevServer, RunningServer, ServerConfig};
use flux_ir_serde::{FRAME_DELTA, FRAME_INIT, Frame};
use tokio_tungstenite::tungstenite::stream::MaybeTlsStream;
use tokio_tungstenite::tungstenite::{Message, WebSocket, connect};

/// The canonical example app on disk, resolved from this crate's manifest dir.
fn example_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/counter")
        .canonicalize()
        .expect("examples/counter must exist at the workspace root")
}

/// A unique scratch directory.
fn scratch_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = std::env::temp_dir().join(format!("flux-watch-race-{tag}-{nanos}"));
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Copies the canonical example into `root`.
fn copy_example(root: &Path) {
    let src = example_dir();
    for entry in fs::read_dir(&src).expect("read example dir") {
        let entry = entry.expect("example entry");
        let dest = root.join(entry.file_name());
        fs::copy(entry.path(), dest).expect("copy example file");
    }
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

/// Sends `Hello` and returns the first non-heartbeat frame bytes.
fn handshake(client: &mut Client) -> Vec<u8> {
    let hello = Frame::hello("ios", "test-harness", &[]).to_bytes();
    client
        .send(Message::Binary(hello.into()))
        .expect("send hello");
    next_frame(client, Duration::from_secs(5)).expect("init frame")
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

/// Triggers N edits to the source file to simulate a burst of saves.
fn trigger_burst(file: &Path, edits: usize) {
    for i in 0..edits {
        let src = format!(
            "compo Counter\n  state count: Int = 0\n  Column(gap: 8.0) {{\n    Text(text: \"tapped ${{count}} times\")\n    Button(text: \"Increment\", onClick: fn() {{ count = count + {i} }})\n  }}\n"
        );
        fs::write(file, src).expect("save edit");
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// Verifies that a client connecting during a burst of recompiles receives
/// Init first and never a Delta with a stale sequence number.
#[tokio::test]
async fn client_never_sees_delta_before_init_during_burst() {
    let root = scratch_dir("race");
    copy_example(&root);
    let file = root.join("main.flux");
    let server = start(&root).await;
    let addr = server.ws_addr();

    // Spawn the edit burst on a background thread so the client connects
    // mid-burst and observes the race window.
    let file_for_thread = file.clone();
    let edit_handle = std::thread::spawn(move || {
        trigger_burst(&file_for_thread, 5);
    });

    // Give the burst a moment to start, so the client connects mid-recompile.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Connect and handshake — this is where the race window used to matter.
    let (mut client, init_bytes) = tokio::task::spawn_blocking(move || {
        let mut client = connect_client(addr);
        let init = handshake(&mut client);
        (client, init)
    })
    .await
    .expect("handshake task");

    // The first frame must be Init, never a Delta.
    assert_eq!(
        frame_type(&init_bytes),
        Some(FRAME_INIT),
        "client must receive Init first, before any Delta (audit H16)"
    );

    // Read subsequent frames; each Delta's seq must be strictly greater than
    // the Init's seq — no stale Delta(old→new) can precede a fresh Init.
    let init = Frame::from_init_bytes(&init_bytes).expect("decodes as Init");
    let mut last_seq = init.seq;
    let mut received_delta = false;
    let mut frames = Vec::new();
    {
        // Collect all remaining frames in a single blocking task so `client`
        // is moved exactly once.
        frames = tokio::task::spawn_blocking(move || {
            let mut out = Vec::new();
            for _ in 0..10 {
                match next_frame(&mut client, Duration::from_secs(2)) {
                    Some(frame) => out.push(frame),
                    None => break,
                }
            }
            out
        })
        .await
        .expect("read task");
    }
    for frame in &frames {
        if frame_type(frame) == Some(FRAME_DELTA) {
            received_delta = true;
            let delta = Frame::from_delta_bytes(frame).expect("decodes as Delta");
            assert!(
                delta.seq > last_seq,
                "Delta seq {} must be > Init seq {} (audit H16: \
                 no Init(new)→Delta(old→new) reordering)",
                delta.seq,
                last_seq
            );
            last_seq = delta.seq;
        }
    }
    let _ = received_delta;

    let _ = edit_handle.join();
    server.shutdown();
}
