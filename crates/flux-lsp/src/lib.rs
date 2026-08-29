//! `flux-lsp` — the Flux language server (FLUX-024, PRD-O user stories 1–3).
//!
//! This crate is the canonical home for every LSP feature (diagnostics,
//! go-to-definition, hover, completion — FLUX-025..029). It is built on
//! [`async_lsp`] over a tokio runtime and **reuses the compiler** (`flux-parser`
//! / `flux-types` / `flux-ir`): it never re-implements analysis, so the LSP and
//! the CLI/DevTools never disagree on a diagnostic (PRD-S's rustc-grade shape).
//!
//! The crate is intentionally a thin, compiling scaffold: it wires the
//! `async-lsp` server loop and lifts the `LspDiagnostic` JSON shape that the
//! existing `flux-cli` `mod lsp` emitter already produces, so the VS Code
//! extension (FLUX-026) and any non-LSP consumer keep working against the same
//! contract. Diagnostics-as-you-type (FLUX-024/025) and the remaining providers
//! are filled in by their own issues.

#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]

use std::ops::ControlFlow;
use std::path::PathBuf;

use async_lsp::lsp_types;
use async_lsp::{
    LanguageServer, MainLoop,
    lsp_types::{
        Diagnostic, DiagnosticSeverity, DidOpenTextDocumentParams, InitializeParams,
        InitializeResult, InitializedParams, Range, SemanticTokensParams, SemanticTokensResult,
        ServerCapabilities, ServerInfo, TextDocumentItem, TextDocumentSyncCapability,
        TextDocumentSyncKind, Url,
    },
    router::Router,
    stdio::{PipeStdin, PipeStdout},
};

mod semantic_tokens;

/// One LSP-shaped diagnostic, reusing the shape `flux-cli`'s `mod lsp` emits
/// (`line`/`character`/`length`/`severity`/`message`/`source`) so the CLI JSON
/// and the LSP `Diagnostic` never diverge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LspDiagnostic {
    /// 1-based line of the diagnostic's start.
    pub line: u32,
    /// 1-based character (column) of the diagnostic's start.
    pub character: u32,
    /// Length in characters of the underlined span.
    pub length: u32,
    /// Severity: always `1` (Error) for compiler diagnostics.
    pub severity: u8,
    /// The human-readable message.
    pub message: String,
    /// Which compiler phase produced the diagnostic (`parse` / `type`).
    pub source: String,
}

/// State for the Flux language server: an in-memory document cache keyed by URI.
///
/// Per PRD-O the server is per-open-document; cross-file project analysis is a
/// later concern (FLUX-029 extends this with incremental `didChange`).
#[derive(Debug, Default)]
pub struct FluxLsp {
    /// Open documents keyed by their `file://` URI string.
    documents: std::sync::Mutex<std::collections::HashMap<Url, String>>,
}

impl FluxLsp {
    /// Creates a new, empty server state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Computes the compiler diagnostics for `text`, mapping `flux-parser`
    /// parse errors into [`LspDiagnostic`]s. Type-checking (FLUX-025) extends
    /// this to also run `flux-types`.
    ///
    /// Parse errors themselves are returned in the `Vec`, never as an `Err`.
    #[must_use]
    pub fn diagnostics_for_text(&self, path: &std::path::Path, text: &str) -> Vec<LspDiagnostic> {
        let path_str = path.to_string_lossy().into_owned();
        let mut out = Vec::new();
        match flux_parser::parse(text, 0, &path_str) {
            Ok(_) => {}
            Err(err) => out.push(LspDiagnostic {
                line: err.location.line,
                character: err.location.column,
                length: err.span.len().max(1),
                severity: 1,
                message: err.message.clone(),
                source: "parse".to_owned(),
            }),
        }
        out
    }

    /// Maps a [`LspDiagnostic`] (1-based `line`/`character`) into an LSP
    /// [`Diagnostic`] with a [`Range`] (0-based, per the LSP spec).
    #[must_use]
    pub fn to_lsp_diagnostic(d: &LspDiagnostic) -> Diagnostic {
        let start = lsp_types::Position {
            line: d.line.saturating_sub(1),
            character: d.character.saturating_sub(1),
        };
        let end = lsp_types::Position {
            line: d.line.saturating_sub(1),
            character: d.character.saturating_sub(1) + d.length,
        };
        Diagnostic {
            range: Range { start, end },
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some(d.source.clone()),
            message: d.message.clone(),
            ..Default::default()
        }
    }
}

impl LanguageServer for FluxLsp {
    type Error = async_lsp::ResponseError;
    type NotifyResult = ControlFlow<async_lsp::Result<()>>;

