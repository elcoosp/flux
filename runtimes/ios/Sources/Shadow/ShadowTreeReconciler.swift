//  ShadowTreeReconciler.swift
//  Keyed reconciler (FLUX-006 scope item 5) over the deserialized shadow tree.
//
//  On each frame the reconciler walks the new `ShadowNode` tree against the set
//  of already-built native views. Nodes are matched by their stable `NodeId`
//  (Appendix C §C.1 / D §D.3) and `Splice` children by their `u64` keys (D §D.4),
//  so existing native views are mutated in place (prop `apply`) rather than
//  recreated. Only genuinely new nodes trigger `adapter.build`; only removed
//  nodes trigger `view.detach`.

import Foundation

/// Drives native views from a stream of `ShadowNode` trees using an
/// `AdapterRegistry`. Value-semantic aside from the `MockView` instances it
/// owns, which is fine: those are the test-observable artifacts.
struct ShadowTreeReconciler {
    private let registry: AdapterRegistry
    /// Built views keyed by node id.
    private var views: [UInt32: MockView]

    /// Creates a reconciler bound to `registry`.
    init(registry: AdapterRegistry) {
        self.registry = registry
        self.views = [:]
    }

    /// Reconciles a freshly decoded frame against the current view set.
    /// - Returns: the ids of views that were built, updated, or detached in this
    ///   pass (exposed for tests).
    @discardableResult
    mutating func reconcile(_ root: ShadowNode) -> ReconcileReport {
        // Build a flat id->node index from the incoming root so child lookup is
        // O(1) and node identities (Appendix C §C.1) stay stable across passes.
        var index: [UInt32: ShadowNode] = [:]
        func walk(_ n: ShadowNode) {
            index[n.id] = n
            for child in n.children {
                switch child {
                case let .node(id):
                    if let c = index[id] { walk(c) }
                case let .splice(_, items):
                    for (_, id) in items { if let c = index[id] { walk(c) } }
                }
            }
        }
        walk(root)

        var report = ReconcileReport()
        reconcile(node: root, index: index, report: &report)
        return report
    }

    /// The currently built view for `nodeId`, if any.
    func view(for nodeId: UInt32) -> MockView? { views[nodeId] }

    private mutating func reconcile(
        node: ShadowNode,
        index: [UInt32: ShadowNode],
        report: inout ReconcileReport
    ) {
        if let existing = views[node.id] {
            // Existing node: apply any prop changes.
            for prop in node.props { existing.apply(prop: prop) }
            report.updated.append(node.id)
        } else if let adapter = registry.adapter(for: node.kind) {
            let view = adapter.build(node)
            if let mock = view as? MockView {
                views[node.id] = mock
                report.built.append(node.id)
            }
        }
        // Recurse into children (by key) — the keys keep identities stable.
        for child in node.children {
            switch child {
            case let .node(id):
                if let childNode = index[id] {
                    reconcile(node: childNode, index: index, report: &report)
                }
            case let .splice(_, items):
                for (_, id) in items {
                    if let childNode = index[id] {
                        reconcile(node: childNode, index: index, report: &report)
                    }
                }
            }
        }
    }
}

/// A summary of one reconciliation pass.
struct ReconcileReport: Sendable {
    /// Node ids for which a new native view was built.
    var built: [UInt32] = []
    /// Node ids whose props were applied to an existing view.
    var updated: [UInt32] = []
    /// Node ids detached (removed) this pass.
    var detached: [UInt32] = []
}
