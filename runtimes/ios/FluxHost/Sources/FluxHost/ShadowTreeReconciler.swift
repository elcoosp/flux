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
import UIKit
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
    /// Whether this node is a `Router` (resolved by the registered adapter name
    /// "Router" — the server lowers a router as a *component* with
    /// `componentId="Router"`, so matching the wire `NodeKind` misses it; Android
    /// detects a router the same way, via the adapter's kind string). Drives the
    /// navigation-signal (97) subscription and the active-child re-filter.
    var isRouter: Bool

    init(adapter: AnyFluxAdapter, view: AnyObject, runtimeProps: [Prop], lastPropHash: UInt64 = 0, isRouter: Bool = false) {
        self.adapter = adapter
        self.view = view
        self.runtimeProps = runtimeProps
        self.lastPropHash = lastPropHash
        self.isRouter = isRouter
    }
}

/// Drives native UIKit views from a stream of `ShadowNode` trees using an
/// `AdapterRegistry` and the real adapters from `FluxUIKit`.
@MainActor
struct ShadowTreeReconciler {
    private let registry: AdapterRegistry
    /// The host coordinator adapters dispatch events back into.
    private weak var executorRef: FluxExecutor?
    /// Built views keyed by node id. Persists across frames so identities are
    /// stable and view state survives updates/pushes/pops.
    private var built: [UInt32: BuiltNode]
    /// The most recent full node table, used to resolve a removed node's
    /// `cleanupHandler` (§18.4) at removal time.
    private var nodeTable: [UInt32: ShadowNode] = [:]
    /// Cloned `ForEach` row `ShadowNode`s (keyed by derived id), populated during
    /// expansion so `emitSubtree`/`reconcileDirty` see them like any other node.
    private var expandedNodeTable: [UInt32: ShadowNode] = [:]
    /// Per-node signal dependencies recorded from the node's props (R1). A prop
    /// whose value is `.int(s)` is treated as a read of signal `s`, so a write to
    /// `s` marks the node dirty. Populated during reconcile and consulted by
    /// `reconcileDirty`.
    private var signalDeps: [UInt32: Set<UInt32>] = [:]

    /// Reverse map from a prop-thunk's stable handler id to the node that owns
    /// it, so a state-preserving `Patch::Handler` updating the thunk body can
    /// re-materialise the node's dynamic props immediately (FR hot-reload).
    /// Keyed by the server-assigned handler id — stable across edits — NOT the
    /// thunk's content hash, which changes whenever the body changes.
    private var thunkHandlerToNode: [UInt32: UInt32] = [:]

    /// The node id of the root view in the currently applied tree. Tracked so a
    /// Delta frame (which may ship a `Replace` of the root when node ids are not
    /// stable across edits) can rebuild and re-present the root view instead of
    /// tearing it down and leaving a blank screen (hot-reload blank-screen bug).
    private var currentRootId: UInt32?

    /// The interning string table for this reconciler. Kept in sync from every
    /// applied frame's `strings` so prop resolution never depends on reaching
    /// back into the executor. A reference type (ADR-0027 T14) so the VM can
    /// intern thunk-derived strings (STR_CONCAT results) into the same instance
    /// the kit resolves from.
    private var table: MaterializationStringTable = MaterializationStringTable()

    /// Per-node signal-graph metadata (ADR-0027 §T13/T14), captured from the
    /// most recently applied frame. Drives thunk materialisation on reconcile.
    private var signalMeta: [UInt32: NodeSignalMeta] = [:]
    /// Component-id → adapter-name bindings (Appendix D §D.9), captured from the
    /// most recently applied frame. Kept separate from the string resolver so a
    /// component id never collides with a prop string id.
    private var componentNames: [UInt32: String] = [:]
    /// Lookup of prop-thunk bytecode by closure hash (the 8-byte BLAKE3), sliced
    /// from the frame's shared handler blob. Populated on every applied frame so
    /// a dirty node can re-run its thunk against the live signal graph.
    private var thunkBlobs: [Data: [UInt8]] = [:]
    /// Per-row ForEach item context: every derived node id inside an expanded
    /// ForEach row maps to the (itemSlot, element) that row was seeded with.
    /// Used to re-seed the shared `itemSlot` with the tapped row's element
    /// before dispatching a handler that captures `task` (the ForEach loop var).
    /// Without this, all rows share the last seeded value and `remove(task)`
    /// always removes the last element (FLUX-072 remove bug).
    private var forEachRowContext: [UInt32: (slot: UInt32, element: FluxValue)] = [:]

    /// Creates a reconciler bound to `registry` and `executor`.
    /// `table` is the shared string table (same instance as the executor's)
    /// so strings interned at dispatch time resolve at materialize time.
    init(registry: AdapterRegistry, executor: FluxExecutor? = nil, table: MaterializationStringTable) {
        self.registry = registry
        self.executorRef = executor
        self.table = table
        self.built = [:]
    }

    /// Points the reconciler (and every adapter it builds) at the host
    /// coordinator, so native controls can dispatch handlers without retaining
    /// the runtime.
    mutating func setExecutor(_ executor: FluxExecutor?) {
        executorRef = executor
    }

