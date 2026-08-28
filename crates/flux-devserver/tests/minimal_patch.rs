//! Minimal-patch (signal-deps) server behaviour (ADR-0027 Phase 2, FA-DEVSERVER).
//!
//! Drives the real dev server over a real WebSocket and asserts that a host
//! dispatch report produces patches addressed *only* to the nodes that read the
//! written signal, plus the degradation path where no `signal_deps` are present
//! and the server must not ship a minimal delta at all.
//!
//! Mirrors the harness in `integration.rs` / `full_pipeline.rs`: boot the server,
//! `Hello` → `Init`, then manipulate the pipeline and observe the wire.

use std::fs;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flux_devserver::{DevServer, DispatchReport, NodeSignalDeps, RunningServer, ServerConfig};
use flux_ir_serde::{FRAME_DELTA, Frame};
use flux_syntax::{HandlerId, NodeId, Patch, SignalId};
use tokio_tungstenite::tungstenite::stream::MaybeTlsStream;
use tokio_tungstenite::tungstenite::{Message, WebSocket, connect};

const GOOD_SOURCE: &str =
    "compo Hello\n  state count: Int = 0\n  Button(text: \"tap\")\n  Text(text: \"{count}\")\n";

/// A unique scratch directory (the crate has no `tempfile` dev-dependency).
fn scratch_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = std::env::temp_dir().join(format!("flux-minpatch-{tag}-{nanos}"));
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

/// Sends a `Hello` handshake and returns the `Init` frame bytes, skipping
/// heartbeats. Returns the live `client` so the caller can keep using it.
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

/// Direct child node ids of a `NodeRef` (the init root's first level already
/// covers the component-instance children).
fn child_node_ids(node: &flux_syntax::NodeRef) -> Vec<NodeId> {
    node.children.iter().flat_map(|c| c.node_ids()).collect()
}

/// Connects a client and completes the handshake, returning the live client.
async fn connected(addr: SocketAddr) -> Client {
    tokio::task::spawn_blocking(move || {
        let mut client = connect_client(addr);
        assert_eq!(
            frame_type(&handshake(&mut client)),
            Some(flux_ir_serde::FRAME_INIT)
        );
        client
    })
    .await
    .expect("handshake task")
}

#[tokio::test]
async fn minimal_patch_shipped_only_for_dependents() {
    let root = scratch_dir("scope");
    let file = root.join("hello.flux");
    fs::write(&file, GOOD_SOURCE).expect("write source");
    let server = start(&root).await;
    let addr = server.ws_addr();

    let client = connected(addr).await;
    // Read the Init to recover the *real* node ids the server assigned.
    let (client, init_bytes) = tokio::task::spawn_blocking(move || {
        let mut client = client;
        let init = handshake(&mut client);
        (client, init)
    })
    .await
    .expect("init read");
    let init = Frame::from_init_bytes(&init_bytes).expect("decodes as Init");

    // The init root's direct children are the component-instance nodes; assert at
    // least two (Button + Text) so the scenario is meaningful.
    let child_ids = child_node_ids(&init.root);
    assert!(
        child_ids.len() >= 2,
        "init must expose at least the Button and Text nodes"
    );

    // Assign `count` (signal 100) as the dep for the *first* child only. A real
    // tree would carry deps on every reading node; here we pin the scope so the
    // assertion is exact: writing 100 must touch exactly one node.
    let reader = child_ids[0];
    let non_reader = child_ids[1];
    server.set_signal_deps(Some(vec![NodeSignalDeps {
        id: reader,
        signal_deps: vec![SignalId::from(100u32)],
    }]));

    // Host reports a dispatch that wrote signal 100.
    let report = DispatchReport {
        handler_id: HandlerId::from(1u32),
        written: SignalId::from(100u32),
    };
    let (client, ()) = tokio::task::spawn_blocking(move || {
        let mut client = client;
        client
            .send(Message::Binary(report.to_bytes().into()))
            .expect("report sent");
        (client, ())
    })
    .await
    .expect("send report");

    // The server should broadcast exactly one Update patch for `reader`.
    let (_, frame) =
        tokio::task::spawn_blocking(move || read_frame_blocking(client, Duration::from_secs(5)))
            .await
            .expect("read task");
    let frame = frame.expect("delta frame arrives");
    assert_eq!(
        frame_type(&frame),
        Some(FRAME_DELTA),
        "dispatch emits Delta"
    );
    let delta = Frame::from_delta_bytes(&frame).expect("decodes as Delta");
    assert_eq!(
        delta.patches.len(),
        1,
        "patch scope must equal |dependents[written]| = 1"
    );
    match &delta.patches[0] {
        Patch::Update { id, .. } => {
            assert_eq!(*id, reader, "the only patch targets the reading node");
            assert_ne!(*id, non_reader, "the non-reading node is never touched");
        }
        other => panic!("expected an Update patch, got {other:?}"),
    }
    server.shutdown();
}

