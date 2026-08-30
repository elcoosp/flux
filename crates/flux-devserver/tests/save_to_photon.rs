//! LANE-H save→photon end-to-end harness (FLUX-073).
//!
//! Drives the *real* dev server (`DevServer::start`) plus a headless
//! loopback WebSocket client (no device) and measures the only number a
//! developer actually feels — **save → pixels**:
//!
//! ```text
//! notify event → debounce → parse → type-check → lower → diff → serialize
//!            → wire (loopback) → host decode/apply → frame received
//! ```
//!
//! It edits a fixture `.flux` file `SAMPLES` (≥ 50) times, recording the
//! wall-clock from `fs::write` to the host receiving the final `Delta` frame,
//! then reports **p50 / p99** and a JSON [`MetricRecord`]. The p95 is checked
//! against the §3.10 budget via [`flux_perf_harness`].
//!
//! This is the **loopback** baseline: it excludes WiFi RTT, on-device decode,
//! signal re-eval, view mutation, layout and raster. Those are the dominant
//! remaining costs and are labelled as out-of-scope until a physical-device /
//! simulator runner exists (then add `Scenario::{IosLanE2e,AndroidLanE2e}`).
//!
//! Harness pattern mirrors `full_pipeline.rs` / `auth_token.rs`: the blocking
//! `tungstenite` connect/read run inside `tokio::task::spawn_blocking` so they
//! never stall the server's accept task on the async reactor (brittleness 6).

use std::fs;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flux_devserver::{DevServer, RunningServer, ServerConfig};
use flux_ir_serde::{FRAME_DELTA, FRAME_INIT};
use flux_perf_harness::{
    Budgets, LatencyMs, MetricKind, MetricRecord, MetricSample, Scenario, evaluate,
};
use tokio_tungstenite::tungstenite::stream::MaybeTlsStream;
use tokio_tungstenite::tungstenite::{Message, WebSocket, connect};

/// File-watch debounce window used by the harness. The production default is
/// 50 ms (`DEFAULT_DEBOUNCE`); a tightened 10 ms keeps the loopback number
/// representative of a watcher-optimized server. The dominant cost the issue
/// calls out is exactly this window, so it is reported alongside the numbers.
const DEBOUNCE: Duration = Duration::from_millis(10);
/// Samples per tree size. The issue requires N ≥ 50 to get a stable tail.
const SAMPLES: usize = 60;
/// `(tree_size, leaf_count)` scale points, matching the §3.10 "50-node" and
/// "~1k-node" wording. `tree_size = leaf_count + 3` (root + `Column` + `Button`).
const TREE_SIZES: &[(u64, u32)] = &[(50, 47), (1000, 997)];

/// A unique scratch directory (the crate has no `tempfile` dev-dependency).
fn scratch_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = std::env::temp_dir().join(format!("flux-s2p-{tag}-{nanos}"));
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Generates a synthetic `leaf_count`-leaf Flux app: a root `Counter` component
/// holding a `Column` whose body is `leaf_count` sibling `Text` primitives plus
/// one `Button` carrying an `onPress` handler. Each `Text` lowers to exactly one
/// primitive node, so the tree carries `leaf_count + 3` nodes — production-scale
/// structure without a real fixture.
///
/// `edit_tag` (when `Some(k)`) rewrites the handler body's increment constant to
/// a distinct value each save. A handler-body change is detected by the
/// differencer's `handlers_equal` check and ships a state-preserving `Patch::Handler`
/// `Delta` (the same trigger the existing `edit_example_app_emits_delta_frame`
/// test relies on). `Text` `text` edits live inside a prop thunk the node-level
/// prop hash does not capture, and literal layout props (`gap`) are likewise not
/// in the diff, so only the handler change reliably produces a frame. The tree
/// size is unchanged across edits, so the e2e sample measures a fixed-size
/// save→photon path (never `Unchanged`).
fn synthetic_app(leaf_count: u32, edit_tag: Option<u32>) -> String {
    let inc = match edit_tag {
        // Edits vary the increment so every save differs from the baseline AND
        // from the previous save (the differ would otherwise return `Unchanged`
        // and ship no frame). The baseline uses a sentinel value no edit takes.
        Some(k) => 1 + (k % 49),
        None => 0,
    };
    let mut src =
        String::from("compo Counter\n    state count: Int = 0\n\n    Column(gap: 8.0) {\n");
    for i in 0..leaf_count {
        // A distinct prop string per leaf, so every node is uniquely
        // identifiable and the differ has real (if unchanged) structure to scan.
        src.push_str(&format!("        Text(text: \"label-{i}\")\n"));
    }
    src.push_str(&format!(
        "        Button(text: \"tap\", onPress: fn() {{ count = count + {inc} }})\n"
    ));
    src.push_str("    }\n\n");
    src
}

