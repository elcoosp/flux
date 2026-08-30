//! `flux fmt` — canonicalize one or more `.flux` files (FLUX-078).
//!
//! Parses each file through `flux-parser`, pretty-prints it back to canonical
//! source, and either writes it in place or (with `--check`) verifies it is
//! already canonical. The printer is a pure function of the parsed AST, so the
//! output is deterministic and round-trips through the parser unchanged.

use std::path::Path;
use std::path::PathBuf;

use anyhow::{Context, bail};

use flux_parser::format_source;

/// Formats every `*.flux` file in `paths` to canonical style.
///
/// When `check` is `true` the files are not written; instead the process exits
/// with an error if any file would change (for CI gating). With `check` false
/// each file is rewritten in place only when its contents differ.
///
/// # Errors
///
/// Returns an error when a path cannot be read, does not parse as Flux, or
/// (in `--check` mode) any file is not already canonical.
pub(crate) fn run(paths: &[PathBuf], check: bool) -> anyhow::Result<()> {
    let mut changed = Vec::new();
    for path in paths {
        format_one(path, check, &mut changed)
            .with_context(|| format!("formatting `{}`", path.display()))?;
    }

    if check && !changed.is_empty() {
        for path in &changed {
            tracing::error!("not canonical: {}", path.display());
        }
        bail!(
            "{} file(s) are not canonically formatted; run `flux fmt` to fix",
            changed.len()
        );
    }
    Ok(())
}

/// Formats a single file. On success, pushes `path` into `changed` when the file
/// was (or would be) modified.
fn format_one(path: &Path, check: bool, changed: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    let source =
        std::fs::read_to_string(path).with_context(|| format!("reading `{}`", path.display()))?;

    let formatted = format_source(&source, file_id_for(path), &path.display().to_string())
        .with_context(|| format!("parsing `{}`", path.display()))?;

    if formatted == source {
        return Ok(());
    }

    changed.push(path.to_path_buf());
    if check {
        return Ok(());
    }

    std::fs::write(path, &formatted).with_context(|| format!("writing `{}`", path.display()))?;
    tracing::info!("formatted {}", path.display());
    Ok(())
}

/// Derives a stable `file_id` from a path so node IDs are reproducible across runs
/// for the same file (mirrors the dev server's content-addressing intent without
/// needing the full pipeline).
fn file_id_for(path: &Path) -> u32 {
    use std::hash::Hasher;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hasher.write(path.to_string_lossy().as_bytes());
    (hasher.finish() & 0xFFFF_FFFF) as u32
}
