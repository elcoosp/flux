//  ShadowTreeReconciler.swift
//  Keyed reconciler (FLUX-006 scope item 5) over the deserialized shadow tree,
//  now driving the real `FluxUIKit` adapter kit (FLUX-016).
//
//  On each frame the reconciler walks the node table against the set of
//  already-built native views. Nodes are matched by their stable `NodeId`
//  (Appendix C §C.1 / D §D.3) and `Splice` children by their `u64` keys (D §D.4),
//  so existing native views are mutated in place (prop `update`) rather than
//  recreated. Only genuinely new nodes trigger `adapter.create()`; only removed
//  nodes trigger `adapter.destroy()`. This is what lets the runtime assert view
//  identity is preserved across updates, pushes and pops.

import Foundation
import FluxUIKit

/// A node whose native view has been built and is being kept alive.
@MainActor
final class BuiltNode {
    /// The adapter that built and owns `view` (retains per-node state).
    let adapter: AnyFluxAdapter
    /// The native UIKit view or view controller.
    let view: AnyObject
    /// The most recent runtime props, so subsequent updates can compute a diff.
    var runtimeProps: [Prop]

    init(adapter: AnyFluxAdapter, view: AnyObject, runtimeProps: [Prop]) {
        self.adapter = adapter
        self.view = view
        self.runtimeProps = runtimeProps
    }
}

/// Drives native UIKit views from a stream of `ShadowNode` trees using an
/// `AdapterRegistry` and the real adapters from `FluxUIKit`.
@MainActor
struct ShadowTreeReconciler {
    private let registry: AdapterRegistry
    /// The host coordinator adapters dispatch events back into.
    private weak var executorRef: (any FluxExecutor)?
    /// Built views keyed by node id. Persists across frames so identities are
    /// stable and view state survives updates/pushes/pops.
    private var built: [UInt32: BuiltNode]

    /// The interning string table for this reconciler. Kept in sync from every
    /// applied frame's `strings` so prop resolution never depends on reaching
    /// back into the executor.
    private var table: StringTable = StringTable()

    /// Creates a reconciler bound to `registry` and `executor`.
    init(registry: AdapterRegistry, executor: (any FluxExecutor)? = nil) {
        self.registry = registry
        self.executorRef = executor
        self.built = [:]
    }

    /// Points the reconciler (and every adapter it builds) at the host
    /// coordinator, so native controls can dispatch handlers without retaining
    /// the runtime.
    mutating func setExecutor(_ executor: (any FluxExecutor)?) {
        executorRef = executor
    }

    /// Reconciles a freshly decoded frame against the current view set.
    /// - Returns: the ids of views that were built, updated, or detached.
    @discardableResult
    mutating func apply(_ frame: FluxFrame) -> ReconcileReport {
        var report = ReconcileReport()
        for str in frame.strings { table.intern(str.stringId, str.value) }
        if let root = frame.root {
            reconcile(nodeId: root.id, nodes: frame.nodes, report: &report)
        }
        for patch in frame.patches {
            applyPatch(patch, nodes: frame.nodes, report: &report)
        }
        return report
    }

    /// The currently built native view for `nodeId`, if any (for test assertions).
    func view(for nodeId: UInt32) -> AnyObject? {
        built[nodeId]?.view
    }

    /// Reconciles the node `nodeId` (resolved from `nodes`) against the built
    /// set, recursing into its children, then handing the child views to the
    /// parent adapter.
    private mutating func reconcile(nodeId: UInt32, nodes: [UInt32: ShadowNode], report: inout ReconcileReport) {
        guard let node = nodes[nodeId] else { return }

        if let existing = built[nodeId] {
            // Existing node: apply any prop changes in place (no recreation).
            let oldKit = kitProps(existing.runtimeProps, table: currentTable())
            let newKit = kitProps(node.props, table: currentTable())
            existing.adapter.update(existing.view, from: oldKit, to: newKit)
            existing.runtimeProps = node.props
            report.updated.append(nodeId)
        } else if let adapter = registry.make(for: node.componentId, executor: executorRef) {
            let view = adapter.create()
            let kit = kitProps(node.props, table: currentTable())
            adapter.update(view, from: Props(), to: kit)
            built[nodeId] = BuiltNode(adapter: adapter, view: view, runtimeProps: node.props)
            report.built.append(nodeId)
            // Bind handlers once, at build time — re-binding on every frame
            // would stack UIControl actions (ButtonAdapter adds one per call).
            for handlerId in node.handlers {
                adapter.bindHandler(handlerId, to: view, nodeId: nodeId)
            }
        }

        // Build/refresh children first, then hand them to the parent adapter.
        let childViews = collectChildViews(of: node, nodes: nodes, report: &report)
        if let owner = built[nodeId] {
            owner.adapter.setChildren(childViews, on: owner.view)
        }
    }

    /// Builds (or refreshes) the children of `node` and returns their views,
    /// in declared order.
    private mutating func collectChildViews(of node: ShadowNode, nodes: [UInt32: ShadowNode], report: inout ReconcileReport) -> [AnyObject] {
        var views: [AnyObject] = []
        for child in node.children {
            let childIds: [UInt32]
            switch child {
            case let .node(id):
                childIds = [id]
            case let .splice(_, items):
                childIds = items.map { $0.node }
            }
            for cid in childIds {
                reconcile(nodeId: cid, nodes: nodes, report: &report)
                if let v = built[cid]?.view { views.append(v) }
            }
        }
        return views
    }

    /// Resolves the string table used to resolve prop strings. The reconciler
    /// keeps its own table, synced from every applied frame, so prop resolution
    /// never depends on reaching back into the executor.
    private func currentTable() -> StringTable {
        table
    }

    /// Applies a single patch to the built views.
    private mutating func applyPatch(_ patch: Patch, nodes: [UInt32: ShadowNode], report: inout ReconcileReport) {
        switch patch {
        case let .update(id, changes, removals):
            guard let existing = built[id] else { return }
            var props = existing.runtimeProps
            for removal in removals { props.removeAll { $0.index == removal } }
            for change in changes { props.removeAll { $0.index == change.index }; props.append(change) }
            let oldKit = kitProps(existing.runtimeProps, table: currentTable())
            let newKit = kitProps(props, table: currentTable())
            existing.adapter.update(existing.view, from: oldKit, to: newKit)
            existing.runtimeProps = props
            report.updated.append(id)

        case let .remove(id):
            if let existing = built.removeValue(forKey: id) {
                existing.adapter.destroy(existing.view)
                report.detached.append(id)
            }

        case let .replace(id, node):
            if let existing = built.removeValue(forKey: id) {
                existing.adapter.destroy(existing.view)
            }
            reconcile(nodeId: node.id, nodes: nodes, report: &report)

        case let .insert(_, _, node):
            reconcile(nodeId: node.id, nodes: nodes, report: &report)

        case let .reorder(parentId, keys):
            guard let parent = built[parentId] else { return }
            let views = keys.compactMap { built[$0]?.view }
            parent.adapter.setChildren(views, on: parent.view)

        case let .handler(id, _):
            // Handler bytecode is stored by the executor; the node id is (re)bound
            // when its view is built. Nothing to mutate on existing views here.
            _ = id
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