    /// Reconciles a freshly decoded frame against the current view set.
    /// - Returns: the ids of views that were built, updated, or detached.
    @discardableResult
    mutating func apply(_ frame: FluxFrame) -> ReconcileReport {
        var report = ReconcileReport()
        table.seed(frame.strings)
        // Cache the ADR-0027 signal metadata + thunk bytecode so subsequent
        // dirty reconciliations can re-materialise dynamic props without the
        // full frame.
        signalMeta = frame.signalMeta
        // Merge the frame's component-name table into the retained map rather
        // than replacing it. A Delta (patch) frame carries an *empty*
        // `componentNames`, so a straight assignment would wipe the table the
        // Init frame populated — and the next reconcile could no longer resolve a
        // primitive's `componentId` to its adapter name (e.g. "Column" →
        // `UIStackView`), falling back to an empty `ContainerAdapter` and leaving
        // a blank screen on hot-reload (FLUX-019). Init seeds it; every later
        // frame only adds/refreshes entries.
        for entry in frame.componentNames {
            componentNames[entry.stringId] = entry.value
        }
        var blobs: [Data: [UInt8]] = [:]
        for handler in frame.handlers {
            if let bytecode = handler.bytecode {
                blobs[Data(handler.closure.hash)] = bytecode
            }
        }
        thunkBlobs = blobs
        // Join the frame's handler ids (stable across edits) to each node's
        // prop-thunk (identified by its content hash) so a state-preserving
        // `Handler` patch can find the node to re-materialise. The delta frame
        // carries both `handlers` (id → hash + bytecode) and `signalMeta`
        // (node → thunk hash), which we connect here.
        var handlerHashToId: [Data: UInt32] = [:]
        for handler in frame.handlers {
            handlerHashToId[Data(handler.closure.hash)] = handler.handlerId
        }
        var map: [UInt32: UInt32] = [:]
        for (nid, meta) in frame.signalMeta where meta.thunk != nil {
            if let hid = handlerHashToId[Data(meta.thunk!.hash)] {
                map[hid] = nid
            }
        }
        // Merge (don't replace): a delta frame carries its own handlers/signalMeta
        // and must update the reverse map, but a delta that only ships a
        // `Handler` patch (no structural change) has an empty `signalMeta`, so
        // replacing here would wipe the map built by the Init frame and the
        // `applyPatch(.handler)` re-materialize would never find its node.
        for (hid, nid) in map {
            thunkHandlerToNode[hid] = nid
        }
        // A Delta frame carries an EMPTY `nodes` map (only patches). The
        // reconciler's authoritative table is `nodeTable`, retained from the
        // last full frame. Patch targets (`.replace`/`.insert`) carry their new
        // node inline, so merge those over `nodeTable` and hand the merged map to
        // `applyPatch` (and to the Delta root reconcile below) — otherwise
        // `reconcile` looks the node up in an empty map and silently drops the
        // structural patch (blank screen on hot-reload). `nodeTable` is only
        // updated by a full frame, so a Delta never clobbers it.
        var patchNodes = nodeTable
        for patch in frame.patches {
            switch patch {
            case let .replace(_, node), let .insert(_, _, node), let .reattach(_, _, node):
                patchNodes[node.id] = node
            default:
                break
            }
        }
        // Keep the latest full node table so removal can resolve a node's
        // `cleanupHandler` (§18.4) even from a patch frame that only lists deltas.
        // Only a full-tree frame (root != nil) carries the authoritative table;
        // a patch frame has an empty `nodes` and must not clobber it.
        if let root = frame.root {
            nodeTable = frame.nodes
            currentRootId = root.id
            reconcile(nodeId: root.id, parentId: 0, nodes: frame.nodes, report: &report)
        } else if let rootId = currentRootId {
            // A Delta frame carries an EMPTY `nodes` map (only patches). Node ids
            // are not stable across edits in the current pipeline (they derive
            // from byte-accurate source spans), so a text edit shifts every id
            // and the differ emits a `Replace` of the whole subtree rather than a
            // minimal `.handler` patch. The differ's `emit_replace` puts the
            // *new* node's id in `Patch::Replace.id`, so the old root id can no
            // longer be found in `built`/`patchNodes` after application. We
            // therefore retarget `currentRootId` to the new root: it is the
            // replaced/inserted node id that is not referenced as a child of any
            // other node in `patchNodes`. Then reconcile from that id so the new
            // root view is built and presented. Without this the old root is
            // torn down, `currentRootId` keeps pointing at the destroyed id, and
            // the screen goes blank (hot-reload blank-screen bug).
            var candidateRoots: Set<UInt32> = []
            var childIds: Set<UInt32> = []
            for patch in frame.patches {
                switch patch {
                case let .replace(_, node), let .insert(_, _, node):
                    candidateRoots.insert(node.id)
                    for child in node.children {
                        if case let .node(cid) = child { childIds.insert(cid) }
                    }
                default:
                    break
                }
            }
            // The new root is a replaced/inserted node that is itself not a child
            // of another replaced/inserted node.
            let newRootId = candidateRoots.subtracting(childIds).first ?? rootId
            // The node ids are not stable across edits (they derive from
            // byte-accurate source spans), so a text edit shifts every id and the
            // differ emits a `Replace` of the *whole* subtree. The incremental
            // reconcile would then try to reuse stale `built` entries keyed by the
            // old (now-gone) child ids and produce empty view shells — blank
            // screen on hot-reload. When the root is replaced we therefore rebuild
            // the entire tree fresh from `patchNodes` (the same path the Init
            // frame takes, which renders correctly), then adopt `patchNodes` as
            // the new authoritative table. This is a full rebuild rather than a
            // surgical patch, which is acceptable for the cold hot-reload case
            // where every id changed.
            built.removeAll()
            nodeTable = patchNodes
            currentRootId = newRootId
            reconcile(nodeId: newRootId, parentId: 0, nodes: patchNodes, report: &report)
        }
        for patch in frame.patches {
            applyPatch(patch, nodes: patchNodes, report: &report)
        }
        // DevTools: after the tree is (re)built, replay the full hierarchy so a
        // connected debugger reflects the current node graph even if it attached
        // after the initial mount (snapshot-on-connect, FLUX-039).
        #if DEBUG
        if fluxDevtoolsSink != nil {
            emitSnapshot()
        }
        #endif
        return report
    }

    /// The currently built native view for `nodeId`, if any (for test assertions).
    func view(for nodeId: UInt32)->AnyObject?{
        built[nodeId]?.view
    }

    /// The native view of the currently applied root, resolved from the
    /// reconciler's own `currentRootId` (kept correct across both full and Delta
    /// frames). The executor's `rootView` delegates here so a Delta that replaces
    /// the root (node ids unstable across edits) still presents the new root
    /// instead of reading a stale, destroyed id and going blank.
    var rootView: AnyObject? {
        guard let id = currentRootId else { return nil }
        return built[id]?.view
    }

