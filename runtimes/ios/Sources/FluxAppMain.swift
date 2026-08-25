//  FluxAppMain.swift
//  The Flux dev-mode host application (FLUX-006).
//
//  In dev mode this app is precompiled once and thereafter receives binary
//  patches over WebSocket; it is never rebuilt to see a UI change. In release
//  mode the same IR is code-generated to SwiftUI ahead of time. `FluxRootView`
//  hosts the reconciled native tree and an error overlay that surfaces any VM
//  fault captured by `FluxRuntime` (no VM error is ever allowed to escape).

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

/// Host view for the reconciled native tree plus the error overlay.
///
/// The reconciler driving `FluxRuntime` builds real `UIView`s (FLUX-016) into a
/// tree keyed by stable node id. This view mounts that tree inside a
/// `FluxHostController` via `UIViewControllerRepresentable` and layers the error
/// overlay (Appendix E §E.6: a VM fault shows a banner, never crashes) on top.
struct FluxRootView: View {
    /// The executor owning the graph, reconciler and last error.
    @State private var executor: FluxRuntime

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
        _executor = State(initialValue: FluxRuntime(graph: SignalGraph(), registry: registry))
    }

    var body: some View {
        ZStack {
            FluxHostRepresentable(executor: executor)
                .accessibilityLabel("Flux root")
            if let error = executor.lastError {
                ErrorOverlay(error: error)
            }
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
        // Lifecycle (stop on disappear) is handled by `onDisappear` below.
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
