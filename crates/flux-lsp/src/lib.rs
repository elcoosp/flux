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
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use async_lsp::lsp_types;
use async_lsp::{
    ClientSocket, LanguageServer, MainLoop,
    lsp_types::{
        CompletionOptions, CompletionParams, CompletionResponse, Diagnostic, DiagnosticSeverity,
        DidChangeTextDocumentParams, DidOpenTextDocumentParams, GotoDefinitionParams,
        GotoDefinitionResponse, Hover, HoverParams, InitializeParams, InitializeResult,
        InitializedParams, PublishDiagnosticsParams, SemanticTokensParams, SemanticTokensResult,
        ServerCapabilities, ServerInfo, TextDocumentItem, TextDocumentSyncCapability,
        TextDocumentSyncKind, Url,
    },
    router::Router,
    stdio::{PipeStdin, PipeStdout},
};

mod completion;
mod goto_def;
mod hover;
mod semantic_tokens;
mod util;

/// One LSP-shaped diagnostic, reusing the shape `flux-cli`'s `mod lsp` emits
/// (`line`/`character`/`length`/`severity`/`message`/`source`) so the CLI JSON
/// and the LSP `Diagnostic` never diverge.
///
/// `Serialize`/`Deserialize` are derived so the `flux lsp` CLI subcommand emits
/// the exact same JSON the language server would, keeping the contract stable
/// for the VS Code extension (FLUX-026) and any non-LSP consumer.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
/// later concern (FLUX-029 extends this with incremental `didChange`). The
/// optional `client` socket is used to publish `window/publishDiagnostics` after
/// an incremental edit is debounced (see [`Self::did_change`]).
#[derive(Debug, Default)]
pub struct FluxLsp {
    /// Open documents keyed by their `file://` URI string.
    documents: std::sync::Mutex<std::collections::HashMap<Url, String>>,
    /// The editor client socket, if the server was started with one (stdio
    /// transport via [`run_stdio`]). `None` in unit tests that construct the
    /// server directly — publishing becomes a no-op.
    client: Option<ClientSocket>,
    /// Monotonic per-URI version counter used to cancel stale debounced
    /// re-analyses: a newer `didChange` for the same URI supersedes an older
    /// pending one.
    versions: std::sync::Mutex<std::collections::HashMap<Url, Arc<AtomicU32>>>,
}

impl FluxLsp {
    /// Creates a new, empty server state (no editor client — publishing is a
    /// no-op). Used by unit tests and non-LSP consumers.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a server bound to an editor `client` socket so it can push
    /// `window/publishDiagnostics` after debounced incremental re-analysis.
    #[must_use]
    pub fn with_client(client: ClientSocket) -> Self {
        Self {
            client: Some(client),
            ..Self::default()
        }
    }

    /// Debounced re-analysis delay after an incremental edit (FLUX-029).
    ///
    /// 50 ms matches the dev-server's watch-debounce (`ServerConfig`) so the
    /// editor and the hot-reload loop re-compile on the same cadence.
    const DEBOUNCE_MS: u64 = 50;

