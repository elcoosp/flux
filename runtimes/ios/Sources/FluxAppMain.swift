//
//  FluxAppMain.swift
//  Skeleton placeholder created by the foundation pass (FLUX-001).
//  The ios-runtime agent (FLUX-006) replaces this with the real host app:
//  wire client, VM, signal graph, reconciler and shadow tree.
//

import SwiftUI

/// The Flux dev-mode host application.
///
/// In dev mode this app is precompiled once and thereafter receives binary
/// patches over WebSocket; it is never rebuilt to see a UI change. In release
/// mode the same IR is code-generated to SwiftUI ahead of time.
@main
struct FluxAppMain: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}

/// Placeholder root view, replaced by the runtime's shadow-tree host view.
struct ContentView: View {
    var body: some View {
        Text("Flux host — awaiting FLUX-006")
    }
}
