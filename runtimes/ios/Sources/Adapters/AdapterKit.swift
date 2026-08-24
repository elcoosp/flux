//  AdapterKit.swift
//  Native adapter contract (Appendix F) + in-dir mock adapters (FLUX-006 scope #11).
//
//  The `FluxAdapter` protocol is the contract every platform widget implements:
//  given a `ShadowNode` it produces a native view and reacts to prop changes.
//  In dev mode (this issue) we use `MockAdapter`s that record the operations
//  they receive so tests can assert on them without a real UIKit/SwiftUI tree.
//  Wiring to the real `FluxUIKit` package is deferred to FLUX-016.

import Foundation

/// A native view produced by an adapter. In dev mode this is a lightweight
/// record; in release mode it would be a real `UIView`/`View`.
protocol FluxView: AnyObject {
    /// The stable node id this view was built for.
    var nodeId: UInt32 { get }
    /// Applies a changed prop to the already-built view.
    func apply(prop: Prop)
    /// Removes the view from its parent (reconciliation removal).
    func detach()
}

/// The contract every adapter conforms to. The props are the contract: an
/// adapter maps `ShadowNode.props` onto native view state. Both the dev
/// (mock) and release (FluxUIKit) implementations consume the same props.
protocol FluxAdapter {
    /// The `NodeKind` this adapter handles (e.g. `.primitive` for `Text`).
    var handles: NodeKind { get }
    /// Builds a fresh native view for `node`.
    func build(_ node: ShadowNode) -> FluxView
}

/// Records the operations a mock adapter performs, so tests can assert the
/// reconciler drives the right views without a real rendering surface.
final class MockView: FluxView {
    let nodeId: UInt32
    private(set) var appliedProps: [Prop] = []
    private(set) var detached = false

    init(nodeId: UInt32) { self.nodeId = nodeId }

    func apply(prop: Prop) { appliedProps.append(prop) }
    func detach() { detached = true }
}

/// A mock adapter that records every node it builds and prop it applies.
final class MockAdapter: FluxAdapter {
    let handles: NodeKind
    private(set) var built: [UInt32: MockView] = [:]
    private(set) var buildOrder: [UInt32] = []

    init(handles: NodeKind) { self.handles = handles }

    func build(_ node: ShadowNode) -> FluxView {
        let view = MockView(nodeId: node.id)
        built[node.id] = view
        buildOrder.append(node.id)
        return view
    }
}

/// The registry of adapters keyed by the node kind they serve.
struct AdapterRegistry {
    private var adapters: [NodeKind: any FluxAdapter]

    init(_ adapters: [any FluxAdapter]) {
        var map: [NodeKind: any FluxAdapter] = [:]
        for a in adapters { map[a.handles] = a }
        self.adapters = map
    }

    /// Returns the adapter for `kind`, or `nil` if none is registered.
    func adapter(for kind: NodeKind) -> (any FluxAdapter)? {
        adapters[kind]
    }
}
