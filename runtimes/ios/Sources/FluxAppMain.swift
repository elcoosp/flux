//  FluxAppMain.swift
//  The Flux dev-mode host application (FLUX-006) + FR-017 reconnect UX.
//
//  In dev mode this app is precompiled once and thereafter receives binary
//  patches over WebSocket; it is never rebuilt to see a UI change. In release
//  mode the same IR is code-generated to SwiftUI ahead of time. `FluxRootView`
//  hosts the reconciled native tree, layers the VM error overlay (Appendix E
//  §E.6), and — per FR-017 — shows a "Reconnecting…" banner with a 1-second
//  retry while the dev-server socket is down.

import SwiftUI
import FluxHost
import UserNotifications

/// The Flux dev-mode host application.
@main
struct FluxAppMain: App {
    var body: some Scene {
        WindowGroup {
            FluxRootView()
        }
    }
}

/// Host view for the reconciled native tree plus the error and reconnect
/// overlays.
///
/// The reconciler driving `FluxExecutor` builds real `UIView`s (FLUX-016) into a
/// tree keyed by stable node id. This view mounts that tree inside a
/// `FluxHostController` via `UIViewControllerRepresentable`, feeds frames from
/// the live `FluxWebSocketTransport` into the runtime, and layers two overlays
/// on top: a VM fault banner (Appendix E §E.6, never crashes) and the FR-017
/// "Reconnecting…" banner while the socket is down.
struct FluxRootView: View {
    /// The executor owning the graph, reconciler and last error.
    @State private var executor: FluxExecutor
    /// The connection state driving the reconnect banner (FR-017).
    @StateObject private var connection: HostConnectionState
    /// The live WebSocket transport feeding frames into the executor.
    @State private var transport: FluxTransport

    init() {
        // Seed the standard-library component names so the registry can resolve
        // each primitive's adapter once an Init frame interns them. The dev
        // host starts with an empty graph until the first Init frame arrives.
        var table = StringTable()
        table.intern(0, "Text")
        table.intern(1, "Button")
        table.intern(2, "Column")
        table.intern(3, "Row")
        table.intern(4, "TextField")
        table.intern(5, "Router")
        table.intern(6, "Screen")
        let registry = AdapterRegistry(table: table)
        // FLUX-045: inject the real device-OS capability host so the six concrete
        // caps (6..=11) perform genuine Apple-framework work (UNUserNotificationCenter /
        // LAContext / BGTaskScheduler / FileManager / UIApplication / CMMotionManager)
        // instead of the dev-safe echoes. The Foundation-only `FluxHost` core stays
        // pure; the real bodies live in the app target's `IOSNativeCapabilityHost`.
        CapabilityRegistry.realNativeHost = IOSNativeCapabilityHost(table: table)
        let runtime = FluxExecutor(graph: SignalGraph(), registry: registry)
        let connection = HostConnectionState()
        // Dev server endpoint. Resolution order:
        //   1. `FLUX_WS_URL` launch environment variable (lets a simulator or
        //      physical device reach the Mac's LAN IP without rebuilding the app),
        //   2. the `FLUX_WS_URL` Info.plist key (CI / pre-provisioned builds),
        //   3. loopback default.
        // A simulator or device that cannot reach the Mac's loopback should be
        // launched with `FLUX_WS_URL=ws://<mac-lan-ip>:7331` (see AGENTS.md §3.9).
        let wsUrlString = ProcessInfo.processInfo.environment["FLUX_WS_URL"]
            ?? (Bundle.main.object(forInfoDictionaryKey: "FLUX_WS_URL") as? String)
            ?? "ws://127.0.0.1:7331"
        guard let wsUrl = URL(string: wsUrlString) else {
            fatalError("FLUX_WS_URL is not a valid WebSocket URL: \(wsUrlString)")
        }
        let transport = FluxWebSocketTransport(url: wsUrl)
        // Wire the VM's string interner to the live transport so derived strings
        // (STR_CONCAT / TO_STRING results, native event payloads) are interned
        // through the dev server's `InternString` RPC and receive a canonical id
        // (brittleness 4c). `StringInterned` replies are routed back into the
        // client by the `onFrame` handler below.
        let interner = InternStringClient(transport: transport)
        runtime.setInterner(interner)
        _executor = State(initialValue: runtime)
        _connection = StateObject(wrappedValue: connection)
        _transport = State(initialValue: transport)
    }

    var body: some View {
        ZStack {
            FluxHostRepresentable(executor: executor)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .accessibilityLabel("Flux root")
            if let error = executor.lastFluxError {
                FluxErrorOverlay(error: error)
            }
            if connection.isReconnecting {
                ReconnectingOverlay()
            }
        }
        .task {
            let _ = try? "TASK_RAN \(Date())\n".write(to: URL(fileURLWithPath: NSTemporaryDirectory() + "flux_task.log"), atomically: true, encoding: .utf8)
            // Open the socket and bind status → banner. Frames decode on the
            // main actor (FluxExecutor is @MainActor) and drive the tree; the dev
            // server pushes a fresh Init frame on each (re)connect, so a
            // reconnect implicitly re-requests the tree (FR-017).
            connection.bind(transport)
            transport.onFrame = { [executor] data in
                executor.handleFrame(data)
            }
            transport.connect()
            // Open the host → DevTools channel so the Flux DevTools desktop app
            // can observe the live VM/signal flow (PRD-P). Telemetry rides the
            // existing `:7331` patch-channel WebSocket (the only port the iOS
            // Simulator forwards from the device), so no separate device→:7333
            // socket is needed. `transport.send` drops frames when offline.
            fluxDevtoolsConnect(send: { transport.send($0) })
        }
        .onDisappear {
            transport.close()
        }
    }
}

