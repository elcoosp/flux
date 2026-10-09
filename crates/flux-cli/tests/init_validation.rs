//! F8: `flux init <name>` must reject names that are not safe as directory
//! names or TOML basic-string values, and must write the *exact* name into
//! `flux.toml`'s `[project] name` field.

use std::path::Path;
use std::process::Command;

/// Runs `flux init <name>` in a fresh tempdir and returns (exit_ok, root_path).
fn try_init(name: &str) -> (bool, std::path::PathBuf) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = std::env::temp_dir().join(format!("flux-init-test-{nanos}"));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let bin = env!("CARGO_BIN_EXE_flux");
    let output = Command::new(bin)
        .args(["init", name])
        .current_dir(&dir)
        .output()
        .expect("spawn flux init");
    (output.status.success(), dir.join(name))
}

/// Rejects names that would break the directory layout or the TOML string.
#[test]
fn init_rejects_unsafe_names() {
    for bad in ["a\"b", "a\nb", "../x", "/abs", "a/b", "a\\b", "", ".", ".."] {
        let (ok, path) = try_init(bad);
        assert!(!ok, "`flux init {bad:?}` must fail; scaffolded {path:?}",);
    }
}

/// Accepts conservative names and writes them verbatim to `flux.toml`.
#[test]
fn init_writes_project_name_to_toml() {
    let (ok, root) = try_init("blog-app");
    assert!(ok, "`flux init blog-app` must succeed");
    let toml = std::fs::read_to_string(root.join("flux.toml")).expect("flux.toml exists");
    assert!(
        toml.contains("name = \"blog-app\""),
        "flux.toml must carry the exact project name; got:\n{toml}",
    );

    // Underscore-names are also acceptable.
    let (ok, root) = try_init("blog_app_2");
    assert!(ok, "`flux init blog_app_2` must succeed");
    let toml = std::fs::read_to_string(root.join("flux.toml")).expect("flux.toml exists");
    assert!(toml.contains("name = \"blog_app_2\""));

    // Reject a leading digit (would be a valid TOML string but a poor
    // directory name in shell contexts).
    let (ok, _) = try_init("2app");
    assert!(!ok, "`flux init 2app` must fail (must start with a letter)");
}

/// Sanity: `flux init <name>` creates a directory named `<name>`.
#[test]
fn init_creates_directory_named_after_project() {
    let (ok, root) = try_init("simple-app");
    assert!(ok);
    assert!(root.is_dir(), "the directory must exist at {root:?}");
    assert!(
        Path::new(&root).join("main.flux").is_file(),
        "main.flux scaffolded",
    );
    assert!(Path::new(&root).join("flux.toml").is_file());
}
