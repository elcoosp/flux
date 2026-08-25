//! `flux init` — scaffold a new Flux project (FLUX-022, spec §14.3).
//!
//! Creates `<name>/` containing a sample entry component, a `.fluxignore` and a
//! `flux.toml` config, producing a directory that `flux dev` and `flux build`
//! can consume directly.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

/// Name of the sample entry component written into a new project.
const ENTRY_FILE: &str = "main.flux";
/// Name of the ignore file written into a new project.
const IGNORE_FILE: &str = ".fluxignore";
/// Name of the project config written into a new project.
const CONFIG_FILE: &str = "flux.toml";

/// A sample entry component that exercises the prelude (no type-checker
/// dependency on the stdlib source): a `Column` of a `Text` and a `Button`.
const SAMPLE_ENTRY: &str = "\
// main.flux — Flux app entry point.
//
// `Hello` is the root component; `flux dev` serves it over WebSocket and
// `flux build` lowers it for iOS / Android.

component Hello {
    state count: Int = 0

    Column(gap: 8.0) {
        Text(text: \"tapped ${count} times\")
        Button(text: \"Increment\", onClick: fn() { count = count + 1 })
    }
}
";

/// Default `.fluxignore` — build output, VCS noise, and editor cruft.
const SAMPLE_IGNORE: &str = "\
# Flux build output
platforms/

# VCS / editor noise
.git/
.gitignore
*.swp
.DS_Store
";

/// Minimal `flux.toml` project config.
const SAMPLE_CONFIG: &str = "\
[project]
name = \"myapp\"
entry = \"main.flux\"

[dev]
ws_port = 7331
http_port = 7332
";

/// Scaffolds a new project named `name`.
///
/// # Errors
///
/// Returns an error when `<name>` already exists and is not an empty directory,
/// or when any scaffold file cannot be written.
///
/// # Examples
///
/// ```ignore
/// flux_cli::run(flux_cli::Command::Init { name: "myapp".into() }).await?;
/// ```
pub(crate) fn run(name: &str) -> anyhow::Result<()> {
    let root = Path::new(name);
    if root.exists() && !is_empty_dir(root) {
        bail!(
            "refusing to scaffold into {name}: path already exists and is not empty — \
             hint: choose a new project name or remove the existing directory"
        );
    }

    fs::create_dir_all(root).with_context(|| format!("creating project directory {name}"))?;
    write_file(root, ENTRY_FILE, SAMPLE_ENTRY)?;
    write_file(root, IGNORE_FILE, SAMPLE_IGNORE)?;
    write_file(root, CONFIG_FILE, SAMPLE_CONFIG)?;

    tracing::info!(project = name, "scaffolded new Flux project");
    println!("created flux project `{name}`");
    Ok(())
}

/// Returns `true` when `path` exists and contains no entries.
fn is_empty_dir(path: &Path) -> bool {
    match fs::read_dir(path) {
        Ok(mut entries) => entries.next().is_none(),
        Err(_) => false,
    }
}

/// Writes `contents` to `root/file_name`, creating parents as needed.
fn write_file(root: &Path, file_name: &str, contents: &str) -> anyhow::Result<PathBuf> {
    let path = root.join(file_name);
    fs::write(&path, contents)
        .with_context(|| format!("writing scaffold file {}", path.display()))?;
    Ok(path)
}