    /// Reconciles the node `nodeId` (resolved from `nodes`) against the built
    /// set, recursing into its children, then handing the child views to the
    /// parent adapter.
    private mutating func reconcile(nodeId: UInt32, parentId: UInt32 = 0, nodes: [UInt32: ShadowNode], report: inout ReconcileReport) {
        guard let node = nodes[nodeId] else { return }
        // Materialise dynamic props (ADR-0027 T14): for a node with a prop thunk
        // this runs the thunk against the live signal graph; for static nodes it
        // returns the shipped props unchanged. Reused everywhere below so the
        // signal deps, kit props, and stored runtime props stay consistent.
        let effectiveProps = materializeProps(for: nodeId, fallbackProps: node.props)
        // Record which signals this node's props read, so a later signal write can
        // mark it dirty without walking the whole tree (R1). A dynamic node uses
        // its thunk's captured `deps`; a static node reads whatever signal refs
        // survive in its shipped props.
        let metaDeps = signalMeta[nodeId]?.deps ?? []
        var deps = Set(metaDeps).union(effectiveProps.compactMap { $0.value.asInt }.compactMap { UInt32(exactly: $0) })
        // A `Router` node must re-reconcile whenever its navigation target
        // changes, so it subscribes to the `Router.navigate` signal (97, ADR-0045).
        // The server lowers a router as a *component* with `componentId="Router"`
        // (the same way Android does — `adapter?.kind == "router"`), so we detect
        // it by the resolved adapter name, not the wire `NodeKind`, which would
        // miss it and leave navigation dead.
        let isRouter = node.kind == .router || componentNames[node.componentId] == "Router"
        if isRouter {
            deps.insert(Self.navigationRouteSignalId)
        }
        signalDeps[nodeId] = deps
        if let existing = built[nodeId] {
            // A `@pure` node whose props' content hash is unchanged depends on
            // nothing else, so its entire subtree is stable: skip re-reconciling
            // it (G6). We still update the recorded hash below.
            let newHash = propHash(effectiveProps)
            if node.isPure, existing.lastPropHash == newHash {
                return
            }
            // Only push an `update` when the props actually changed (R2). When the
            // hash matches we skip `adapter.update` entirely — the view already
            // reflects these props — but still recurse into children below, since
            // a descendant may have changed via a patch.
            if existing.lastPropHash != newHash {
                let oldKit = kitProps(existing.runtimeProps, table: currentTable())
                let newKit = kitProps(effectiveProps, table: currentTable())
                existing.adapter.update(existing.view, from: oldKit, to: newKit)
                existing.runtimeProps = effectiveProps
                existing.lastPropHash = newHash
                report.updated.append(nodeId)
                // DevTools: report the updated node with its live layout frame
                // (the view is a real `UIView`, so geometry is available here).
                #if DEBUG
                let uiv = existing.view as? UIView
                let rect = uiv.map {
                    Rect(
                        x: Double($0.frame.origin.x),
                        y: Double($0.frame.origin.y),
                        width: Double($0.frame.size.width),
                        height: Double($0.frame.size.height),
                    )
                }
                fluxDevtoolsEmit(.viewMutation(nodeId: nodeId, nativeViewId: UInt64(nodeId), parentId: parentId, mutationKind: 0, frame: rect, componentName: componentNames[node.componentId] ?? "?"))
                #endif
            }
        } else {
            // A `Component` node (Appendix C) is a host-side container: it has no
            // primitive adapter of its own, so it hosts its children in a plain
            // `UIView`. Its `componentId` is the interned component name, which
            // collides with primitive ids on the wire, so we must branch on `kind`
            // rather than resolving `componentId` through the registry. Primitives
            // resolve through the registry by id; any still-unbound id degrades to
            // a container rather than crashing (B8).
            let adapter: AnyFluxAdapter
            if signalMeta[nodeId]?.itemSlot != nil {
                // A `ForEach` node hosts its expanded rows: it must lay them out
                // vertically (like a `Column`) so the rows get intrinsic height
                // and become visible. A plain `ContainerAdapter` is a zero-height
                // `UIView` that clips its children and hides the whole list
                // (FLUX-072 #3).
                adapter = AnyFluxAdapter(ColumnAdapter(executor: executorRef))
            } else if node.kind == .component {
                adapter = AnyFluxAdapter(ContainerAdapter(executor: executorRef))
            } else if let name = componentNames[node.componentId],
                      let prim = registry.make(named: name, executor: executorRef) {
                adapter = prim
            } else {
                adapter = AnyFluxAdapter(ContainerAdapter(executor: executorRef))
            }
            let kit = kitProps(effectiveProps, table: currentTable())
            let view = adapter.create()
            adapter.update(view, from: Props(), to: kit)
            let hash = propHash(effectiveProps)
            built[nodeId] = BuiltNode(adapter: adapter, view: view, runtimeProps: effectiveProps, lastPropHash: hash, isRouter: isRouter)
            report.built.append(nodeId)
            // DevTools: report the newly-built node so the component tree shows
            // the live node graph. Geometry (frame) is unavailable until the view
            // is laid out, so it is sent on the first update instead.
            #if DEBUG
            fluxDevtoolsEmit(.viewMutation(nodeId: nodeId, nativeViewId: UInt64(nodeId), parentId: parentId, mutationKind: 0, frame: nil, componentName: componentNames[node.componentId] ?? "?"))
            #endif
            // Bind handlers once, at build time — re-binding on every frame
            // would stack UIControl actions (ButtonAdapter adds one per call).
            for handlerId in node.handlers {
                adapter.bindHandler(handlerId, to: view, nodeId: nodeId)
            }
            // Run the node's `onMount` block exactly once, on first build (G5).
            if let mount = node.mountHandler {
                (executorRef as? FluxExecutor)?.runLifecycle(mount)
            }
        }

        // Build/refresh children first, then hand them to the parent adapter.
        let childViews = collectChildViews(of: node, nodes: nodes, report: &report)
        if let owner = built[nodeId] {
            owner.adapter.setChildren(childViews, on: owner.view)
        }
    }

