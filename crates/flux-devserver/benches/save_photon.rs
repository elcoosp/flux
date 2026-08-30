//! Criterion bench: LANE-H save→photon end-to-end (FLUX-073).
//!
//! This is the CI-telemetry artifact for the headline "Save → pixels" budget. It
//! drives the *real* dev server plus a headless loopback WebSocket client (no
//! device) and measures `notify` event → host applying the final `Delta` frame,
//! reporting **p50 / p99** for two tree sizes (50-node and ~1k-node) per the
//! §3.10 scale points.
//!
//! It is deliberately a plain `harness = false` binary rather than a
//! `criterion::bench_function` timing loop: the e2e path is dominated by the
//! file-watcher debounce and the WebSocket round-trip, so the meaningful signal
//! is the *distribution* over N (≥ 50) independent saves, not criterion's own
//! inner-loop timing. We collect the raw samples and print the percentiles plus
//! a JSON [`MetricRecord`] (consumable by DevTools / a dashboard later).
//!
//! This is the **loopback** baseline: it excludes WiFi RTT, on-device decode,
//! signal re-eval, view mutation, layout and raster. Until a physical-device /
//! simulator runner exists, treat these numbers as the honest baseline to
//! tighten against (the per-stage §3.10 micro-budgets remain the localization
//! signal for *where* a regression lives).
//!
//! Harness pattern mirrors `tests/full_pipeline.rs` / `tests/auth_token.rs`: the
//! blocking `tungstenite` connect/read run inside `spawn_blocking` so they never
//! stall the server's accept task on the async reactor (brittleness 6).

use std::fs;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flux_devserver::{DevServer, ServerConfig};
use flux_ir_serde::FRAME_INIT;
use flux_perf_harness::{
    Budgets, LatencyMs, MetricKind, MetricRecord, MetricSample, Scenario, evaluate,
};
use tokio_tungstenite::tungstenite::stream::MaybeTlsStream;
use tokio_tungstenite::tungstenite::{Message, WebSocket, connect};

/// File-watch debounce window used by the harness (production default is 50 ms;
/// a tightened 10 ms keeps the loopback number representative of a
/// watcher-optimized server).
const DEBOUNCE: Duration = Duration::from_millis(10);
/// Samples per tree size (≥ 50 for a stable tail, per FLUX-073).
const SAMPLES: usize = 60;
/// `(tree_size, leaf_count)` scale points. `tree_size = leaf_count + 3`.
const TREE_SIZES: &[(u64, u32)] = &[(50, 47), (1000, 997)];

fn scratch_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = std::env::temp_dir().join(format!("flux-s2p-bench-{tag}-{nanos}"));
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Generates a synthetic `leaf_count`-leaf app with a `Button` whose `onPress`
/// handler body varies by `edit_tag` (the only edit the differencer detects as
/// a `Delta` over a stable-size tree — see `tests/save_to_photon.rs`).
fn synthetic_app(leaf_count: u32, edit_tag: Option<u32>) -> String {
    let inc = match edit_tag {
        Some(k) => 1 + (k % 49),
        None => 0,
    };
    let mut src =
        String::from("compo Counter\n    state count: Int = 0\n\n    Column(gap: 8.0) {\n");
    for i in 0..leaf_count {
        src.push_str(&format!("        Text(text: \"label-{i}\")\n"));
    }
    src.push_str(&format!(
        "        Button(text: \"tap\", onPress: fn() {{ count = count + {inc} }})\n"
    ));
    src.push_str("    }\n\n");
    src
}

type Client = WebSocket<MaybeTlsStream<std::net::TcpStream>>;

fn connect_client(addr: SocketAddr) -> Client {
    let (socket, _response) = connect(format!("ws://{addr}/")).expect("ws connect");
    socket
}

fn handshake(client: &mut Client) -> Vec<u8> {
    let hello = flux_ir_serde::Frame::hello("ios", "save-to-photon-bench", &[]).to_bytes();
    client
        .send(Message::Binary(hello.into()))
        .expect("send hello");
    next_frame(client, Duration::from_secs(5)).expect("init frame")
}

fn frame_type(bytes: &[u8]) -> Option<u8> {
    bytes.get(5).copied()
}

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

fn measure_one(client: &mut Client, file: &Path, source: &str) -> f64 {
    let start = Instant::now();
    fs::write(file, source).expect("save edit");
    let frame = next_frame(client, Duration::from_secs(5)).expect("delta frame arrives");
    assert_eq!(frame_type(&frame), Some(flux_ir_serde::FRAME_DELTA));
    start.elapsed().as_secs_f64() * 1_000.0
}

async fn run_one_tree(tree_size: u64, leaves: u32) -> MetricRecord {
    let root = scratch_dir(&format!("tree-{tree_size}"));
    let file = root.join("main.flux");
    fs::write(&file, synthetic_app(leaves, None)).expect("write baseline source");

    let config = ServerConfig::new(&root)
        .with_ws_addr("127.0.0.1:0".parse::<SocketAddr>().expect("addr"))
        .with_http_addr("127.0.0.1:0".parse::<SocketAddr>().expect("addr"))
        .with_debounce(DEBOUNCE);
    let server = DevServer::start(config).await.expect("server starts");
    let addr = server.ws_addr();

    let samples = tokio::task::spawn_blocking(move || {
        let mut client = connect_client(addr);
        assert_eq!(
            frame_type(&handshake(&mut client)),
            Some(FRAME_INIT),
            "bench must receive Init on handshake"
        );
        std::thread::sleep(Duration::from_millis(150));
        let mut samples = Vec::with_capacity(SAMPLES);
        for k in 0..SAMPLES {
            let src = synthetic_app(leaves, Some(k as u32));
            samples.push(measure_one(&mut client, &file, &src));
            std::thread::sleep(Duration::from_millis(20));
        }
        server.shutdown();
        samples
    })
    .await
    .expect("bench task");

    MetricRecord::new(
        Scenario::LoopbackE2e,
        MetricKind::SaveToPhoton,
        tree_size,
        samples
            .iter()
            .map(|ms| MetricSample::latency(LatencyMs::from_raw(*ms)))
            .collect(),
    )
}

fn main() {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    runtime.block_on(async {
        for &(tree_size, leaves) in TREE_SIZES {
            let record = run_one_tree(tree_size, leaves).await;
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
            println!(
                "LANE-H record json: {}",
                record.to_json().expect("serialize")
            );

            let verdict = evaluate(&record, &Budgets::v1());
            println!(
                "LANE-H gate: passed={} observed_p95={:.3}ms ceiling={:.3}ms",
                verdict.passed, verdict.observed_p95, verdict.ceiling
            );
        }
    });
}