/// Reads the next non-heartbeat frame on the blocking pool, returning both the
/// client (for chained use) and the frame.
fn read_frame_blocking(mut client: Client, timeout: Duration) -> (Client, Option<Vec<u8>>) {
    let frame = next_frame(&mut client, timeout);
    (client, frame)
}

#[tokio::test]
async fn no_patch_when_signal_has_no_dependents() {
    let root = scratch_dir("noop");
    let file = root.join("hello.flux");
    fs::write(&file, GOOD_SOURCE).expect("write source");
    let server = start(&root).await;
    let addr = server.ws_addr();

    let client = connected(addr).await;
    let (client, init_bytes) = tokio::task::spawn_blocking(move || {
        let mut client = client;
        let init = handshake(&mut client);
        (client, init)
    })
    .await
    .expect("init read");
    let init = Frame::from_init_bytes(&init_bytes).expect("decodes as Init");
    let child_ids = child_node_ids(&init.root);

    // Only `reader` reads signal 100; `lonely` (999) has no readers.
    let reader = child_ids[0];
    server.set_signal_deps(Some(vec![NodeSignalDeps {
        id: reader,
        signal_deps: vec![SignalId::from(100u32)],
    }]));

    let report = DispatchReport {
        handler_id: HandlerId::from(2u32),
        written: SignalId::from(999u32), // nothing reads this
    };
    let client = tokio::task::spawn_blocking(move || {
        let mut client = client;
        client
            .send(Message::Binary(report.to_bytes().into()))
            .expect("report sent");
        client
    })
    .await
    .expect("send report");

    // No frame should arrive: `noop_dispatch` ships nothing.
    let (_, frame) =
        tokio::task::spawn_blocking(move || read_frame_blocking(client, Duration::from_secs(1)))
            .await
            .expect("read task");
    assert!(
        frame.is_none(),
        "a dispatch writing a signal with no dependents ships no patch"
    );
    server.shutdown();
}

#[tokio::test]
async fn degradation_ships_no_minimal_patch_without_signal_deps() {
    let root = scratch_dir("degrade");
    let file = root.join("hello.flux");
    fs::write(&file, GOOD_SOURCE).expect("write source");
    // No `set_signal_deps` call → index inactive → degrade to coarse frame.
    let server = start(&root).await;
    let addr = server.ws_addr();

    let client = connected(addr).await;
    let report = DispatchReport {
        handler_id: HandlerId::from(3u32),
        written: SignalId::from(100u32),
    };
    let client = tokio::task::spawn_blocking(move || {
        let mut client = client;
        client
            .send(Message::Binary(report.to_bytes().into()))
            .expect("report sent");
        client
    })
    .await
    .expect("send report");

    // With no dependency data the server must not fabricate a minimal patch; it
    // stays silent (the host keeps its coarse-frame reconcile walk).
    let (_, frame) =
        tokio::task::spawn_blocking(move || read_frame_blocking(client, Duration::from_secs(1)))
            .await
            .expect("read task");
    assert!(
        frame.is_none(),
        "without signal_deps the server degrades and ships no minimal patch"
    );
    server.shutdown();
}

#[tokio::test]
async fn previous_good_tree_retained_after_dispatch_path() {
    // Regression guard: exercising the dispatch path must not disturb the file
    // save → Delta pipeline (the 20 existing tests cover that; here we assert the
    // dispatch path leaves `has_tree` intact).
    let root = scratch_dir("retain");
    let file = root.join("hello.flux");
    fs::write(&file, GOOD_SOURCE).expect("write source");
    let server = start(&root).await;
    assert!(server.has_tree(), "tree compiled at start-up");

    let addr = server.ws_addr();
    let client = connected(addr).await;
    let report = DispatchReport {
        handler_id: HandlerId::from(4u32),
        written: SignalId::from(100u32),
    };
    let client = tokio::task::spawn_blocking(move || {
        let mut client = client;
        client
            .send(Message::Binary(report.to_bytes().into()))
            .expect("report sent");
        client
    })
    .await
    .expect("send report");

    tokio::task::spawn_blocking(move || {
        let _ = read_frame_blocking(client, Duration::from_secs(1));
    })
    .await
    .expect("drain");

    assert!(
        server.has_tree(),
        "the tree survives the dispatch-report path"
    );
    server.shutdown();
}