    /// Replays the currently-built shadow tree to DevTools as `viewMutation(add)`
    /// events, so a freshly-connected debugger shows the full hierarchy without
    /// waiting for the next mount or interaction. Walks `nodeTable` from the root,
    /// emitting each node with its true parent id and live layout frame.
    ///
    /// Non-mutating: it only reads the built views and node table and emits
    /// telemetry; it never creates or destroys views.
    func emitSnapshot() {
        #if DEBUG
        guard let root = currentRootId else { return }
        emitSubtree(nodeId: root, parentId: 0, nodes: nodeTable)
        #endif
    }

    /// Recursive helper for `emitSnapshot`: emits `nodeId` with `parentId`, then
    /// descends into the node's declared children.
    private func emitSubtree(nodeId: UInt32, parentId: UInt32, nodes: [UInt32: ShadowNode]) {
        #if DEBUG
        guard let node = nodes[nodeId] else { return }
        let rect = (built[nodeId]?.view as? UIView).map {
            Rect(
                x: Double($0.frame.origin.x),
                y: Double($0.frame.origin.y),
                width: Double($0.frame.size.width),
                height: Double($0.frame.size.height),
            )
        }
        fluxDevtoolsEmit(
            .viewMutation(
                nodeId: nodeId,
                nativeViewId: UInt64(nodeId),
                parentId: parentId,
                mutationKind: 0,
                frame: rect,
                componentName: componentNames[node.componentId] ?? "?",
            )
        )
        for child in node.children {
            let childIds: [UInt32]
            switch child {
            case let .node(id):
                childIds = [id]
            case let .splice(_, items):
                childIds = items.map { $0.node }
            }
            for cid in childIds {
                emitSubtree(nodeId: cid, parentId: nodeId, nodes: nodes)
            }
        }
        #endif
    }

    /// Re-reconciles only the nodes whose recorded signal dependencies intersect
    /// `signalIds` — the signals a handler just wrote — plus their ancestors, so a
    /// changed view is re-parented into its parent (Perf R1). This replaces the
    /// per-dispatch whole-tree re-walk: on a tap only the signal-dependent
    /// subtrees are touched.
    ///
    /// The walk uses the reconciler's own authoritative `nodeTable` (seeded from
    /// every applied frame), NOT a caller-passed copy — mirroring the Android host,
    /// which always reconciles against its own `nodes` map. Passing a separate
    /// `currentNodes` copy from the executor desynced from the built views and
    /// left the router node unreachable, so navigation never re-attached the
    /// active screen.
    ///
    /// A node whose own dependencies changed is always re-applied (even if its raw
    /// prop bytes are unchanged, because the signal(s) behind them may carry new
    /// values). A node that is merely an ancestor of a dirty descendant only
    /// re-attaches its children; a fully clean subtree is never visited.
    @discardableResult
    mutating func reconcileDirty(rootId: UInt32, signalIds: Set<UInt32>) -> ReconcileReport {
        var report = ReconcileReport()
        _ = reconcileDirty(nodeId: rootId, parentId: 0, signalIds: signalIds, nodes: nodeTable, report: &report)
        return report
    }

