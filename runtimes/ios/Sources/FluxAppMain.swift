//  FluxAppMain.swift
//  The Flux dev-mode host application (FLUX-006).
//
//  In dev mode this app is precompiled once and thereafter receives binary
//  patches over WebSocket; it is never rebuilt to see a UI change. In release
//  mode the same IR is code-generated to SwiftUI ahead of time. `FluxRootView`
//  hosts the reconciled native tree and an error overlay that surfaces any VM
//  fault captured by `FluxRuntime` (no VM error is ever allowed to escape).

import SwiftUI

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
/// The reconciler's `MockView`s stand in for real `UIView`s in dev mode (the
/// live UIKit tree is wired through `FluxUIKit` in FLUX-016). The overlay shows
/// the last VM error with its byte offset so developers see failures inline.
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
            Color.clear
                .accessibilityLabel("Flux root")
            if let error = executor.lastError {
                ErrorOverlay(error: error)
            }
        }
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