    /// Updates the in-memory document cache for `uri` and schedules a debounced
    /// re-analysis that publishes `window/publishDiagnostics` to the editor.
    ///
    /// Applied incrementally (the LSP client sends `TextDocumentContentChangeEvent`s
    /// with `range`), so this is the FLUX-029 incremental path: only the cached
    /// text is mutated, and the heavy `parse -> type_check` runs once per burst
    /// of keystrokes, not per keystroke.
    fn did_change(
        &mut self,
        params: DidChangeTextDocumentParams,
    ) -> ControlFlow<async_lsp::Result<()>> {
        let uri = params.text_document.uri.clone();
        let version = params.text_document.version;
        // Fold the incremental content changes into the cached document.
        let mut docs = self.documents.lock().expect("documents mutex poisoned");
        let text = docs.entry(uri.clone()).or_default();
        for change in params.content_changes {
            match change.range {
                // Incremental: apply the edit at the given range.
                Some(range) => {
                    if let Some(updated) = crate::util::apply_range_edit(text, range, &change.text)
                    {
                        *text = updated;
                    } else {
                        // Fall back to full replace if the range can't be mapped
                        // (defensive — should not happen with a conformant client).
                        *text = change.text;
                    }
                }
                // Full document sync: the client sent the entire new text.
                None => *text = change.text,
            }
        }
        drop(docs);

        // Record this version and schedule a debounced publish tied to it.
        let mut versions = self.versions.lock().expect("versions mutex poisoned");
        let counter = versions
            .entry(uri.clone())
            .or_insert_with(|| Arc::new(AtomicU32::new(0)));
        let my_version = counter.fetch_add(1, Ordering::SeqCst) + 1;
        let counter = Arc::clone(counter);
        drop(versions);

        if let Some(client) = self.client.clone() {
            let uri_for_task = uri.clone();
            let path = std::path::PathBuf::from(uri.path());
            let text = self
                .documents
                .lock()
                .expect("documents mutex poisoned")
                .get(&uri)
                .cloned()
                .unwrap_or_default();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(FluxLsp::DEBOUNCE_MS)).await;
                // A newer edit superseded this one — skip the stale publish.
                if counter.load(Ordering::SeqCst) != my_version {
                    return;
                }
                let server = FluxLsp::new();
                let diags = server.diagnostics_with_types(&path, &text, true);
                let lsp_diags: Vec<Diagnostic> =
                    diags.iter().map(FluxLsp::to_lsp_diagnostic).collect();
                let _ = client.notify::<lsp_types::notification::PublishDiagnostics>(
                    PublishDiagnosticsParams {
                        uri: uri_for_task,
                        diagnostics: lsp_diags,
                        version: Some(version),
                    },
                );
            });
        }
        ControlFlow::Continue(())
    }
}

impl FluxLsp {
    /// Computes parse-only compiler diagnostics for `text`, mapping
    /// `flux-parser` parse errors into [`LspDiagnostic`]s.
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
            range: lsp_types::Range { start, end },
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some(d.source.clone()),
            message: d.message.clone(),
            ..Default::default()
        }
    }
}

/// A 1-based `line`/`character` position resolved from a type-checker span.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FluxTypeSpan {
    line: u32,
    character: u32,
}

/// Resolves a `TypeError`'s byte-offset span start into a 1-based line/column.
/// The type checker emits absolute byte offsets in its [`flux_syntax::Span`];
/// [`flux_parser::Location::from_offset`] is the same routine the parser uses.
#[must_use]
fn resolve_type_span(source: &str, offset: usize) -> FluxTypeSpan {
    let loc = flux_parser::Location::from_offset(source, offset);
    FluxTypeSpan {
        line: loc.line,
        character: loc.column,
    }
}