    /// Recursive worker for `reconcileDirty`. Returns whether this subtree contains
    /// a dirty node, so ancestors know to re-attach their children.
    private mutating func reconcileDirty(
        nodeId: UInt32,
        parentId: UInt32 = 0,
        signalIds: Set<UInt32>,
        nodes: [UInt32: ShadowNode],
        report: inout ReconcileReport
    )->Bool{
        guard let node = nodes[nodeId] else { return false }
        let deps = signalDeps[nodeId, default: []]
        let isDirty = !deps.intersection(signalIds).isEmpty

        // Materialise dynamic props (ADR-0027 T14) so a signal write re-evaluates
        // interpolations against the new graph value. For static nodes this is the
        // shipped props unchanged.
        let effectiveProps = materializeProps(for: nodeId, fallbackProps: node.props)

        // Visit children first to find dirty descendants and collect their views.
        // A `ForEach` node's `children` are a *template* splice, not real rows: the
        // rows are expanded lazily by `collectChildViews` from the live list signal.
        // So for a ForEach we must NOT recurse into the template here (that would
        // build a stray template view) — the expansion below handles the real rows.
        var childViews: [AnyObject] = []
        var anyChildDirty = false
        if signalMeta[nodeId]?.itemSlot != nil,
           let itemSlot = signalMeta[nodeId]?.itemSlot,
           let executor = executorRef {
            // A `ForEach` node must be re-expanded from the live list signal on
            // every dirty pass (e.g. when `tasks.push` writes signal 3): the rows
            // are real views, not the template splice. Seed each row's `itemSlot`
            // and reconcile it synchronously so the new/changed rows appear
            // (FLUX-072 #1 — "add task doesn't render"). Without this the list
            // never re-expands and appended tasks are invisible.
            var templateChildIds: [UInt32] = []
            for child in node.children {
                switch child {
                case let .node(id): templateChildIds.append(id)
                case let .splice(_, items): templateChildIds.append(contentsOf: items.map { $0.node })
                }
            }
            #if DEBUG
            NSLog("[FluxRT] ForEach node \(nodeId): expanding \(templateChildIds.count) template children")
            #endif
            let (ids, expanded, elements) = expandForEach(nodeId: nodeId, templateChildIds: templateChildIds, nodes: nodes)
            #if DEBUG
            NSLog("[FluxRT] ForEach node \(nodeId): expanded to \(ids.count) rows, \(expanded.count) nodes, \(elements.count) elements")
            #endif
            expandedNodeTable.merge(expanded) { $1 }
            let mergedNodes = nodes.merging(expanded) { $1 }
            for (rowId, element) in zip(ids, elements) {
                executor.seedSignal(itemSlot, element)
                let rowDirty = reconcileDirty(nodeId: rowId, parentId: node.id, signalIds: signalIds, nodes: mergedNodes, report: &report)
                anyChildDirty = anyChildDirty || rowDirty
                if let v = built[rowId]?.view { childViews.append(v) }
            }
        } else {
            for child in node.children {
                let childIds: [UInt32]
                switch child {
                case let .node(id):
                    childIds = [id]
                case let .splice(_, items):
                    childIds = items.map { $0.node }
                }
                for cid in childIds {
                    let childDirty = reconcileDirty(nodeId: cid, parentId: node.id, signalIds: signalIds, nodes: nodes, report: &report)
                    anyChildDirty = anyChildDirty || childDirty
                    if let v = built[cid]?.view { childViews.append(v) }
                }
            }
        }

        let affected = isDirty || anyChildDirty
        guard affected else { return false }

        if let owner = built[nodeId] {
            // Re-materialize a node whose own dependencies changed (R1).
            if isDirty {
                let oldKit = kitProps(owner.runtimeProps, table: currentTable())
                let newKit = kitProps(effectiveProps, table: currentTable())
                owner.adapter.update(owner.view, from: oldKit, to: newKit)
                owner.runtimeProps = effectiveProps
                owner.lastPropHash = propHash(effectiveProps)
                report.updated.append(nodeId)
                // DevTools: report the updated node with its live layout frame.
                #if DEBUG
                let uiv = owner.view as? UIView
                let rect = uiv.map {
                    Rect(
                        x: Double($0.frame.origin.x),
                        y: Double($0.frame.origin.y),
                        width: Double($0.frame.size.width),
                        height: Double($0.frame.size.height)
                    )
                }
                fluxDevtoolsEmit(.viewMutation(nodeId: nodeId, nativeViewId: UInt64(nodeId), parentId: parentId, mutationKind: 0, frame: rect, componentName: componentNames[node.componentId] ?? "?"))
                #endif
            }
            // Re-parent children so a dirty descendant lands in this view. For a
            // Router, navigation (signal 97) changes which single child is active,
            // so `collectChildViews` must be re-run to re-apply `routerActiveChildId`
            // (which filters to exactly the active Screen and builds it). For a
            // `ForEach` (itemSlot != nil) the same re-run re-expands the rows from
            // the live list signal — without this the rows never appear after an
            // `append`/`remove`/clear and the list stays blank (FLUX-072 / ADR-0050).
            let views: [AnyObject]
            if node.kind == .router || componentNames[node.componentId] == "Router" || signalMeta[nodeId]?.itemSlot != nil {
                views = collectChildViews(of: node, nodes: nodes, report: &report)
                #if DEBUG
                NSLog("[FluxRT] reconcileDirty router/foreach: collected \(views.count) views for node \(nodeId)")
                #endif
            } else {
                views = childViews
            }
            owner.adapter.setChildren(views, on: owner.view)
        } else if isDirty {
            // A dirty node that was never built (shouldn't happen on dispatch, but
            // be safe): fall back to a full reconcile of this subtree.
            reconcile(nodeId: nodeId, nodes: nodes, report: &report)
        }
        return true
    }

    /// Expands a `ForEach` node into one cloned row per list element (FLUX-072 / ADR-0050).
    /// Returns the derived child ids plus a map of cloned `ShadowNode`s (so the
    /// recursive `reconcile` finds them). For each element it seeds the row's
    /// `item` signal slot (from `signalMeta[id].itemSlot`) with `list[i]` in the
    /// live graph, so the template row's thunk materialises the right value.
    private mutating func expandForEach(
        nodeId: UInt32,
        templateChildIds: [UInt32],
        nodes: [UInt32: ShadowNode]
    ) -> (childIds: [UInt32], expanded: [UInt32: ShadowNode], elements: [FluxValue]) {
        guard let meta = signalMeta[nodeId],
              let itemSlot = meta.itemSlot,
              let listSignal = meta.deps.first,
              let executor = executorRef else { return (templateChildIds, [:], []) }
        let raw = executor.graph.read(listSignal)
        guard case let .list(items) = raw else { return (templateChildIds, [:], []) }
        guard let templateId = templateChildIds.first,
              nodes[templateId] != nil else { return (templateChildIds, [:], []) }
        var childIds: [UInt32] = []
        var expanded: [UInt32: ShadowNode] = [:]
        var elements: [FluxValue] = []
        for (_, element) in items.enumerated() {
            let rowId = deriveForEachRowId(foreachId: nodeId, index: UInt32(childIds.count))
            // The id the caller will reconcile is the *cloned template* node
            // (the inlined row), not the row key: `cloneSubtree` stores the
            // template clone under `deriveForEachChildId(rowId, templateId)`
            // (high bits 0xC0…), whereas `rowId` alone (0x80…) is never a key in
            // `expanded`. Returning `rowId` made `reconcile(rowId)` look up a
            // missing node and silently build nothing — the blank-list bug
            // (FLUX-072 #4).
            let derivedTemplateId = deriveForEachChildId(rowId: rowId, origId: templateId)
            // Deep-clone the template row (incl. any component body inlined at
            // the call site) under per-row derived ids, so each row is a distinct
            // subtree holding its own value instead of sharing the template's
            // signal. The original `itemSlot` is seeded per row by the caller
            // (collectChildViews) right before each row is reconciled.
            var subtree: [UInt32: ShadowNode] = [:]
            var subtreeMeta: [UInt32: NodeSignalMeta] = [:]
            cloneSubtree(templateId, rowId: rowId, nodes: nodes, into: &subtree, meta: &subtreeMeta)
            expanded.merge(subtree) { $1 }
            signalMeta.merge(subtreeMeta) { $1 }
            // Track per-row element for handler dispatch: every derived node
            // inside this row maps to the same (slot, element) so a tap on any
            // child (e.g. Button's derived id) can re-seed `itemSlot` with the
            // correct row value before the VM reads it. Without this the shared
            // slot holds the last row's value and `remove(task)` removes the
            // wrong task.
            for derivedId in subtree.keys {
                forEachRowContext[derivedId] = (itemSlot, element)
            }
            childIds.append(derivedTemplateId)
            elements.append(element)
        }
        return (childIds, expanded, elements)
    }

