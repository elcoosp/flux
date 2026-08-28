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

    /// The async string interner that publishes derived strings to the dev server
    /// (brittleness 4c). Wired from the executor at startup so a prop-thunk that
    /// builds `tapped \(count) times` receives a canonical id the kit can look up.
    /// Defaults to `NoOpStringInterner` for offline reconciliation.
    private var interner: any AnyStringInterner = NoOpStringInterner()

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

    /// Replaces the string interner used when materialising prop thunks
    /// (brittleness 4c). Called by the executor once the live transport exists.
    mutating func setInterner(_ interner: any AnyStringInterner){
        self.interner = interner
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
            reconcile(nodeId: root.id, nodes: frame.nodes, report: &report)
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
            reconcile(nodeId: newRootId, nodes: patchNodes, report: &report)
        }
        for patch in frame.patches {
            applyPatch(patch, nodes: patchNodes, report: &report)
        }
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
    private mutating func reconcile(nodeId: UInt32, nodes: [UInt32: ShadowNode], report: inout ReconcileReport) {
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
            if node.kind == .component {
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
        _ = reconcileDirty(nodeId: rootId, signalIds: signalIds, nodes: nodeTable, report: &report)
        return report
    }

    /// Recursive worker for `reconcileDirty`. Returns whether this subtree contains
    /// a dirty node, so ancestors know to re-attach their children.
    private mutating func reconcileDirty(
        nodeId: UInt32,
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
                let newKit = kitProps(effectiveProps, table: currentTable())
                owner.adapter.update(owner.view, from: oldKit, to: newKit)
                owner.runtimeProps = effectiveProps
                owner.lastPropHash = propHash(effectiveProps)
                report.updated.append(nodeId)
            }
            // Re-parent children so a dirty descendant lands in this view. For a
            // Router, navigation (signal 97) changes which single child is active,
            // so `collectChildViews` must be re-run to re-apply `routerActiveChildId`
            // (which filters to exactly the active Screen and builds it). Re-attaching
            // the blanket `childViews` collected above would keep showing the
            // originally-built child and navigation would "do nothing".
            let views: [AnyObject]
            // Detect a router by the resolved adapter name (the server lowers it
            // as a `component` with `componentId="Router"`, so the wire `NodeKind`
            // is `.component`, not `.router`). This mirrors Android's
            // `adapter?.kind == ROUTER_KIND` check and must not rely on the
            // build-time `owner.isRouter` flag, which can be stale if
            // `componentNames` was populated after the node was first built.
            if node.kind == .router || componentNames[node.componentId] == "Router" {
                views = collectChildViews(of: node, nodes: nodes, report: &report)
                #if DEBUG
                NSLog("[FluxRT] reconcileDirty router: collected \(views.count) views for node \(nodeId)")
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
            for cid in childIds {
                guard activeChildId == nil || cid == activeChildId else { continue }
                reconcile(nodeId: cid, nodes: nodes, report: &report)
                if let v = built[cid]?.view { views.append(v) }
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
        if let runtime = executorRef as? FluxRuntime,
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
        guard let bytecode = thunkBlobs[Data(thunk.hash)],
              let runtime = executorRef as? FluxRuntime else {
            #if DEBUG
            NSLog("[materialize] node \(nodeId) thunk present but bytecode missing (blobs=\(thunkBlobs.count), hash=\(Data(thunk.hash).map { String(format: "%02x", $0) }.joined()))")
            #endif
            return fallbackProps
        }
        #if DEBUG
        NSLog("[materialize] node \(nodeId) RUNNING thunk hash=\(Data(thunk.hash).map { String(format: "%02x", $0) }.joined()) bcLen=\(bytecode.count) deps=\(meta.deps) layout=\(meta.layout)")
        #endif
        do {
            // The thunk only reads signals; run it against a copy of the live
            // graph so materialisation never mutates graph state. Derived strings
            // (e.g. `STR_CONCAT` results) are interned through the dev server's
            // canonical string table via `interner` (brittleness 4c) — no local
            // synthetic id is ever minted.
            var store = runtime.graph
            let outcome = try FluxBytecodeVM.run(
                bytecode,
                signals: &store,
                payload: .null,
                stringTable: currentTable()
            )
            guard case let .record(fields) = outcome.registers[1] else {
                #if DEBUG
                NSLog("[materialize] node \(nodeId) thunk result r1 not a record: \(outcome.registers[1])")
                #endif
                return fallbackProps
            }
            var props: [Prop] = []
            props.reserveCapacity(meta.layout.count)
            for (position, propIdx) in meta.layout.enumerated() {
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
