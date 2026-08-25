//! Headless full-pipeline end-to-end test (PE-E, FLUX-022 / FLUX-019).
//!
//! Drives the *whole* Rust pipeline against the canonical `examples/counter`
//! app without any native runtime:
//!
//! 1. `flux dev` — boot [`DevServer`] over the example, complete the `Hello`
//!    handshake and decode the `Init` frame (parse → type-check → lower →
//!    serialize).
//! 2. Edit `main.flux` and assert a `Delta` frame with at least one patch is
//!    shipped and decodes cleanly (recompile → diff → serialize).
//! 3. `flux build --platform ios` — assert the release codegen path emits a
//!    non-empty `Generated/*.swift` containing `Button` (parser → type-check →
//!    lower → codegen), proving a consumer can run the whole pipeline
//!    headlessly.
//!
//! The example app under test lives at `examples/counter/` (relative to the
//! workspace root, two levels above this crate).

use std::fs;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
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

/// A unique scratch directory (the crate has no `tempfile` dev-dependency).
fn scratch_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = std::env::temp_dir().join(format!("flux-fullpipeline-{tag}-{nanos}"));
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Copies the canonical example into `root` so it can be mutated freely.
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

/// Locates the `flux` CLI binary: `CARGO_BIN_EXE_flux`, a prebuilt
/// `target/{debug,release}/flux`, or the `flux` on `PATH`.
fn find_flux_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_flux") {
        return Some(PathBuf::from(p));
    }
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent()?;
        loop {
            for sub in ["debug", "release"] {
                let candidate = dir.join(sub).join("flux");
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
            dir = dir.parent()?;
        }
    }
    None
}

/// Runs `flux build --platform ios` against `root`, returning whether it exited
/// successfully. Falls back to `cargo run -p flux-cli` when no prebuilt `flux`
/// is discoverable (CI builds the binary on demand).
fn run_flux_build_ios(root: &Path) -> bool {
    let platform = "ios";
    let root_arg = root.to_string_lossy().into_owned();
    if let Some(bin) = find_flux_bin() {
        Command::new(bin)
            .arg("build")
            .arg(platform)
            .arg("--root")
            .arg(&root_arg)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else {
        Command::new("cargo")
            .args(["run", "-q", "-p", "flux-cli", "--"])
            .arg("build")
            .arg(platform)
            .arg("--root")
            .arg(&root_arg)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

#[tokio::test]
async fn devserver_serves_init_for_example_app() {
    let root = scratch_dir("init");
    copy_example(&root);
    let server = start(&root).await;
    let addr = server.ws_addr();

    let frame = tokio::task::spawn_blocking(move || {
        let mut client = connect_client(addr);
        handshake(&mut client)
    })
    .await
    .expect("handshake task");

    assert_eq!(
        frame_type(&frame),
        Some(FRAME_INIT),
        "the example app must answer Hello with an Init frame"
    );
    let init = Frame::from_init_bytes(&frame).expect("decodes as Init");
    // The synthetic root wraps the real `Counter` component, so a populated
    // tree carries at least one child node.
    assert!(
        !init.root.children.is_empty(),
        "Init must decode to a non-empty tree (the counter component)"
    );
    assert!(
        !init.source_map.is_empty(),
        "Init must carry the example's source map (main.flux)"
    );
    server.shutdown();
}

#[tokio::test]
async fn edit_example_app_emits_delta_frame() {
    let root = scratch_dir("delta");
    copy_example(&root);
    let file = root.join("main.flux");
    let server = start(&root).await;
    let addr = server.ws_addr();

    let mut client = tokio::task::spawn_blocking(move || {
        let mut client = connect_client(addr);
        assert_eq!(frame_type(&handshake(&mut client)), Some(FRAME_INIT));
        client
    })
    .await
    .expect("handshake task");

    tokio::time::sleep(Duration::from_millis(100)).await;
    // Editing the increment changes a node's prop, so the recompile must ship
    // a Delta carrying at least one patch.
    fs::write(&file, EXAMPLE_EDITED).expect("save edit");

    let frame =
        tokio::task::spawn_blocking(move || next_frame(&mut client, Duration::from_secs(5)))
            .await
            .expect("read task")
            .expect("delta frame arrives");
    assert_eq!(
        frame_type(&frame),
        Some(FRAME_DELTA),
        "edit must ship Delta"
    );
    let delta = Frame::from_delta_bytes(&frame).expect("decodes as Delta");
    assert!(
        !delta.patches.is_empty(),
        "an edit to the counter handler must produce at least one patch"
    );
    server.shutdown();
}

#[tokio::test]
async fn release_build_emits_generated_swift_for_example() {
    let root = scratch_dir("build");
    copy_example(&root);
    // Start from a clean slate so we assert what this build produced.
    let _ = fs::remove_dir_all(root.join("platforms"));

    assert!(
        run_flux_build_ios(&root),
        "flux build --platform ios must compile the example app"
    );

    let generated = root.join("platforms/ios/Generated");
    assert!(
        generated.is_dir(),
        "flux build must write platforms/ios/Generated/"
    );
    let swift = std::fs::read_dir(&generated)
        .expect("read Generated dir")
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().and_then(|e| e.to_str()) == Some("swift"))
        .expect("flux build must emit a .swift file");
    let body = fs::read_to_string(&swift).expect("read generated swift");
    assert!(!body.is_empty(), "generated swift must be non-empty");
    assert!(
        body.contains("Button"),
        "generated swift must render the Button adapter (got: {body})"
    );
}

/// `main.flux` with the increment constant changed from `1` to `5` — an edit
/// that alters a node prop and therefore must produce a `Delta`.
const EXAMPLE_EDITED: &str = "\
// main.flux — edited copy of examples/counter for the delta e2e test.
component Counter {
    state count: Int = 0

    Column(gap: 8.0) {
        Text(text: \"tapped ${count} times\")
        Button(text: \"Increment\", onClick: fn() { count = count + 5 })
    }
}
";