    /// Derives a stable per-row id from the `ForEach` id + element index.
    private func deriveForEachRowId(foreachId: UInt32, index: UInt32) -> UInt32 {
        // FNV-ish mix; deterministic and collision-resistant for practical trees.
        var h: UInt32 = foreachId &* 0x0100_0193
        h = h ^ index
        h = h &* 0x0100_0193
        return h | 0x8000_0000 // high bit marks expanded rows
    }

    /// Derives a stable per-row id for a *child* of an expanded `ForEach` row,
    /// from the row id + the child's original template id. This keeps every
    /// row's subtree (incl. a component body inlined at the call site) a distinct
    /// set of node ids, so each row renders its own per-row `itemSlot` value
    /// instead of the shared template's signal (the multi-row ForEach bug,
    /// FLUX-072 / ADR-0050).
    private func deriveForEachChildId(rowId: UInt32, origId: UInt32) -> UInt32 {
        var h: UInt32 = rowId &* 0x0100_0193
        h = h ^ origId
        h = h &* 0x0100_0193
        h = h ^ 0x5555_5555
        return h | 0xC000_0000 // distinct high bits mark expanded row children
    }

    /// Deep-clones the subtree rooted at `origId` into `expanded`, rewriting each
    /// node's id to a per-row derived id and copying its `signalMeta` so the
    /// cloned prop thunks resolve against the live graph. The cloned node ids are
    /// unique per row, which is what lets multiple `ForEach` rows hold distinct
    /// values instead of sharing one signal.
    private func cloneSubtree(
        _ origId: UInt32,
        rowId: UInt32,
        nodes: [UInt32: ShadowNode],
        into expanded: inout [UInt32: ShadowNode],
        meta: inout [UInt32: NodeSignalMeta]
    ) {
        guard let orig = nodes[origId] else { return }
        let newId = deriveForEachChildId(rowId: rowId, origId: origId)
        guard expanded[newId] == nil else { return }
        var childRefs: [Child] = []
        for child in orig.children {
            switch child {
            case let .node(cid):
                cloneSubtree(cid, rowId: rowId, nodes: nodes, into: &expanded, meta: &meta)
                childRefs.append(.node(deriveForEachChildId(rowId: rowId, origId: cid)))
            case let .splice(count, items):
                let newItems = items.map { (key: $0.key, node: deriveForEachChildId(rowId: rowId, origId: $0.node)) }
                for it in items {
                    cloneSubtree(it.node, rowId: rowId, nodes: nodes, into: &expanded, meta: &meta)
                }
                childRefs.append(.splice(itemCount: count, items: newItems))
            }
        }
        let clone = ShadowNode(
            id: newId,
            kind: orig.kind,
            componentId: orig.componentId,
            props: orig.props,
            childCount: orig.childCount,
            children: childRefs,
            handlerCount: orig.handlerCount,
            handlers: orig.handlers,
            span: orig.span,
            mountHandler: orig.mountHandler,
            cleanupHandler: orig.cleanupHandler,
            isPure: orig.isPure
        )
        expanded[newId] = clone
        if let m = signalMeta[origId] { meta[newId] = m }
    }

    /// Returns the ForEach row context for a derived node id (the (itemSlot,
    /// element) that row was seeded with), or nil when the node is not inside
    /// an expanded ForEach row. Used to re-seed the shared `itemSlot` before
    /// dispatching a handler that captures the loop var.
    func itemContext(for nodeId: UInt32) -> (slot: UInt32, element: FluxValue)? {
        forEachRowContext[nodeId]
    }

    /// Builds (or refreshes) the children of `node` and returns their views,
    /// in declared order.
    private mutating func collectChildViews(of node: ShadowNode, nodes: [UInt32: ShadowNode], report: inout ReconcileReport) -> [AnyObject] {
        var views: [AnyObject] = []
        // A `Router` presents only the active-route `Screen` (ADR-0045): it must
        // not build/reconcile the hidden sibling screens, so we scope the walk to
        // the single active child id returned by `routerActiveChildId`.
        let activeChildId = (node.kind == .router || componentNames[node.componentId] == "Router") ? routerActiveChildId(node, nodes: nodes) : nil
        for child in node.children {
            let childIds: [UInt32]
            switch child {
            case let .node(id):
                childIds = [id]
            case let .splice(_, items):
                childIds = items.map { $0.node }
            }
            let expandedChildIds: [UInt32]
            let mergedNodes: [UInt32: ShadowNode]
            if signalMeta[node.id]?.itemSlot != nil,
               let itemSlot = signalMeta[node.id]?.itemSlot,
               let executor = executorRef {
                // `expandForEach` deep-clones each template row (incl. any component
                // body inlined at the call site) under per-row derived ids and
                // returns the matching list elements. We must seed each row's
                // `itemSlot` with its own element and reconcile that row
                // *synchronously* — interleaving seed+build per row is what keeps
                // every row bound to its own value. A single shared itemSlot
                // seeded once for all rows would leave every row showing the last
                // element (the multi-row ForEach bug, FLUX-072 / ADR-0050).
                let (ids, expanded, elements) = expandForEach(nodeId: node.id, templateChildIds: childIds, nodes: nodes)
                expandedChildIds = ids
                mergedNodes = nodes.merging(expanded) { $1 }
                expandedNodeTable.merge(expanded) { $1 }
                for (rowId, element) in zip(ids, elements) {
                    guard activeChildId == nil || rowId == activeChildId else { continue }
                    executor.seedSignal(itemSlot, element)
                    reconcile(nodeId: rowId, parentId: node.id, nodes: mergedNodes, report: &report)
                    if let v = built[rowId]?.view { views.append(v) }
                }
            } else {
                expandedChildIds = childIds
                mergedNodes = nodes
                for cid in expandedChildIds {
                    guard activeChildId == nil || cid == activeChildId else { continue }
                    reconcile(nodeId: cid, parentId: node.id, nodes: mergedNodes, report: &report)
                    if let v = built[cid]?.view { views.append(v) }
                }
            }
        }
        return views
    }

