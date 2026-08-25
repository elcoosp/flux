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
    /// Content hash of `runtimeProps` from the last reconciliation, used by the
    /// `@pure` subtree skip (G6) to detect unchanged props without re-walking
    /// the children.
    var lastPropHash: UInt64

    init(adapter: AnyFluxAdapter, view: AnyObject, runtimeProps: [Prop], lastPropHash: UInt64 = 0) {
        self.adapter = adapter
        self.view = view
        self.runtimeProps = runtimeProps
        self.lastPropHash = lastPropHash
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
    /// The most recent full node table, used to resolve a removed node's
    /// `cleanupHandler` (§18.4) at removal time.
    private var nodeTable: [UInt32: ShadowNode] = [:]
    /// Per-node signal dependencies recorded from the node's props (R1). A prop
    /// whose value is `.int(s)` is treated as a read of signal `s`, so a write to
    /// `s` marks the node dirty. Populated during reconcile and consulted by
    /// `reconcileDirty`.
    private var signalDeps: [UInt32: Set<UInt32>] = [:]

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
        // Keep the latest full node table so removal can resolve a node's
        // `cleanupHandler` (§18.4) even from a patch frame that only lists deltas.
        // Only a full-tree frame (root != nil) carries the authoritative table;
        // a patch frame has an empty `nodes` and must not clobber it.
        if let root = frame.root {
            nodeTable = frame.nodes
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
        // Record which signals this node's props read, so a later signal write can
        // mark it dirty without walking the whole tree (R1). A prop whose value is a
        // signal reference (`.int(s)`) is treated as a read of signal `s`.
        signalDeps[nodeId] = Set(node.props.compactMap { $0.value.asInt }.compactMap { UInt32(exactly: $0) })
        if let existing = built[nodeId] {
            // A `@pure` node whose props' content hash is unchanged depends on
            // nothing else, so its entire subtree is stable: skip re-reconciling
            // it (G6). We still update the recorded hash below.
            let newHash = propHash(node.props)
            if node.isPure, existing.lastPropHash == newHash {
                return
            }
            // Only push an `update` when the props actually changed (R2). When the
            // hash matches we skip `adapter.update` entirely — the view already
            // reflects these props — but still recurse into children below, since
            // a descendant may have changed via a patch.
            if existing.lastPropHash != newHash {
                let oldKit = kitProps(existing.runtimeProps, table: currentTable())
                let newKit = kitProps(node.props, table: currentTable())
                existing.adapter.update(existing.view, from: oldKit, to: newKit)
                existing.runtimeProps = node.props
                existing.lastPropHash = newHash
                report.updated.append(nodeId)
            }
        } else if let adapter = registry.make(for: node.componentId, executor: executorRef) {
            let kit = kitProps(node.props, table: currentTable())
            let view = adapter.create()
            adapter.update(view, from: Props(), to: kit)
            let hash = propHash(node.props)
            built[nodeId] = BuiltNode(adapter: adapter, view: view, runtimeProps: node.props, lastPropHash: hash)
            report.built.append(nodeId)
            // Bind handlers once, at build time — re-binding on every frame
            // would stack UIControl actions (ButtonAdapter adds one per call).
            for handlerId in node.handlers {
                adapter.bindHandler(handlerId, to: view, nodeId: nodeId)
            }
            // Run the node's `onMount` block exactly once, on first build (G5).
            if let mount = node.mountHandler {
                (executorRef as? FluxRuntime)?.runLifecycle(mount)
            }
        }

        // Build/refresh children first, then hand them to the parent adapter.
        let childViews = collectChildViews(of: node, nodes: nodes, report: &report)
        if let owner = built[nodeId] {
            owner.adapter.setChildren(childViews, on: owner.view)
        }
    }

    /// Re-reconciles only the nodes whose recorded signal dependencies intersect
    /// `signalIds` — the signals a handler just wrote — plus their ancestors, so a
    /// changed view is re-parented into its parent (Perf R1). This replaces the
    /// per-dispatch whole-tree re-walk: on a tap only the signal-dependent
    /// subtrees are touched.
    ///
    /// A node whose own dependencies changed is always re-applied (even if its raw
    /// prop bytes are unchanged, because the signal(s) behind them may carry new
    /// values). A node that is merely an ancestor of a dirty descendant only
    /// re-attaches its children; a fully clean subtree is never visited.
    @discardableResult
    mutating func reconcileDirty(rootId: UInt32, nodes: [UInt32: ShadowNode], signalIds: Set<UInt32>) -> ReconcileReport {
        var report = ReconcileReport()
        _ = reconcileDirty(nodeId: rootId, signalIds: signalIds, nodes: nodes, report: &report)
        return report
    }

    /// Recursive worker for `reconcileDirty`. Returns whether this subtree contains
    /// a dirty node, so ancestors know to re-attach their children.
    private mutating func reconcileDirty(
        nodeId: UInt32,
        signalIds: Set<UInt32>,
        nodes: [UInt32: ShadowNode],
        report: inout ReconcileReport
    ) -> Bool {
        guard let node = nodes[nodeId] else { return false }
        let deps = signalDeps[nodeId, default: []]
        let isDirty = !deps.intersection(signalIds).isEmpty

        // Visit children first to find dirty descendants and collect their views.
        var childViews: [AnyObject] = []
        var anyChildDirty = false
        for child in node.children {
            let childIds: [UInt32]
            switch child {
            case let .node(id):
                childIds = [id]
            case let .splice(_, items):
                childIds = items.map { $0.node }
            }
            for cid in childIds {
                let childDirty = reconcileDirty(nodeId: cid, signalIds: signalIds, nodes: nodes, report: &report)
                anyChildDirty = anyChildDirty || childDirty
                if let v = built[cid]?.view { childViews.append(v) }
            }
        }

        let affected = isDirty || anyChildDirty
        guard affected else { return false }

        if let owner = built[nodeId] {
            // Re-materialize a node whose own dependencies changed (R1).
            if isDirty {
                let oldKit = kitProps(owner.runtimeProps, table: currentTable())
                let newKit = kitProps(node.props, table: currentTable())
                owner.adapter.update(owner.view, from: oldKit, to: newKit)
                owner.runtimeProps = node.props
                owner.lastPropHash = propHash(node.props)
                report.updated.append(nodeId)
            }
            // Re-parent children so a dirty descendant lands in this view.
            owner.adapter.setChildren(childViews, on: owner.view)
        } else if isDirty {
            // A dirty node that was never built (shouldn't happen on dispatch, but
            // be safe): fall back to a full reconcile of this subtree.
            reconcile(nodeId: nodeId, nodes: nodes, report: &report)
        }
        return true
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
            // Run the node's `onCleanup` block (§18.4) before tearing down its
            // native view, so resources it acquired in `onMount` are released.
            if let node = nodeTable[id], let cleanup = node.cleanupHandler {
                (executorRef as? FluxRuntime)?.runLifecycle(cleanup)
            }
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
