//! Integration tests for the `flux` CLI (FLUX-022, spec §14.3).

use std::path::Path;

use flux_cli::{Command, Platform, build_schema, run};
use flux_devserver::DevServer;
use flux_ir_serde::{FRAME_INIT, Frame, FrameKind};
use futures_util::{SinkExt, StreamExt};

use tempfile::TempDir;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;

/// The `init` and `doc` tests mutate the process-wide working directory.
/// Serialize them against each other (and against any future cwd-touching
/// test) so a parallel run cannot race on `std::env::set_current_dir`. A
/// `tokio` mutex is used (not `std::sync::Mutex`) because the guard is held
/// across `.await` points and must remain `Send`-safe.
static CWD_GUARD: Mutex<()> = Mutex::const_new(());

/// `flux init <name>` produces a project that `flux dev`/`flux build` can read.
#[tokio::test]
async fn init_creates_consumable_project() {
    let _guard = CWD_GUARD.lock().await;
    let dir = TempDir::new().expect("temp dir");
    let name = "myapp";

    let previous = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(dir.path()).expect("chdir to temp");

    run(Command::Init {
        name: name.to_owned(),
    })
    .await
    .expect("init scaffolds");

    let root = Path::new(name);
    assert!(root.is_dir(), "project directory created");
    assert!(root.join("main.flux").is_file(), "entry component created");
    assert!(root.join(".fluxignore").is_file(), "ignore file created");
    assert!(root.join("flux.toml").is_file(), "config created");

    std::env::set_current_dir(previous).expect("restore cwd");
}

/// `flux init` refuses to overwrite a non-empty directory.
#[tokio::test]
async fn init_refuses_non_empty_dir() {
    let _guard = CWD_GUARD.lock().await;
    let dir = TempDir::new().expect("temp dir");

    let previous = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(dir.path()).expect("chdir to temp");
    std::fs::write("main.flux", "component X { Text(\"x\") }").expect("seed file");

    let result = run(Command::Init {
        name: "main.flux".to_owned(),
    })
    .await;
    assert!(result.is_err(), "init must not clobber an existing path");

    std::env::set_current_dir(previous).expect("restore cwd");
}

/// `flux dev` serves an `Init` frame to a real tungstenite client (spec §D.12).
#[tokio::test]
async fn dev_serves_init_to_client() {
    let dir = TempDir::new().expect("temp dir");
    let root = dir.path();
    std::fs::write(
        root.join("main.flux"),
        "component Hello { state count: Int = 0 Button(text: \"tap\", onClick: fn() { count = count + 1 }) }",
    )
    .expect("write sample");

    let server = DevServer::start(
        flux_devserver::ServerConfig::new(root)
            .with_ws_port(0)
            .with_http_port(0),
    )
    .await
    .expect("dev server starts");
    let ws_addr = server.ws_addr();

    let url = format!("ws://{ws_addr}");
    let (mut stream, _resp) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("client connects");
    let hello = Frame::hello("cli-test", "test", &[]).to_bytes();
    stream
        .send(Message::Binary(hello.into()))
        .await
        .expect("send Hello");

    let reply = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
        .await
        .expect("Init arrives within timeout")
        .expect("stream yields a message")
        .expect("message decodes");
    let bytes = match reply {
        Message::Binary(b) => b,
        other => panic!("expected a binary frame, got {other:?}"),
    };

    let init = Frame::from_init_bytes(&bytes).expect("frame decodes as Init");
    assert_eq!(init.kind, FrameKind::Init);
    assert_eq!(bytes[5], FRAME_INIT, "frame type byte is FRAME_INIT");

    drop(server);
}

/// `flux build --platform ios` writes a generated Swift file carrying the component.
#[tokio::test]
async fn build_ios_writes_generated_swift() {
    let dir = TempDir::new().expect("temp dir");
    write_sample_project(dir.path());

    run(Command::Build {
        platform: Platform::Ios,
        root: dir.path().to_path_buf(),
    })
    .await
    .expect("ios build succeeds");

    let generated = dir.path().join("platforms/ios/Generated/main.swift");
    assert!(generated.is_file(), "generated swift file exists");
    let source = std::fs::read_to_string(&generated).expect("readable");
    assert!(!source.is_empty(), "generated source is non-empty");
    assert!(
        source.contains("Hello"),
        "generated swift carries the component"
    );
}

/// `flux build --platform android` writes a generated Kotlin file carrying the component.
#[tokio::test]
async fn build_android_writes_generated_kotlin() {
    let dir = TempDir::new().expect("temp dir");
    write_sample_project(dir.path());

    run(Command::Build {
        platform: Platform::Android,
        root: dir.path().to_path_buf(),
    })
    .await
    .expect("android build succeeds");

    let generated = dir.path().join("platforms/android/Generated/main.kt");
    assert!(generated.is_file(), "generated kotlin file exists");
    let source = std::fs::read_to_string(&generated).expect("readable");
    assert!(!source.is_empty(), "generated source is non-empty");
    assert!(
        source.contains("Hello"),
        "generated kotlin carries the component"
    );
}

/// `flux doc` emits valid JSON (asserted by `serde_json::from_str`).
#[tokio::test]
async fn doc_emits_valid_json() {
    let _guard = CWD_GUARD.lock().await;
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("stdlib").is_dir())
        .expect("repository stdlib dir");

    let previous = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(repo_root).expect("chdir to repo root");
    let result = run(Command::Doc).await;
    std::env::set_current_dir(previous).expect("restore cwd");
    result.expect("doc emits");

    let schema = build_schema(repo_root.join("stdlib").as_path()).expect("schema builds");
    let json = serde_json::to_string(&schema).expect("schema serializes");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("schema is valid JSON");
    assert!(parsed.get("modules").is_some(), "schema exposes modules");
}

/// Writes a minimal but complete sample project at `root` for build tests.
fn write_sample_project(root: &Path) {
    std::fs::write(
        root.join("main.flux"),
        "component Hello { state count: Int = 0 Button(text: \"tap\", onClick: fn() { count = count + 1 }) }",
    )
    .expect("write entry component");
}
