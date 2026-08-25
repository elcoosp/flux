//! Source gathering shared by `flux build` and `flux doc` (FLUX-022).
//!
//! Recursively collects every `.flux` file under a root so the `FileId`
//! assignment downstream is deterministic (sorted by path).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

/// Recursively collects every `.flux` source under `root`, sorted by path.
///
/// # Errors
///
/// Returns an error when `root` cannot be read, or when it contains no `.flux`
/// sources at all (a project with nothing to compile is a user mistake).
pub(crate) fn gather(root: &Path) -> anyhow::Result<Vec<(PathBuf, String)>> {
    let mut out = Vec::new();
    collect_into(root, &mut out)
        .with_context(|| format!("reading .flux sources from {}", root.display()))?;
    out.sort_by(|a, b| a.0.cmp(&b.0));
    if out.is_empty() {
        bail!(
            "no .flux sources found under {} — hint: add a .flux entry component",
            root.display()
        );
    }
    Ok(out)
}

/// Walks `dir`, appending every `.flux` file's path and contents to `out`.
fn collect_into(dir: &Path, out: &mut Vec<(PathBuf, String)>) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_into(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("flux") {
            match fs::read_to_string(&path) {
                Ok(source) => out.push((path, source)),
                Err(error) => tracing::warn!(
                    path = %path.display(),
                    %error,
                    "cannot read source file; skipping"
                ),
            }
        }
    }
    Ok(())
}
