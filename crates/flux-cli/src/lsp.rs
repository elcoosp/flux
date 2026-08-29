//! `flux lsp <file>` — one-shot compiler diagnostics (FLUX-025).
//!
//! Emits a JSON array of [`LspDiagnostic`]s for a `.flux` file, reusing the
//! exact same shape the `flux-lsp` language server publishes so the VS Code
//! extension (FLUX-026) and any non-LSP consumer never disagree with the
//! editor. The diagnostics run the full `parse → type_check` pipeline
//! (`diagnostics_with_types`); the `--types` flag (default on) selects whether
//! the type-checker runs, keeping parse-only as the fast path for huge files.

use std::fs;

use anyhow::{Context, anyhow};

use flux_lsp::FluxLsp;
use flux_lsp::LspDiagnostic;

/// Collects parse + type diagnostics for `file` into a `Vec<LspDiagnostic>`.
///
/// Reads `file`, computes diagnostics via [`FluxLsp::diagnostics_with_types`],
/// and returns them. When `types` is `false` only parse diagnostics are emitted.
///
/// # Errors
///
/// Returns an error when `file` cannot be read, or when `file` is not a `.flux`
/// source (type-checking requires the compiler front-end, which only accepts
/// Flux). A source full of type errors still yields its `Vec` and the caller
/// prints `[]`-style JSON and exits `0`.
pub fn collect_lsp(file: &std::path::Path, types: bool) -> anyhow::Result<Vec<LspDiagnostic>> {
    if file.extension().and_then(|e| e.to_str()) != Some("flux") {
        return Err(anyhow!(
            "expected a `.flux` source file, got `{}` — hint: pass the path to a Flux component/screen",
            file.display()
        ));
    }

    let text = fs::read_to_string(file)
        .with_context(|| format!("reading source file {}", file.display()))?;

    Ok(FluxLsp::new().diagnostics_with_types(file, &text, types))
}

/// Runs `flux lsp <file>`.
///
/// Collects diagnostics via [`collect_lsp`] and prints them as a JSON array to
/// stdout.
///
/// # Errors
///
/// Propagates any error from [`collect_lsp`] (unreadable file, non-`.flux` input).
pub(crate) fn run(file: &std::path::Path, types: bool) -> anyhow::Result<()> {
    let diags = collect_lsp(file, types)?;
    let json = serde_json::to_string_pretty(&diags).context("serializing diagnostics to JSON")?;
    println!("{json}");
    Ok(())
}