/// Bridges `FluxHostController` into SwiftUI, hosting the reconciler's root
/// `UIView`. The controller is created once and retained; the reconciler still
/// owns every per-node view (the controller only mounts the root).
private struct FluxHostRepresentable: UIViewControllerRepresentable {
    let executor: FluxExecutor

    func makeUIViewController(context: Context) -> FluxHostController {
        FluxHostController(executor: executor)
    }

    func updateUIViewController(_ controller: FluxHostController, context: Context) {
        // The reconciler drives in-place view updates; nothing to sync here.
    }
}

/// The unified on-device error overlay (FLUX-028 / FLUX-075 / ADR-0057).
///
/// One visual language for every fault the runtime can surface — a VM fault
/// (`.vm`), a wire/protocol fault (`.wire`), a runtime capability error
/// (`.capability`/`.runtime`), or a server compile/type error (`.compile`/
/// `.server`/`.parse`/`.type`). The executor collapses `VmError` and
/// `ServerError` into a single `FluxError` (carrying an ADR-0057 source
/// excerpt); this view is pure presentation and never guesses at the cause.
///
/// Visual spec (shared with the Android `ErrorOverlay`, §3.11):
/// - a colored accent bar keyed by severity (fatal = red, fault = amber/red),
/// - a one-line title = kind + short label,
/// - the human message (what/why/how),
/// - `path:line:col` when an excerpt is present,
/// - the cited source line in monospace with a `^` caret under the column,
/// - a formatted dispatch stack when call sites are available.
///
/// Fatal compile/type/parse errors render as a full-width top-anchored tinted
/// panel (the tree it would have replaced is gone); VM/wire/runtime faults
/// render as a dismissible material card that keeps the last good tree on
/// screen (Appendix E §E.6).
struct FluxErrorOverlay: View {
    let error: FluxError

    /// Fatal compile/type/parse/server errors replace the tree; VM/wire/runtime
    /// faults are recoverable and keep the last good UI.
    private var isFatal: Bool {
        switch error.kind {
        case .parse, .type, .compile, .server: return true
        default: return false
        }
    }

    /// The accent color keyed by fault kind.
    private var accent: Color {
        switch error.kind {
        case .compile, .parse, .type, .server: return .red
        case .vm: return .red
        case .runtime, .capability: return .orange
        case .wire: return .orange
        }
    }

    private var titleLabel: String {
        switch error.kind {
        case .compile, .parse, .type, .server: return "Flux compile error"
        case .vm: return "Flux VM fault"
        case .runtime: return "Flux runtime error"
        case .capability: return "Flux capability error"
        case .wire: return "Flux wire error"
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .top, spacing: 10) {
                RoundedRectangle(cornerRadius: 4)
                    .fill(accent)
                    .frame(width: 4)
                VStack(alignment: .leading, spacing: 2) {
                    Text(titleLabel)
                        .font(.headline)
                        .foregroundStyle(.primary)
                    Text(error.kind.rawValue)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .textCase(.uppercase)
                }
                Spacer(minLength: 0)
            }
            Text(error.message)
                .font(.subheadline)
                .foregroundStyle(.primary)
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
            if let excerpt = error.excerpt {
                VStack(alignment: .leading, spacing: 2) {
                    Text("\(excerpt.path):\(excerpt.line):\(excerpt.column)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .textSelection(.enabled)
                    MonoSnippetView(snippet: excerpt.snippet, column: Int(excerpt.column))
                }
            }
            if !error.callSites.isEmpty {
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(Array(error.callSites.enumerated()), id: \.offset) { _, site in
                        Text(site)
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                            .textSelection(.enabled)
                    }
                }
            }
        }
        .padding(14)
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .stroke(accent.opacity(0.5), lineWidth: 1)
        )
        .padding()
        .frame(maxWidth: .infinity, alignment: isFatal ? .top : .bottom)
    }
}

/// Renders a source line in monospace with a `^` caret under the cited column.
private struct MonoSnippetView: View {
    let snippet: String
    let column: Int

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(snippet)
                .font(.system(.caption, design: .monospaced))
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
            if column > 0 {
                Text(caret)
                    .font(.system(.caption, design: .monospaced))
                    .foregroundStyle(.red)
            }
        }
    }

    /// `column - 1` spaces followed by `^`. Clamped so an out-of-range column
    /// still renders a visible marker at the line start.
    private var caret: String {
        let pad = max(0, min(column, snippet.count) - 1)
        return String(repeating: " ", count: pad) + "^"
    }
}

/// FR-017 "Reconnecting…" banner shown while the dev-server socket is down.
/// Reuses the error-overlay's material styling so the two banners read as one
/// family; it is informational (amber) rather than a fault (red).
struct ReconnectingOverlay: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("Flux")
                .font(.headline)
            Text("Reconnecting…")
                .font(.subheadline)
        }
        .padding()
        .background(.thinMaterial)
        .cornerRadius(12)
        .padding()
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom)
    }
}
