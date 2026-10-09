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
pub(crate) const SAMPLE_ENTRY: &str = "\
// main.flux — Flux app entry point.
//
// `Hello` is the root component; `flux dev` serves it over WebSocket and
// `flux build` lowers it for iOS / Android.

compo Hello
    state count: Int = 0

    Column(gap: 8.0) {
        Text(text: \"tapped ${count} times\")
        Button(text: \"Increment\", onPress: fn() { count = count + 1 })
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
///
/// `{{PROJECT_NAME}}` is replaced with the (validated) user-supplied name.
/// A distinct placeholder is used so a template with the literal substring
/// `myapp` elsewhere would not be corrupted.
const SAMPLE_CONFIG: &str = "\
[project]
name = \"{{PROJECT_NAME}}\"
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
/// Maximum length of a project name.
const MAX_PROJECT_NAME_LEN: usize = 64;

/// Validates a project name.
///
/// The name becomes both a directory name (so it must be a single path
/// segment, no separators or `..`) and a TOML basic-string value (so it must
/// not contain a `"` or a newline). Restricting to `[A-Za-z][A-Za-z0-9_-]*`
/// covers every reasonable project identifier and keeps the generator free of
/// escaping logic — the previous implementation substituted the raw name into
/// `flux.toml`, which could inject TOML keys or break the string.
///
/// # Errors
///
/// Returns an error naming the offending character when the name is empty,
/// too long, starts with a non-letter, or contains anything but ASCII
/// alphanumerics, `-`, or `_`.
fn validate_project_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() {
        bail!("invalid project name: empty — hint: pass a name like `flux init blog-app`");
    }
    if name.len() > MAX_PROJECT_NAME_LEN {
        bail!(
            "invalid project name {name:?}: exceeds {MAX_PROJECT_NAME_LEN} characters",
        );
    }
    let mut chars = name.chars();
    let first = chars.next().expect("non-empty checked above");
    if !first.is_ascii_alphabetic() {
        bail!(
            "invalid project name {name:?}: must start with an ASCII letter \
             — hint: use `[A-Za-z][A-Za-z0-9_-]*`, e.g. `blog-app`"
        );
    }
    for c in chars {
        if !c.is_ascii_alphanumeric() && c != '-' && c != '_' {
            bail!(
                "invalid project name {name:?}: contains `{c}` — hint: use only \
                 ASCII letters, digits, `-`, and `_`"
            );
        }
    }
    Ok(())
}

pub(crate) fn run(name: &str) -> anyhow::Result<()> {
    validate_project_name(name)?;
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
    // Interpolate the (validated) user-supplied project name.
    let config = SAMPLE_CONFIG.replace("{{PROJECT_NAME}}", name);
    write_file(root, CONFIG_FILE, &config)?;

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
