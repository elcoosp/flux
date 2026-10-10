//! FLUX-F17: `flux build` emits a `<stem>.flux.map.json` sidecar alongside
//! each generated native source, so a release-mode crash can be resolved
//! back to its originating `.flux` component.

use std::process::Command;

fn flux_bin() -> &'static str {
    env!("CARGO_BIN_EXE_flux")
}

#[test]
fn build_writes_source_map_sidecar() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(&project).expect("mkdir");
    // Minimal project: one component, one entry.
    std::fs::write(
        project.join("main.flux"),
        "compo Hello\n  state n: Int = 0\n  Text(text: \"hi\")\n",
    )
    .expect("write flux");
    std::fs::write(
        project.join("flux.toml"),
        "[project]\nname = \"sidecar-test\"\nentry = \"main.flux\"\n",
    )
    .expect("write toml");

    // `flux build <PLATFORM> --root <PATH>`: platform is positional; root is
    // passed through `--root` (the default `.` would be the tempdir, not the
    // project).
    let out = Command::new(flux_bin())
        .current_dir(tmp.path())
        .arg("build")
        .arg("ios")
        .arg("--root")
        .arg(&project)
        .output()
        .expect("run flux build");
    assert!(
        out.status.success(),
        "flux build failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // The sidecar may land under any generated `main.flux.map.json` under
    // the project. Find it recursively.
    let found = find_file(&project, "main.flux.map.json")
        .or_else(|| find_file(&project, "HelloApp.flux.map.json"))
        .or_else(|| find_file(&project, "MainActivity.flux.map.json"));
    let sidecar = found.expect("source map sidecar must be written somewhere under the project");

    let json = std::fs::read_to_string(&sidecar).expect("read sidecar");
    assert!(json.contains("\"native_symbol\": \"Hello\""), "expected Hello symbol: {json}");
    assert!(json.contains("\"line\":"), "expected line field: {json}");
}

fn find_file(root: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    walk(root)
        .into_iter()
        .find(|e| e.file_name().and_then(|s| s.to_str()) == Some(name))
}

fn walk(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(root) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                out.extend(walk(&p));
            } else {
                out.push(p);
            }
        }
    }
    out
}