impl FluxLsp {
    /// Computes the full `parse -> type_check` diagnostics for `text` (FLUX-025).
    /// Reuses `flux_parser::parse` then `flux_types::type_check` so editor
    /// diagnostics match `flux dev`. Parse/type errors are tagged `source`
    /// `"parse"`/`"type"`. When parsing fails the type-checker is skipped.
    /// `types_enabled` selects whether type-checking runs (the `flux lsp --types`
    /// flag); when `false` this equals [`Self::diagnostics_for_text`].
    #[must_use]
    pub fn diagnostics_with_types(
        &self,
        path: &std::path::Path,
        text: &str,
        types_enabled: bool,
    ) -> Vec<LspDiagnostic> {
        let path_str = path.to_string_lossy().into_owned();
        let mut out = Vec::new();
        let ast = match flux_parser::parse(text, 0, &path_str) {
            Ok(ast) => ast,
            Err(err) => {
                out.push(LspDiagnostic {
                    line: err.location.line,
                    character: err.location.column,
                    length: err.span.len().max(1),
                    severity: 1,
                    message: err.message.clone(),
                    source: "parse".to_owned(),
                });
                return out;
            }
        };
        if !types_enabled {
            return out;
        }
        if let Err(type_err) = flux_types::type_check(&ast) {
            let FluxTypeSpan { line, character } =
                resolve_type_span(text, type_err.span.start as usize);
            let hint = type_err
                .hint
                .as_deref()
                .map(|h| format!(" (hint: {h})"))
                .unwrap_or_default();
            out.push(LspDiagnostic {
                line,
                character,
                length: type_err.span.len().max(1),
                severity: 1,
                message: format!("{}{}", type_err.message, hint),
                source: "type".to_owned(),
            });
        }
        out
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
                        TextDocumentSyncKind::INCREMENTAL,
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
                    // FLUX-027: advertise the three new providers.
                    hover_provider: Some(async_lsp::lsp_types::HoverProviderCapability::Simple(true)),
                    definition_provider: Some(async_lsp::lsp_types::OneOf::Left(true)),
                    completion_provider: Some(CompletionOptions {
                        trigger_characters: Some(vec![
                            ".".to_owned(),
                            "(".to_owned(),
                            ":".to_owned(),
                        ]),
                        ..Default::default()
                    }),
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

    fn did_change(&mut self, params: DidChangeTextDocumentParams) -> Self::NotifyResult {
        self.did_change(params)
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

    fn definition(
        &mut self,
        params: GotoDefinitionParams,
    ) -> futures::future::BoxFuture<
        'static,
        Result<Option<GotoDefinitionResponse>, async_lsp::ResponseError>,
    > {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let text = self
            .documents
            .lock()
            .expect("documents mutex poisoned")
            .get(&uri)
            .cloned()
            .unwrap_or_default();
        Box::pin(async move {
            let Some(cursor) = util::position_to_offset(&text, pos.line, pos.character) else {
                return Ok(None);
            };
            let Some(ast) = flux_parser::parse(&text, 0, uri.path()).ok() else {
                return Ok(None);
            };
            let Some(span) = goto_def::DefIndex::build(&ast).resolve(&text, cursor) else {
                return Ok(None);
            };
            let range = util::span_to_range(&text, span);
            Ok(Some(GotoDefinitionResponse::Scalar(
                async_lsp::lsp_types::Location { uri, range },
            )))
        })
    }

    fn hover(
        &mut self,
        params: HoverParams,
    ) -> futures::future::BoxFuture<'static, Result<Option<Hover>, async_lsp::ResponseError>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let text = self
            .documents
            .lock()
            .expect("documents mutex poisoned")
            .get(&uri)
            .cloned()
            .unwrap_or_default();
        Box::pin(async move {
            let Some(cursor) = util::position_to_offset(&text, pos.line, pos.character) else {
                return Ok(None);
            };
            let Ok(ast) = flux_parser::parse(&text, 0, uri.path()) else {
                return Ok(None);
            };
            let Ok(typed) = flux_types::type_check(&ast) else {
                return Ok(None);
            };
            Ok(hover::hover_at(&ast, &typed, &text, cursor))
        })
    }

    fn completion(
        &mut self,
        params: CompletionParams,
    ) -> futures::future::BoxFuture<
        'static,
        Result<Option<CompletionResponse>, async_lsp::ResponseError>,
    > {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let text = self
            .documents
            .lock()
            .expect("documents mutex poisoned")
            .get(&uri)
            .cloned()
            .unwrap_or_default();
        Box::pin(async move { Ok(completion::completions_at(&text, pos)) })
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
    let (main_loop, _client_socket) =
        MainLoop::new_server(|client| Router::from_language_server(FluxLsp::with_client(client)));
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

    #[test]
    fn diagnostics_with_types_reports_type_error_with_hint() {
        let src = "compo Bad\n  let s = 1 + \"not a number\"\n\n";
        let path = fixture(src);
        let text = std::fs::read_to_string(&path).expect("read");
        let diags = FluxLsp::new().diagnostics_with_types(&path, &text, true);
        let type_diag = diags
            .iter()
            .find(|d| d.source == "type")
            .expect("expected a type-source diagnostic");
        assert!(!type_diag.message.is_empty());
        assert!(
            type_diag.message.contains("hint"),
            "type diagnostic must carry a how-hint: {type_diag:?}"
        );
        assert!(type_diag.line >= 1);
        assert_eq!(type_diag.severity, 1);
        assert_eq!(type_diag.source, "type");
    }

    #[test]
    fn diagnostics_with_types_flags_parse_error_without_running_types() {
        let src = "compo Broken\n  if true {\n";
        let path = fixture(src);
        let text = std::fs::read_to_string(&path).expect("read");
        let diags = FluxLsp::new().diagnostics_with_types(&path, &text, true);
        assert!(diags.iter().any(|d| d.source == "parse"));
        assert!(!diags.iter().any(|d| d.source == "type"));
    }

    #[test]
    fn diagnostics_with_types_disabled_skips_type_check() {
        let src = "compo Bad\n  let s = 1 + \"not a number\"\n\n";
        let path = fixture(src);
        let text = std::fs::read_to_string(&path).expect("read");
        let diags = FluxLsp::new().diagnostics_with_types(&path, &text, false);
        assert!(
            !diags.iter().any(|d| d.source == "type"),
            "types disabled must skip type-check"
        );
    }

    #[test]
    fn diagnostics_with_types_serializes_like_cli_contract() {
        let src = "compo Bad\n  let s = 1 + \"not a number\"\n\n";
        let path = fixture(src);
        let text = std::fs::read_to_string(&path).expect("read");
        let diags = FluxLsp::new().diagnostics_with_types(&path, &text, true);
        let json = serde_json::to_string(&diags).expect("serialize");
        assert!(
            json.contains("\"source\":\"type\""),
            "JSON must carry the type source: {json}"
        );
        let back: Vec<LspDiagnostic> = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, diags);
    }

    // FLUX-027 integration smoke test: the `definition` handler resolves a usage
    // to its declaration span using the pure provider, over the real
    // `LanguageServer` trait path.
    #[tokio::test]
    async fn definition_resolves_usage_to_declaration_span() {
        use async_lsp::lsp_types::{Position, TextDocumentPositionParams};

        let mut server = FluxLsp::new();
        let uri: Url = "file:///counter.flux".parse().expect("uri");
        let text = "compo Counter\n  Button(text: \"tap\")\n  Counter()\n".to_owned();
        let _ = server.did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "flux".to_owned(),
                version: 1,
                text: text.clone(),
            },
        });

        // Cursor on the `Counter()` usage (line 2, column 2).
        let resp = server
            .definition(GotoDefinitionParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: async_lsp::lsp_types::TextDocumentIdentifier {
                        uri: uri.clone(),
                    },
                    position: Position {
                        line: 2,
                        character: 2,
                    },
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .expect("definition ok");
        let GotoDefinitionResponse::Scalar(loc) = resp.expect("some definition") else {
            panic!("expected scalar location");
        };
        // Declaration `Counter` starts at byte 6 (0-based) → line 0, col 6.
        assert_eq!(loc.range.start.line, 0);
        assert_eq!(loc.range.start.character, 6);
    }

    #[test]
    fn apply_range_edit_inserts_within_line() {
        // Insert " world" at the end of line 0 ("compo Ok" -> "compo Ok world").
        let range = async_lsp::lsp_types::Range {
            start: async_lsp::lsp_types::Position {
                line: 0,
                character: 8,
            },
            end: async_lsp::lsp_types::Position {
                line: 0,
                character: 8,
            },
        };
        let out =
            crate::util::apply_range_edit("compo Ok\n", range, " world").expect("edit applies");
        assert_eq!(out, "compo Ok world\n");
    }

    #[test]
    fn apply_range_edit_replaces_span_across_lines() {
        // Replace "Ok\n  Text" (lines 0-1) with "Bad\n  Button".
        let range = async_lsp::lsp_types::Range {
            start: async_lsp::lsp_types::Position {
                line: 0,
                character: 6,
            },
            end: async_lsp::lsp_types::Position {
                line: 1,
                character: 6,
            },
        };
        let out = crate::util::apply_range_edit(
            "compo Ok\n  Text(text: \"hi\")\n",
            range,
            "Bad\n  Button",
        )
        .expect("edit applies");
        assert_eq!(out, "compo Bad\n  Button(text: \"hi\")\n");
    }

    #[test]
    fn did_change_folds_incremental_edit_into_document_cache() {
        use async_lsp::lsp_types::{
            DidChangeTextDocumentParams, TextDocumentContentChangeEvent,
            VersionedTextDocumentIdentifier,
        };
        let mut server = FluxLsp::new();
        let uri: Url = "file:///edit.flux".parse().expect("uri");
        // Seed via didOpen.
        let _ = server.did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "flux".to_owned(),
                version: 1,
                text: "compo Ok\n  Text(text: \"hi\")\n".to_owned(),
            },
        });
        // Incremental edit: change `Ok` -> `Bad` at line 0, col 6..8.
        let range = async_lsp::lsp_types::Range {
            start: async_lsp::lsp_types::Position {
                line: 0,
                character: 6,
            },
            end: async_lsp::lsp_types::Position {
                line: 0,
                character: 8,
            },
        };
        let _ = server.did_change(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version: 2,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: Some(range),
                range_length: None,
                text: "Bad".to_owned(),
            }],
        });
        let cached = server
            .documents
            .lock()
            .expect("documents mutex poisoned")
            .get(&uri)
            .cloned()
            .expect("document cached");
        assert_eq!(cached, "compo Bad\n  Text(text: \"hi\")\n");
    }
}