    fn initialize(
        &mut self,
        _: InitializeParams,
    ) -> futures::future::BoxFuture<'static, Result<InitializeResult, async_lsp::ResponseError>>
    {
        Box::pin(async {
            Ok(InitializeResult {
                capabilities: ServerCapabilities {
                    text_document_sync: Some(TextDocumentSyncCapability::Kind(
                        TextDocumentSyncKind::FULL,
                    )),
                    semantic_tokens_provider: Some(
                        async_lsp::lsp_types::SemanticTokensServerCapabilities::SemanticTokensOptions(
                            async_lsp::lsp_types::SemanticTokensOptions {
                                legend: semantic_tokens::legend(),
                                full: Some(async_lsp::lsp_types::SemanticTokensFullOptions::Bool(true)),
                                range: None,
                                ..Default::default()
                            },
                        ),
                    ),
                    ..Default::default()
                },
                server_info: Some(ServerInfo {
                    name: "flux-lsp".to_owned(),
                    version: Some(env!("CARGO_PKG_VERSION").to_owned()),
                }),
            })
        })
    }

    fn shutdown(
        &mut self,
        _: (),
    ) -> futures::future::BoxFuture<'static, Result<(), async_lsp::ResponseError>> {
        Box::pin(async { Ok(()) })
    }

    fn initialized(&mut self, _: InitializedParams) -> Self::NotifyResult {
        tracing::info!("flux-lsp initialized");
        ControlFlow::Continue(())
    }

    fn did_open(&mut self, params: DidOpenTextDocumentParams) -> Self::NotifyResult {
        let DidOpenTextDocumentParams {
            text_document: TextDocumentItem { uri, text, .. },
        } = params;
        self.documents
            .lock()
            .expect("documents mutex poisoned")
            .insert(uri, text);
        ControlFlow::Continue(())
    }

    fn semantic_tokens_full(
        &mut self,
        params: SemanticTokensParams,
    ) -> futures::future::BoxFuture<
        'static,
        Result<Option<SemanticTokensResult>, async_lsp::ResponseError>,
    > {
        // Read the document text out of the cache first so the returned future
        // owns everything it needs and is `'static` (the trait requires
        // `BoxFuture<'static>`). The highlight pass itself is CPU-bound and runs
        // inside the future.
        let uri = params.text_document.uri;
        let text = self
            .documents
            .lock()
            .expect("documents mutex poisoned")
            .get(&uri)
            .cloned()
            .unwrap_or_default();
        Box::pin(async move {
            let data = semantic_tokens::tokens_for_text(&text);
            Ok(Some(async_lsp::lsp_types::SemanticTokensResult::Tokens(
                async_lsp::lsp_types::SemanticTokens {
                    result_id: None,
                    data,
                },
            )))
        })
    }
}

/// Runs the `flux-lsp` server over a tokio stdio transport (stdin/stdout).
///
/// Uses `async-lsp`'s `tokio` stdio helpers so reads/writes never spawn
/// blocking threads (per the crate's documented caveats).
///
/// # Errors
///
/// Propagates any error from the `async-lsp` main loop (transport EOF, malformed
/// JSON-RPC, …).
pub async fn run_stdio() -> async_lsp::Result<()> {
    let router = Router::from_language_server(FluxLsp::new());
    let (main_loop, _client_socket) = MainLoop::new_server(|_client| router);
    let stdin = PipeStdin::lock_tokio()?;
    let stdout = PipeStdout::lock_tokio()?;
    main_loop.run_buffered(stdin, stdout).await
}

/// Re-export so callers (and `flux-cli`) can build a path from a URI without
/// depending on `lsp_types` directly.
#[must_use]
pub fn uri_to_path(uri: &Url) -> Option<PathBuf> {
    uri.to_file_path().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;

    fn fixture(content: &str) -> std::path::PathBuf {
        let mut f = tempfile::NamedTempFile::new().expect("temp file");
        f.write_all(content.as_bytes()).expect("write");
        f.into_temp_path().keep().expect("keep")
    }

    #[test]
    fn diagnostics_reports_parse_error_with_lsp_position() {
        // A malformed component body (mirrors flux-cli's existing test).
        let path = fixture("compo Broken\n  if true {\n");
        let text = std::fs::read_to_string(&path).expect("read");
        let diags = FluxLsp::new().diagnostics_for_text(&path, &text);
        assert_eq!(diags.len(), 1);
        let d = &diags[0];
        assert_eq!(d.line, 3);
        assert_eq!(d.character, 1);
        assert_eq!(d.severity, 1);
        assert_eq!(d.source, "parse");
        assert!(!d.message.is_empty());

        let lsp = FluxLsp::to_lsp_diagnostic(d);
        assert_eq!(lsp.range.start.line, 2);
        assert_eq!(lsp.range.start.character, 0);
    }

    #[test]
    fn diagnostics_is_empty_for_well_formed_source() {
        let path = fixture("compo Ok\n  Text(text: \"hi\")\n");
        let text = std::fs::read_to_string(&path).expect("read");
        let diags = FluxLsp::new().diagnostics_for_text(&path, &text);
        assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
    }
}