async fn start(leaf_count: u32, root: &Path) -> RunningServer {
    let config = ServerConfig::new(root)
        .with_ws_addr("127.0.0.1:0".parse::<SocketAddr>().expect("addr"))
        .with_http_addr("127.0.0.1:0".parse::<SocketAddr>().expect("addr"))
        .with_debounce(DEBOUNCE);
    // Seed the tree so the first handshake answers with a full Init.
    let _ = leaf_count;
    DevServer::start(config).await.expect("server starts")
}

type Client = WebSocket<MaybeTlsStream<std::net::TcpStream>>;

fn connect_client(addr: SocketAddr) -> Client {
    let (socket, _response) = connect(format!("ws://{addr}/")).expect("ws connect");
    socket
}

/// Sends `Hello` and returns the first non-heartbeat frame bytes (the `Init`).
fn handshake(client: &mut Client) -> Vec<u8> {
    let hello = flux_ir_serde::Frame::hello("ios", "save-to-photon-harness", &[]).to_bytes();
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

/// Saves `source` over `file` and returns the millisecond wall-clock from the
/// save to the host receiving the resulting `Delta` frame.
fn measure_one(client: &mut Client, _server: &RunningServer, file: &Path, source: &str) -> f64 {
    let start = Instant::now();
    fs::write(file, source).expect("save edit");
    let frame = next_frame(client, Duration::from_secs(5)).expect("delta frame arrives");
    assert_eq!(
        frame_type(&frame),
        Some(FRAME_DELTA),
        "every edit must ship a Delta frame"
    );
    start.elapsed().as_secs_f64() * 1_000.0
}

#[tokio::test]
async fn save_to_photon_e2e_reports_p50_p99() {
    for &(tree_size, leaves) in TREE_SIZES {
        let root = scratch_dir(&format!("tree-{tree_size}"));
        let file = root.join("main.flux");
        fs::write(&file, synthetic_app(leaves, None)).expect("write baseline source");

        let server = start(leaves, &root).await;
        let addr = server.ws_addr();

        // One persistent client does the handshake then all edits so the
        // measurement covers the real `notify` → apply path for every save.
        // `server` is moved into the blocking task and shut down there so the
        // task is `'static` (spawn_blocking), and the outer scope owns only the
        // aggregated result.
        let samples = tokio::task::spawn_blocking(move || {
            let mut client = connect_client(addr);
            assert_eq!(
                frame_type(&handshake(&mut client)),
                Some(FRAME_INIT),
                "harness must receive Init on handshake"
            );
            // Settle so the watcher is fully subscribed before we start editing
            // (mirrors `full_pipeline.rs`, which sleeps 100ms before its first
            // save). Without this the first `fs::write` can race the watcher's
            // startup and be missed, leaving the pipeline on the baseline source.
            std::thread::sleep(Duration::from_millis(150));
            let mut samples = Vec::with_capacity(SAMPLES);
            for k in 0..SAMPLES {
                let src = synthetic_app(leaves, Some(k as u32));
                samples.push(measure_one(&mut client, &server, &file, &src));
                // Settle so consecutive saves land in separate debounce windows
                // and each is measured as its own save→photon sample.
                std::thread::sleep(Duration::from_millis(20));
            }
            server.shutdown();
            samples
        })
        .await
        .expect("harness task");

        let record = MetricRecord::new(
            Scenario::LoopbackE2e,
            MetricKind::SaveToPhoton,
            tree_size,
            samples
                .iter()
                .map(|ms| MetricSample::latency(LatencyMs::from_raw(*ms)))
                .collect(),
        );

        let p50 = record.p50().map_or(f64::NAN, |l| l.as_f64());
        let p99 = record.p99().map_or(f64::NAN, |l| l.as_f64());
        let pmean = record.mean().map_or(f64::NAN, |l| l.as_f64());
        println!(
            "LANE-H save→photon (loopback) tree_size={tree_size} samples={SAMPLES} \
             debounce_ms={} mean={:.3}ms p50={:.3}ms p99={:.3}ms",
            DEBOUNCE.as_millis(),
            pmean,
            p50,
            p99
        );
        let json = record.to_json().expect("serialize record");
        println!("LANE-H record json: {json}");

        let verdict = evaluate(&record, &Budgets::v1());
        assert!(
            verdict.passed,
            "save→photon p95 {:.3}ms exceeded §3.10 ceiling {:.3}ms \
             (loopback baseline, tree_size {tree_size}): {}",
            verdict.observed_p95, verdict.ceiling, verdict.reason
        );
    }
}
