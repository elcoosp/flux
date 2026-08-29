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
            if let error = executor.lastError {
                ErrorOverlay(error: error)
            }
            if let serverError = executor.serverError {
                ServerErrorOverlay(error: serverError)
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

/// Inline error overlay shown when a VM fault is captured by the executor.
struct ErrorOverlay: View {
    let error: VmError

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Flux VM fault")
                .font(.headline)
            Text(error.kind.name)
                .font(.subheadline)
            Text("at byte offset \(error.offset)")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding()
        .background(.thinMaterial)
        .cornerRadius(12)
        .padding()
    }
}

/// FR-017 "Reconnecting…" banner shown while the dev-server socket is down.
/// Reuses the error-overlay's material styling so the two banners read as one
/// family; it is informational (amber) rather than a fault (red).
/// Inline overlay shown when the dev server reports a failed recompile via an
/// `Error` (0x03) frame. Mirrors `ErrorOverlay`'s material styling; it is a red
/// fault banner that keeps the last good tree visible (Appendix E §E.6).
struct ServerErrorOverlay: View {
    let error: ServerError

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Flux compile error")
                .font(.headline)
            Text(error.message)
                .font(.subheadline)
                .textSelection(.enabled)
            if let location = error.location {
                Text(location)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .padding()
        .background(.thinMaterial)
        .cornerRadius(12)
        .padding()
        .frame(maxWidth: .infinity, alignment: .top)
    }
}

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
