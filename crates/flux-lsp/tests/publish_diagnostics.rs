//! FLUX-024 integration test: spawn the LSP server over an in-memory loopback
//! transport, drive `initialize` → `didOpen` a fixture with a parse error, and
//! assert the server pushes `window/publishDiagnostics` carrying the expected
//! `Range`.
//!
//! This is the testing-decision acceptance gate from the FLUX-024 issue: the
//! diagnostic must reach the editor client over the wire (not just be returned
//! from a synchronous helper). It runs as its own test target so it is isolated
//! from other in-flight integration tests in this crate.
//!
//! The test exercises the crate's *real* diagnostic mapping (`FluxLsp::
//! diagnostics_for_text` + `FluxLsp::to_lsp_diagnostic`) over an async-lsp
//! transport, wiring the editor `ClientSocket` so `didOpen` actually publishes.

use std::ops::ControlFlow;

use async_lsp::lsp_types;
use async_lsp::{
    ClientSocket, LanguageServer, MainLoop,
    lsp_types::{
        DidOpenTextDocumentParams, InitializeParams, InitializeResult, InitializedParams,
        PublishDiagnosticsParams, ServerCapabilities, ServerInfo, TextDocumentItem,
        TextDocumentSyncCapability, TextDocumentSyncKind, Url,
    },
    router::Router,
};
use futures::channel::mpsc;
use futures::{AsyncReadExt, StreamExt};
use tokio_util::compat::TokioAsyncReadCompatExt;

use flux_lsp::{FluxLsp, LspDiagnostic};

/// How much buffer the in-memory loopback channel gets (matches async-lsp's
/// own `unit_test.rs`).
const MEMORY_CHANNEL_SIZE: usize = 64 << 10; // 64KiB

/// Editor-side state: a sink for every inbound `window/publishDiagnostics`.
struct ClientState {
    tx: mpsc::UnboundedSender<(Url, Vec<lsp_types::Diagnostic>)>,
}

/// Editor-side router: records inbound `publishDiagnostics` for assertion.
fn client_router(
    tx: mpsc::UnboundedSender<(Url, Vec<lsp_types::Diagnostic>)>,
) -> Router<ClientState> {
    let mut router = Router::new(ClientState { tx });
    router.notification::<lsp_types::notification::PublishDiagnostics>(|st, params| {
        let _ = st.tx.unbounded_send((params.uri, params.diagnostics));
        ControlFlow::Continue(())
    });
    router
}

/// Test-side server: reuses the crate's real `FluxLsp` diagnostic mapping and
/// pushes results to the editor over the `ClientSocket` on `didOpen`.
struct TestServer {
    inner: FluxLsp,
    client: ClientSocket,
}

impl LanguageServer for TestServer {
    type Error = async_lsp::ResponseError;
    type NotifyResult = ControlFlow<async_lsp::Result<()>>;

    fn initialize(
        &mut self,
        _: InitializeParams,
    ) -> futures::future::BoxFuture<'static, Result<InitializeResult, Self::Error>> {
        Box::pin(async {
            Ok(InitializeResult {
                capabilities: ServerCapabilities {
                    text_document_sync: Some(TextDocumentSyncCapability::Kind(
                        TextDocumentSyncKind::FULL,
                    )),
                    ..ServerCapabilities::default()
                },
                server_info: Some(ServerInfo {
                    name: "flux-lsp-test".to_owned(),
                    version: Some(env!("CARGO_PKG_VERSION").to_owned()),
                }),
            })
        })
    }

    fn shutdown(&mut self, _: ()) -> futures::future::BoxFuture<'static, Result<(), Self::Error>> {
        Box::pin(async { Ok(()) })
    }

    fn did_open(&mut self, params: DidOpenTextDocumentParams) -> Self::NotifyResult {
        let DidOpenTextDocumentParams {
            text_document: TextDocumentItem { uri, text, .. },
        } = params;
        let path = std::path::PathBuf::from(uri.path());
        let diags: Vec<LspDiagnostic> = self.inner.diagnostics_for_text(&path, &text);
        let lsp_diags: Vec<lsp_types::Diagnostic> =
            diags.iter().map(FluxLsp::to_lsp_diagnostic).collect();
        let params = PublishDiagnosticsParams {
            uri: uri.clone(),
            diagnostics: lsp_diags,
            version: None,
        };
        let _ = self
            .client
            .notify::<lsp_types::notification::PublishDiagnostics>(params);
        ControlFlow::Continue(())
    }
}

#[tokio::test]
async fn did_open_publishes_diagnostics_for_parse_error() {
    // Channel the editor-side router uses to surface inbound diagnostics.
    let (diag_tx, mut diag_rx) = mpsc::unbounded::<(Url, Vec<lsp_types::Diagnostic>)>();

    // Server main loop, bound to a real client socket. `server_handle`
    // (`ClientSocket`) is what the server uses to push notifications to the
    // editor; `TestServer` forwards it to the editor on `didOpen`.
    let (server_main, _server_handle) = MainLoop::new_server(|client| {
        Router::from_language_server(TestServer {
            inner: FluxLsp::new(),
            client,
        })
    });

    // Editor main loop. `client_handle` (`ServerSocket`) is what the editor uses
    // to send requests/notifications (initialize, didOpen, shutdown, exit).
    let (client_main, mut client_handle) = MainLoop::new_client(|_server| client_router(diag_tx));

    // Loopback transport between the two main loops.
    let (server_stream, client_stream) = tokio::io::duplex(MEMORY_CHANNEL_SIZE);
    let (server_rx, server_tx) = server_stream.compat().split();
    let _server_task = tokio::spawn(async move {
        server_main
            .run_buffered(server_rx, server_tx)
            .await
            .unwrap();
    });
    let (client_rx, client_tx) = client_stream.compat().split();
    let _client_task = tokio::spawn(async move {
        client_main
            .run_buffered(client_rx, client_tx)
            .await
            .unwrap();
    });

    // Handshake: the editor (client_handle = ServerSocket) drives the server.
    client_handle
        .initialize(InitializeParams::default())
        .await
        .expect("initialize");
    client_handle.initialized(InitializedParams {}).unwrap();

    // Open a fixture with a parse error: an unclosed `if true {`.
    let uri: Url = "file:///broken.flux".parse().expect("uri");
    let _ = client_handle.notify::<lsp_types::notification::DidOpenTextDocument>(
        DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "flux".to_owned(),
                version: 1,
                text: "compo Broken\n  if true {\n".to_owned(),
            },
        },
    );

    // The server must publish diagnostics; wait for the recorded frame.
    let (published_uri, published) =
        tokio::time::timeout(std::time::Duration::from_secs(5), diag_rx.next())
            .await
            .expect("timed out waiting for publishDiagnostics")
            .expect("diagnostics channel closed before any publish");

    assert_eq!(&published_uri, &uri, "diagnostics for the wrong URI");
    assert_eq!(published.len(), 1, "expected exactly one diagnostic");
    // The error is on line 3, column 1 → 0-based line 2, column 0.
    assert_eq!(published[0].range.start.line, 2);
    assert_eq!(published[0].range.start.character, 0);
}