    /// Resolves the string table used to resolve prop strings. The reconciler
    /// keeps its own table, synced from every applied frame, so prop resolution
    /// never depends on reaching back into the executor.
    private func currentTable()->MaterializationStringTable{
        table
    }

    /// FNV-1a (32-bit) hash of [name], truncated to `UInt16` — matches the wire
    /// encoder's `prop_index_for_name` (Appendix C), so a `route` prop decoded
    /// from the server resolves to the same index here.
    private static let routePropIndex: UInt16 = {
        var h: UInt32 = 0x811c_9dc5
        for b in "route".utf8 {
            h = (h ^ UInt32(b)) &* 0x0100_0193
        }
        return UInt16(truncatingIfNeeded: h)
    }()

    /// The signal id `Router.navigate` writes its target to (ADR-0045).
    private static let navigationRouteSignalId: UInt32 = 97

    /// For a `Router` node, returns the id of the single `Screen` child whose
    /// `route` prop equals the active navigation target (read from signal 97).
    /// When the signal is unset, malformed, or no screen matches, returns the
    /// first child so the stack always shows a screen (mirrors Android).
    private func routerActiveChildId(_ node: ShadowNode, nodes: [UInt32: ShadowNode]) -> UInt32? {
        // The server lowers a router as a *component* (`componentId="Router"`),
        // so its wire `NodeKind` is `.component`, not `.router`. Accept either,
        // matching Android's `adapter?.kind == ROUTER_KIND` detection.
        let isRouter = node.kind == .router || componentNames[node.componentId] == "Router"
        guard isRouter else { return nil }
        var activeRoute: String?
        if let runtime = executorRef as? FluxExecutor,
           let record = runtime.graph.read(Self.navigationRouteSignalId) {
            // `Router.navigate(target)` writes the VM's CALL_CAP `args` register to
            // signal 97. The iOS compiler lowers `Router.navigate("x")` to
            // `LOAD_STR_CONST` + `CALL_CAP`, so `args` is a RAW `.str(targetId)`
            // (not a wrapped record). Accept both a raw `.str` and a `.record`
            // whose first field is a `.str`, so navigation swaps the visible screen
            // on a real tap rather than always showing the first child.
            let routeId: UInt32?
            switch record {
            case let .str(id):
                routeId = id
            case let .record(fields):
                routeId = fields.first.flatMap { field -> UInt32? in
                    if case let .str(id) = field.value { id } else { nil }
                }
            default:
                routeId = nil
            }
            // The route string id written by `Router.navigate` resolves through the
            // reconciler's frame-seeded string table (the same table the screen's
            // route prop is resolved from, below), so navigation swaps the visible
            // screen on a real tap. If it is nil the signal is unset/malformed and we
            // fall back to the first child.
            if let rid = routeId, let route = currentTable().lookup(rid) {
                activeRoute = route
            }
        }
        #if DEBUG
        NSLog("[FluxRT] routerActiveChildId: kind=\(node.kind) comp=\(node.componentId) isRouter=\(isRouter) active=\(activeRoute ?? "nil")")
        #endif
        var firstChildId: UInt32?
        var matchedChild: UInt32?
        for child in node.children {
            let childIds: [UInt32]
            switch child {
            case let .node(id): childIds = [id]
            case let .splice(_, items): childIds = items.map { $0.node }
            }
            for cid in childIds {
                if firstChildId == nil { firstChildId = cid }
                guard let childNode = nodes[cid] else { continue }
                // A `Screen` is lowered by the server as a *component*
                // (`componentId="Screen"`), so its wire `NodeKind` is `.component`,
                // not `.screen`. Detect it by the resolved adapter name, matching
                // how Android finds screens (and how the router is detected above),
                // otherwise every screen child is skipped and navigation silently
                // falls back to the first child (no swap on tap).
                let isScreen = childNode.kind == .screen || componentNames[childNode.componentId] == "Screen"
                guard isScreen else { continue }
                guard let prop = childNode.props.first(where: { $0.index == Self.routePropIndex }),
                      case let .str(id) = prop.value,
                      let route = currentTable().lookup(id) else { continue }
                // When signal 97 is unset (initial render) fall back to the first
                // screen; when set, prefer the route match. Either way pick a
                // screen so the router never blanks (mirrors Android).
                if activeRoute == nil {
                    if matchedChild == nil { matchedChild = cid }
                } else if route == activeRoute {
                    if matchedChild == nil { matchedChild = cid }
                }
            }
        }
        return matchedChild ?? firstChildId
    }

