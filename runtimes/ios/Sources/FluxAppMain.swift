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
/// The reconciler driving `FluxRuntime` builds real `UIView`s (FLUX-016) into a
/// tree keyed by stable node id. This view mounts that tree inside a
/// `FluxHostController` via `UIViewControllerRepresentable`, feeds frames from
/// the live `FluxWebSocketTransport` into the runtime, and layers two overlays
/// on top: a VM fault banner (Appendix E §E.6, never crashes) and the FR-017
/// "Reconnecting…" banner while the socket is down.
struct FluxRootView: View {
    /// The executor owning the graph, reconciler and last error.
    @State private var executor: FluxRuntime
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
        let runtime = FluxRuntime(graph: SignalGraph(), registry: registry)
        let connection = HostConnectionState()
        // Dev server runs locally; plaintext WebSocket is permitted by the app's
        // NSAppTransportSecurity (see project.yml).
        let transport = FluxWebSocketTransport(url: URL(string: "ws://127.0.0.1:9001")!)
        _executor = State(initialValue: runtime)
        _connection = StateObject(wrappedValue: connection)
        _transport = State(initialValue: transport)
    }

    var body: some View {
        ZStack {
            FluxHostRepresentable(executor: executor)
                .accessibilityLabel("Flux root")
            if let error = executor.lastError {
                ErrorOverlay(error: error)
            }
            if connection.isReconnecting {
                ReconnectingOverlay()
            }
        }
        .task {
            // Open the socket and bind status → banner. Frames decode on the
            // main actor (FluxRuntime is @MainActor) and drive the tree; the dev
            // server pushes a fresh Init frame on each (re)connect, so a
            // reconnect implicitly re-requests the tree (FR-017).
            connection.bind(transport)
            transport.onFrame = { [executor] data in
                _ = try? executor.applyFrame(data)
            }
            transport.connect()
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
    let executor: FluxRuntime

    func makeUIViewController(context: Context) -> FluxHostController {
        FluxHostController(executor: executor)
    }

    func updateUIViewController(_ controller: FluxHostController, context: Context) {
        // The reconciler drives in-place view updates; nothing to sync here.
    }
}

/// Inline error overlay shown when a VM fault is captured by the executor.
struct ErrorOverlay: View {
    let error: VMError

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