    /// Materialises the props of `node` by running its ADR-0027 prop thunk
    /// against the live signal graph, falling back to the shipped static props
    /// when the node has no thunk or the thunk cannot be evaluated.
    ///
    /// For a dynamic node the lowering compiled every prop expression into a
    /// single closure whose result `Record` (in `r1`) holds each prop value at
    /// its field position; `meta.layout` maps that position to the on-wire
    /// `PropIdx`. A node with no thunk (pure literals / control-only) returns its
    /// shipped props unchanged.
    private func materializeProps(for nodeId: UInt32, fallbackProps: [Prop]) -> [Prop] {
        guard let meta = signalMeta[nodeId], let thunk = meta.thunk else {
            #if DEBUG
            NSLog("[materialize] node \(nodeId) no thunk (meta=\(signalMeta[nodeId] != nil)) -> shipped \(fallbackProps.count) props")
            #endif
            return fallbackProps
        }
        // Skip thunk evaluation if any dependency signal is unseeded (null).
        // This happens for template children of a ForEach: their thunks
        // reference the ForEach's itemSlot signal, which is only seeded
        // per-row during ForEach expansion (which happens after this node's
        // props are materialized). Running the thunk here would read null →
        // nullDereference crash. The per-row reconcile (with seeded itemSlot)
        // will materialize these props correctly.
        let hasUnseededDeps = meta.deps.contains { dep in
            guard let value = executorRef?.graph.read(dep) else {
                return true // unseeded signal
            }
            if case .null = value {
                return true // explicitly null
            }
            return false
        }
        if hasUnseededDeps {
            #if DEBUG
            NSLog("[materialize] node \(nodeId) thunk deps unseeded (deps=\(meta.deps)) -> shipped \(fallbackProps.count) props")
            #endif
            return fallbackProps
        }
        guard let bytecode = thunkBlobs[Data(thunk.hash)],
              let runtime = executorRef as? FluxExecutor else {
            #if DEBUG
            NSLog("[materialize] node \(nodeId) thunk present but bytecode missing (blobs=\(thunkBlobs.count), hash=\(Data(thunk.hash).map { String(format: "%02x", $0) }.joined()))")
            #endif
            return fallbackProps
        }
        do {
            // The thunk only reads signals; run it against a copy of the live
            // graph so materialisation never mutates graph state. Derived strings
            // (e.g. `STR_CONCAT` results) are interned synchronously into the
            // shared `MaterializationStringTable` (brittleness 4c) — no server
            // round-trip for local strings.
            var store = runtime.graph
            let outcome = try FluxBytecodeVM.run(
                bytecode,
                signals: &store,
                payload: .null,
                stringTable: currentTable()
            )
            #if DEBUG
            let regs = (0..<16).map { FluxExecutor.describe(outcome.registers[$0], table: currentTable()) }
                .enumerated().map { "r\($0.offset)=\($0.element)" }
            NSLog("[FluxRT] thunk HALT node %d regs: %@", nodeId, regs.joined(separator: " "))
            #endif
            guard case let .record(fields) = outcome.registers[1] else {
                #if DEBUG
                NSLog("[materialize] node \(nodeId) thunk result r1 not a record: \(outcome.registers[1])")
                #endif
                return fallbackProps
            }
            var props: [Prop] = []
            props.reserveCapacity(meta.layout.count)
            for (position, propIdx) in meta.layout.enumerated() {
                // The thunk's result `Record` is positional: field `position`
                // holds the value that must be stored under `meta.layout[position]`
                // (the canonical `PropIdx`). So read by position, not by key
                // (Appendix C / ADR-0027 prop-thunk contract). User *data* records
                // (e.g. an appended `Task`) are canonical-keyed, but they are read
                // by the thunk via `GET_FIELD` (which looks up by canonical key),
                // never positionally here.
                guard position < fields.count else { break }
                props.append(Prop(index: propIdx, value: fields[position].value))
            }
            #if DEBUG
            let resolved = props.map { p -> String in
                if case let .str(id) = p.value { return currentTable().lookup(id) ?? "UNRESOLVED(\(id))" }
                return "\(p.value)"
            }
            NSLog("[materialize] node \(nodeId) OK resolved=\(resolved)")
            #endif
            return props
        } catch {
            #if DEBUG
            NSLog("[materialize] node \(nodeId) thunk THREW: \(error)")
            #endif
            // Materialisation is best-effort: degrade to shipped props rather
            // than leaving the view stale or crashing the reconcile.
            return fallbackProps
        }
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
                (executorRef as? FluxExecutor)?.runLifecycle(cleanup)
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

        case let .reattach(old, new, node):
            // Phase 3 state preservation: re-key the live instance from `old` to
            // `new` WITHOUT destroying and rebuilding it, so its signal state,
            // refs, scroll position and text input survive a structural edit
            // (e.g. `Column` → `Row`, or a re-spanned subtree) that changed the
            // node's id but not its component identity.
            guard let existing = built.removeValue(forKey: old) else {
                // No live instance to preserve (first appearance as `new`):
                // build it fresh rather than going blank.
                reconcile(nodeId: new, nodes: nodes, report: &report)
                return
            }
            // Preserve the built view + adapter (state lives in the native view
            // and the adapter's per-node closure table). Only the key changes.
            let preserved = existing
            built[new] = preserved
            nodeTable[new] = node
            // Keep the retained signal-deps map coherent for dirty walks.
            if let deps = signalDeps[old] { signalDeps[new] = deps }
            // Re-materialise props against the new node shape and push the delta
            // to the (preserved) native view.
            let newProps = materializeProps(for: new, fallbackProps: node.props)
            let oldKit = kitProps(preserved.runtimeProps, table: currentTable())
            let newKit = kitProps(newProps, table: currentTable())
            preserved.adapter.update(preserved.view, from: oldKit, to: newKit)
            preserved.runtimeProps = newProps
            preserved.lastPropHash = propHash(newProps)
            preserved.isRouter = node.kind == .router || componentNames[node.componentId] == "Router"
            built[new] = preserved
            report.updated.append(new)
            // Re-parent children from the preserved node so a nested dirty child
            // lands correctly; handler bindings are retained (handler ids are
            // stable across the reattach).
            let childViews = collectChildViews(of: node, nodes: nodes, report: &report)
            if let owner = built[new] {
                owner.adapter.setChildren(childViews, on: owner.view)
            }

        case let .handler(id, _):
            // A state-preserving handler swap. `apply` already re-registered the
            // new bytecode via `frame.handlers`, so the executor's closure table
            // is current. If this handler is a node's prop thunk, re-materialise
            // that node's dynamic props so a thunk-body edit (e.g. a changed
            // string literal) takes effect immediately, without waiting for a
            // signal change (FR hot-reload). Non-thunk handlers (e.g. `onClick`)
            // need no view mutation here.
            guard let nodeId = thunkHandlerToNode[id], let existing = built[nodeId] else { return }
            let newProps = materializeProps(for: nodeId, fallbackProps: existing.runtimeProps)
            let oldKit = kitProps(existing.runtimeProps, table: currentTable())
            let newKit = kitProps(newProps, table: currentTable())
            existing.adapter.update(existing.view, from: oldKit, to: newKit)
            existing.runtimeProps = newProps
            report.updated.append(nodeId)
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
