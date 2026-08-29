package dev.flux.host.shadow

import dev.flux.host.AdapterRegistry
import dev.flux.host.BuildFlags
import dev.flux.host.StringTableEntry
import dev.flux.host.vm.FluxBytecodeVM
import dev.flux.host.vm.FluxValue
import dev.flux.host.vm.StringResolver
import dev.flux.host.vm.VmResult
import dev.flux.host.wire.ClosureRef
import dev.flux.host.wire.FluxFrame
import dev.flux.host.vm.debug.TelemetryBridge
import dev.flux.host.vm.debug.TelemetryEvent
import dev.flux.host.wire.NodeSignalMeta
import dev.flux.host.wire.Patch
import dev.flux.host.wire.PropDiff
import dev.flux.host.wire.WireChild
import dev.flux.host.wire.WireNode
import dev.flux.host.wire.toKitValue
import dev.flux.ui.FluxAdapter
import dev.flux.ui.FluxExecutor
import dev.flux.ui.FluxNativeView
import dev.flux.ui.FluxUiKit
import dev.flux.ui.Props
import java.lang.ref.WeakReference
import dev.flux.host.FluxExecutor as HostExecutor

/**
 * Stable map key for a closure content hash.
 *
 * A Flux `ClosureRef.hash` is an 8-byte BLAKE3 digest carried as a Kotlin
 * `ByteArray`. `ByteArray` has identity-based `hashCode()`/`equals()` on the
 * JVM, so using it directly as a `HashMap` key misses on equal-content-but-
 * distinct instances (the signal-meta thunk and the handler table carry the
 * same hash bytes but as separate `ByteArray`s). We fold the eight bytes into a
 * single `ULong`, which has value-based equality, mirroring how iOS keys thunk
 * bytecode by `Data` (content-hashed). Both the insert (handler table) and the
 * lookup (signal-meta thunk) go through this, so the hash is interpreted
 * identically on both sides.
 */
private fun thunkKey(hash: ByteArray): ULong {
    var acc = 0uL
    for (b in hash) acc = (acc shl 8) or (b.toULong() and 0xFFu)
    return acc
}

/**
 * The host render tree: a map of [ShadowNode]s keyed by id, plus the adapter
 * registry that translates IR nodes into native views.
 *
 * The shadow tree is the source of truth for *structure* (the dev server owns
 * the IR; the host owns the signal graph — ADR-0002). Patches from the wire (or
 * a full-tree Init frame) mutate it through [applyFrame]; the reconciler then
 * drives the adapters so the native view subtree matches.
 *
 * Resolution goes through the [AdapterRegistry], which maps an interned
 * `ComponentId` (carried on every wire node) to a dev adapter from the
 * `adapters/ui-kotlin` kit (FLUX-017).
 *
 * **Dirty-set reconcile (ADR-0027 Phase 1 + R1).** A handler dispatch writes a
 * set of signals; only the nodes whose prop expressions read those signals may
 * change. [reconcileDirty] descends *just* the subtrees containing a dirty node
 * — a clean sibling is never visited (independent of tree size) — and
 * re-materializes old/new prop kits only for nodes that actually change
 * (T5/R2). The view-identity guarantee (EndToEndTest) holds: nodes that are not
 * built/updated keep their exact `FluxNativeView` instance.
 *
 * @property registry the adapter registry, keyed by `ComponentId`. Mutated in
 *   place as string-table deltas arrive so the tree always resolves against the
 *   latest `Init` frame.
 */
public class ShadowTree(
    internal var registry: AdapterRegistry,
) {
    internal val nodes = LinkedHashMap<UInt, ShadowNode>()
    internal val parents = LinkedHashMap<UInt, UInt>()
    internal var root: ShadowNode? = null
    internal var executorRef: FluxExecutor? = null

    // Resolves interned string ids (wire `StrVal`) to their text. Rebuilt from
    // each frame's string table in `applyFrame` so `kitFromWire` can materialize
    // real strings instead of raw ids (Appendix D §D.9). Kept in a persistent
    // map so a Delta frame can *merge* its changed strings into the strings
    // already shipped by earlier frames, rather than dropping them.
    internal var stringLookupTable: MutableMap<UInt, String> = HashMap()
    internal var stringLookup: (UInt) -> String? = { stringLookupTable[it] }

    /**
     * The `kind` tag the wire assigns a `Router` node (Appendix F.6). Tests and
     * the adapter registry use the same string, so it lives in one place.
     */
    internal val ROUTER_KIND: String = "router"

    /**
     * The signal `Router.navigate(target)` writes its target into (ADR-0045).
     * The registry stores the whole argument record there; this tree reads the
     * record's first field (the route string id) to decide which `Screen` shows.
     */
    internal val NAVIGATION_ROUTE_SIGNAL_ID: UInt = 97u

    // How many times each node's view has been reconciled (built or updated).
    // Used by the `@pure` skip (§18.10) and observable in tests.
    internal val reconciled = LinkedHashMap<UInt, Int>()

    // Per-node signal dependencies (R1): signal ids whose int-valued props this
    // node reads. Built during build/apply; consulted on every dispatch.
    internal val signalDeps = LinkedHashMap<UInt, MutableSet<UInt>>()

    // ADR-0027 (FA-IRWIRE) per-node signal metadata, keyed by node id, captured
    // from the most recently applied frame so dirty reconciles can re-run thunks.
    internal var signalMeta: Map<UInt, NodeSignalMeta> = emptyMap()

    // Lookup of prop-thunk bytecode by closure hash, sliced from the frame's
    // shared handler blob. Populated on every applied frame so a dirty node can
    // re-materialise its dynamic props against the live signal graph.
    internal var thunkBlobs: Map<ULong, ByteArray> = emptyMap()

    // Reverse map from a prop-thunk's stable handler id to the node that owns it,
    // so a state-preserving Handler patch updating the thunk body can
    // re-materialise the node's dynamic props immediately (FR hot-reload). Keyed
    // by the server-assigned handler id — stable across edits — NOT the thunk's
    // content hash, which changes whenever the body changes.
    internal var thunkHandlerToNode: MutableMap<UInt, UInt> = mutableMapOf()

    // Cumulative reconcile counters (reconcile-counters-and-budgets.md).
    internal var builtCount = 0u
    internal var updatedCount = 0u
    internal var skippedUnchangedCount = 0u
    internal var skippedPureCount = 0u
    internal var detachedCount = 0u
    internal var propMaterializations = 0u

    // Monotonic script-step counter, incremented once per apply/dispatch step so
    // each `step_end` trace event carries a unique `i` (reconcile-trace-format.md).
    internal var stepCount = 0u

    // The frame sequence number of the most recent apply (for trace events).
    internal var lastSeq = 0u

    /**
     * Trace sink (INV-2): when non-null, every reconcile step emits a
     * [TraceEvent] line. Production leaves this `null` so the hot path allocates
     * nothing and pays no serialization cost.
     */
    public var trace: ((TraceEvent) -> Unit)? = null

    /**
     * Creates the `MutableState` backing each node's [ShadowNode.props]. Defaults
     * to a plain `mutableStateOf`; the app overrides it (it is the same object
     * the renderer reads, so Compose observes in-place prop mutations).
     */
    public var propsStateFactory: (Props) -> androidx.compose.runtime.MutableState<Props> =
        { androidx.compose.runtime.mutableStateOf(it) }

    /** The current root node, or `null` before an Init frame is applied. */
    public val rootNode: ShadowNode? get() = root

    /**
     * Emits [event] only under [BuildFlags.DEBUG] (brittleness 8d). A sink
     * attached in a release build is ignored, so R8 strips the call site from
     * release (INV-2: the hot path pays nothing in production).
     *
     * @param event the trace event to emit when tracing is active.
     */
    internal fun emitTrace(event: TraceEvent) {
        if (BuildFlags.DEBUG) trace?.invoke(event)
    }

    /** All nodes currently in the tree, in insertion order. */
    public fun allNodes(): List<ShadowNode> = nodes.values.toList()

    /**
     * How many times [id]'s view has been reconciled (built or updated). A
     * `@pure` node whose props are unchanged is never re-reconciled, so its
     * count stays put even as siblings change (§18.10).
     */
    public fun reconcileCount(id: UInt): Int = reconciled[id] ?: 0

    /** Signals [id] reads (R1), for trace/parity inspection. */
    public fun signalDependencies(id: UInt): Set<UInt> = signalDeps[id]?.toSet() ?: emptySet()

    /** The sequence number of the most recently applied frame (for trace events). */
    public fun lastSeq(): UInt = lastSeq

    /**
     * Applies a decoded [frame] to the tree, creating/updating/removing nodes
     * and reconciling children. Full-tree frames replace the root; delta frames
     * replay their patches. Returns the resulting root (when present).
     */
    public fun applyFrame(
        frame: FluxFrame,
        executor: FluxExecutor,
    ): ShadowNode? {
        executorRef = executor
        lastSeq = frame.seq
        if (frame.strings.isNotEmpty()) {
            // String literals live in a SEPARATE id space from `ComponentId`s
            // (a StringId and a ComponentId can share a numeric value, §D.9).
            // They feed only the string resolver — feeding them to the adapter
            // registry would overwrite a component-name binding at the same id
            // and break resolution (e.g. a literal at id 2 clobbering the
            // "Column" component, surfacing as
            // `no adapter registered for component 2`). A Delta frame carries
            // only its changed strings, so merge rather than replace to keep
            // strings from earlier frames resolvable.
            val merged = HashMap(stringLookupTable)
            for ((id, text) in frame.strings) merged[id] = text
            stringLookupTable = merged
            stringLookup = { id -> merged[id] }
        }
        // Appendix D §D.9: the Init frame's `component_names` section binds each
        // `ComponentId` to its adapter name. These are a SEPARATE id space from
        // the string literals in `frame.strings` and must not leak into the
        // string resolver (a ComponentId and a StringId can share a numeric
        // value). Feed them only to the registry, and only when the frame
        // actually carries them — a Delta without `componentNames` must keep the
        // registrations established by the Init frame, or every component loses
        // its adapter after the first edit.
        if (frame.componentNames.isNotEmpty()) {
            registry = registry.withEntries(frame.componentNames.map { StringTableEntry(it.id, it.text) })
        }
        // ADR-0027 (FA-IRWIRE): cache the per-node signal metadata and slice each
        // prop-thunk's bytecode from the shared handler blob so dirty reconciles
        // can re-materialise dynamic props without a full frame.
        // Hot-reload (Delta) frames only carry `signalMeta` when their flags set
        // FLAG_NODE_HAS_SIGNAL_DEPS; a Delta without it must NOT wipe the thunk
        // table, or interpolation breaks permanently after the first edit.
        signalMeta = if (!frame.fullTree && frame.signalMeta.isEmpty()) signalMeta else frame.signalMeta
        // Thunk bytecode is resolved by content hash from the frame's handler
        // table — every host slices the shared blob per-handler at the handler's
        // own `ClosureRef` offset, then keys the result by hash (parity contract,
        // Appendix F; mirrors `ShadowTreeReconciler.materializeProps` on iOS). We
        // deliberately do NOT slice by `signalMeta.thunk.bytecodeOffset`: that
        // would re-introduce offset-based resolution and diverge from iOS, which
        // ignores the offset entirely and looks up by hash. The prop-thunk lives
        // in the shared blob alongside the handlers, so it is reachable through
        // the same handler table keyed by its content hash.
        val blob = frame.bytecodeBlob
        val thunks = LinkedHashMap<ULong, ByteArray>()
        if (blob != null && blob.len > 0) {
            for (handler in frame.handlers) {
                val ref = handler.closure
                val start = ref.bytecodeOffset.toInt()
                val len = ref.bytecodeLen.toInt()
                val absStart = blob.offset + start
                if (start >= 0 && len > 0 && absStart + len <= blob.data.size) {
                    thunks[thunkKey(ref.hash)] = blob.data.copyOfRange(absStart, absStart + len)
                }
            }
        }
        thunkBlobs = thunks
        // Join the frame's handler ids (stable across edits) to each node's
        // prop-thunk (identified by its content hash) so a state-preserving
        // Handler patch can find the node to re-materialise. The delta frame
        // carries both `handlers` (id -> hash) and `signalMeta` (node -> thunk
        // hash), which we connect here.
        val handlerHashToId = LinkedHashMap<ULong, UInt>()
        for (h in frame.handlers) handlerHashToId[thunkKey(h.closure.hash)] = h.handlerId
        val map = mutableMapOf<UInt, UInt>()
        for ((nid, meta) in frame.signalMeta) {
            val thunk = meta.thunk ?: continue
            val hid = handlerHashToId[thunkKey(thunk.hash)] ?: continue
            map[hid] = nid
        }
        // Merge (don't replace): a Delta frame carries its own `handlers` +
        // `signalMeta` and must refresh the reverse map, but a Delta that only
        // ships a `Handler` patch (no structural change) has an empty
        // `signalMeta`, so a straight assignment would wipe the map the Init
        // frame built — and `applyPatch(.handler)` would then never find its
        // node to re-materialise (FR hot-reload; mirrors the iOS fix). Stale
        // entries pointing at old (destroyed) ids are harmless: `applyPatch`
        // checks `nodes[nodeId]` and no-ops when missing.
        for ((hid, nid) in map) thunkHandlerToNode[hid] = nid
        if (frame.fullTree && frame.root != null) {
            val index = LinkedHashMap<UInt, WireNode>()
            index[frame.root.id] = frame.root
            for (n in frame.extraNodes) index[n.id] = n
            emitTrace(
                TraceEvent.Frame(
                    seq = frame.seq,
                    full = true,
                    root = frame.root.id,
                    nodes = (1u + frame.extraNodes.size.toUInt()),
                    patches = 0u,
                ),
            )
            val built = build(frame.root, index, executor, depth = 0u)
            // T9: tear down the prior subtree before dropping it, so stale views
            // are released and no index entry survives (which would phantom-dirty).
            root?.let { destroySubtree(it) }
            root = built
            nodes.clear()
            parents.clear()
            collect(built)
            emitStepEnd()
            return built
        }
        emitTrace(
            TraceEvent.Frame(
                seq = frame.seq,
                full = false,
                root = null,
                nodes = 0u,
                patches = frame.patches.size.toUInt(),
            ),
        )
        // Node ids are not stable across edits (they derive from byte-accurate
        // source spans), so a text edit shifts every id and the differ emits a
        // `Replace` of the *whole* subtree rather than a minimal `.handler`
        // patch. Each such `Replace` carries its `WireNode` inline, but with
        // child *id references* (not inline subtrees), and a whole-tree replace
        // arrives as several `Replace` patches (root + every descendant). Build a
        // single merged index of every `Replace`/`Insert` node in this frame so a
        // replaced root can resolve its children through the index instead of a
        // one-node map (which dropped children and left empty shells — blank
        // screen on hot reload, FLUX-019). When the replaced id is the current
        // root, reassign `root` so the renderer mounts the new tree.
        val patchIndex = LinkedHashMap<UInt, WireNode>()
        for (patch in frame.patches) {
            when (patch.tag.toInt()) {
                0x01, 0x03 -> patch.node?.let { patchIndex[it.id] = it }
            }
        }
        // A source edit shifts every node id (ids derive from byte-accurate
        // spans), so the differ emits `Remove` patches for the entire old tree
        // followed by `Insert` patches for the new one. Applying those
        // sequentially tears the old root down first, so every `Insert` finds
        // its parent already removed and silently no-ops — the tree goes
        // empty/stale and the UI never reflects the edit (hot-reload
        // regression, FLUX-019). Detect that whole-tree pattern and rebuild
        // from the merged `patchIndex` in one pass instead, mirroring the
        // `Replace`-root path.
        val oldRootId = root?.id
        val oldRootRemoved =
            oldRootId != null &&
                frame.patches.any { it.tag.toInt() == 0x04 && it.id == oldRootId }
        val newRootInserted =
            patchIndex.keys.any { candidate ->
                patchIndex.values.none { childIdList(it).contains(candidate) }
            }
        if (oldRootRemoved && newRootInserted) {
            rebuildFromPatchIndex(patchIndex, executor)
            emitTrace(
                TraceEvent.Frame(
                    seq = frame.seq,
                    full = false,
                    root = root?.id,
                    nodes = nodes.size.toUInt(),
                    patches = frame.patches.size.toUInt(),
                ),
            )
            emitStepEnd()
            return root
        }
        if (frame.patches.isNotEmpty()) {
            for (patch in frame.patches) applyPatch(patch, patchIndex, executor)
            emitTrace(TraceEvent.ApplyPatch(seq = frame.seq, patches = frame.patches.size.toUInt()))
            emitStepEnd()
        }
        return root
    }

    /**
     * Rebuilds the entire tree from a merged index of `Replace`/`Insert` nodes
     * (the whole-subtree hot-reload pattern where node ids are unstable across
     * edits). Finds the new root — the node not referenced as a child of any
     * other node in the index — builds it and its descendants recursively
     * through the index, then reassigns [root] and refreshes the [nodes] and
     * [parents] maps. The old tree is destroyed first so no stale node survives
     * the hot reload.
     *
     * This is the `Insert`-based analogue of the `Replace`-root path: both
     * represent a total tree swap, and both must rebuild from the merged index
     * rather than applying `Remove`/`Insert` patches sequentially (which would
     * drop every inserted node once its parent was torn down).
     */
    private fun rebuildFromPatchIndex(
        patchIndex: Map<UInt, WireNode>,
        executor: FluxExecutor,
    ) {
        val childIds = patchIndex.values.flatMap { childIdList(it) }.toSet()
        val rootId = patchIndex.keys.firstOrNull { it !in childIds } ?: return
        root?.let { destroySubtree(it) }
        nodes.clear()
        parents.clear()
        val newRoot = build(patchIndex[rootId]!!, patchIndex, executor, depth = 0u)
        root = newRoot
        collect(newRoot)
    }

    private fun collect(node: ShadowNode) {
        nodes[node.id] = node
        for (child in node.children) {
            parents[child.id] = node.id
            collect(child)
        }
    }

    private fun applyPatch(
        patch: Patch,
        patchIndex: Map<UInt, WireNode>,
        executor: FluxExecutor,
    ) {
        executorRef = executor
        when (patch.tag.toInt()) {
            0x01 -> { // Replace
                val wire = patch.node ?: return
                // Whole-tree (root) replacement: node ids are unstable across
                // edits, so a text edit ships a `Replace` of the current root
                // id. Tear the old subtree down and reassign `root` so the
                // renderer mounts the freshly built tree (blank-screen-on-hot-
                // reload bug, FLUX-019). The merged `patchIndex` lets the new
                // root resolve its children (they arrive as sibling `Replace`
                // patches), unlike a one-node map which produced empty shells.
                if (patch.id == root?.id) {
                    root?.let { destroySubtree(it) }
                    val built = build(wire, patchIndex, executor, depth = 0u)
                    root = built
                    nodes.clear()
                    parents.clear()
                    collect(built)
                    return
                }
                val built = build(wire, patchIndex, executor, depth = 0u)
                val existing = nodes[patch.id]
                if (existing != null) {
                    val parentId = parents[patch.id]
                    parentId?.let { pid ->
                        val parent = nodes[pid] ?: return@let
                        val idx = parent.children.indexOfFirst { it.id == patch.id }
                        if (idx >= 0) {
                            destroySubtree(existing)
                            parent.children[idx] = built
                        }
                    }
                }
                built.children.forEach { parents[it.id] = built.id }
                nodes[patch.id] = built
                collect(built)
            }
            0x02 -> { // Update
                val node = nodes[patch.id] ?: return
                val diff = patch.diff ?: return
                val merged = mergeProps(node.wireProps, diff)
                // ADR-0027 (FA-IRWIRE): materialise dynamic props for the merged
                // field set before sending to the adapter.
                val newKit = materializeProps(merged.fields, patch.id)
                // `@pure` skip (§18.10): a pure node whose raw props are
                // referentially equal is a function of its props — nothing to do.
                // (No reconcile count: the node was not revisited — see G6.)
                if (node.isPure && merged.fields == node.wireProps.fields) {
                    skippedPureCount++
                    emitTrace(TraceEvent.SkipUnchanged(seq = lastSeq, id = patch.id))
                    return
                }
                // T5/R2: skip the adapter update when raw props AND the child-id
                // list are identical — no native mutation is required.
                if (merged.fields == node.wireProps.fields && !childListChanged(node, merged.childIds)) {
                    skippedUnchangedCount++
                    emitTrace(TraceEvent.SkipUnchanged(seq = lastSeq, id = patch.id))
                    return
                }
                // Materialize old + new kits exactly once, on genuine change.
                val oldKit = node.props
                node.wireProps = WireProps(merged.fields, merged.childIds)
                node.props = newKit
                reconciled[patch.id] = (reconciled[patch.id] ?: 0) + 1
                updatedCount++
                propMaterializations += 2u
                withAdapter(node.kind, node.componentId, node.view) { adapter, view ->
                    adapter.update(view, newKit)
                }
                // DevTools: report the view mutation so the component tree shows
                // the live node graph. The host crate is Android-free and drives
                // in-memory adapter views, so geometry (frame) is unavailable here
                // — it is filled by the platform shell (ADR-0048). `null` still
                // records node presence in the DevTools state.
                if (TelemetryBridge.sink != null) {
                    TelemetryBridge.emit(
                        TelemetryEvent.ViewMutation(
                            nodeId = node.id,
                            nativeViewId = node.view.nodeId.toULong(),
                            parentId = parents[patch.id] ?: 0u,
                            mutationKind = 0u.toUByte(),
                            frame = null,
                        ),
                    )
                }
                emitTrace(TraceEvent.Update(seq = lastSeq, id = patch.id))
            }
            0x03 -> { // Insert
                val wire = patch.node ?: return
                val parent = nodes[patch.parentId] ?: return
                val built = build(wire, mapOf(wire.id to wire), executor, depth = 0u)
                val idx = patch.index.toInt().coerceIn(0, parent.children.size)
                parent.children.add(idx, built)
                parents[built.id] = patch.parentId
                built.children.forEach { parents[it.id] = built.id }
                nodes[built.id] = built
                collect(built)
                emitTrace(
                    TraceEvent.SetChildren(
                        seq = lastSeq,
                        id = parent.id,
                        n = parent.children.size.toUInt(),
                    ),
                )
                withAdapter(parent.kind, parent.componentId, parent.view) { adapter, view ->
                    adapter.setChildren(view, parent.children.map { it.id }, parent.children.map { it.view })
                }
                // DevTools: report the newly-created node so the component tree
                // shows the live node graph (geometry is unavailable in the
                // Android-free host crate; the shell fills it — ADR-0048).
                if (TelemetryBridge.sink != null) {
                    TelemetryBridge.emit(
                        TelemetryEvent.ViewMutation(
                            nodeId = built.id,
                            nativeViewId = built.view.nodeId.toULong(),
                            parentId = patch.parentId,
                            mutationKind = 0u.toUByte(),
                            frame = null,
                        ),
                    )
                }
            }
            0x04 -> { // Remove
                val node = nodes.remove(patch.id) ?: return
                parents[patch.id]?.let { pid ->
                    nodes[pid]?.children?.removeIf { it.id == patch.id }
                    parents.remove(patch.id)
                }
                // Fire the node's `onCleanup` lifecycle hook (§18.4) before the
                // view is torn down, so teardown side effects run live.
                (executor as? HostExecutor)?.onNodeRemoved(patch.id)
                destroySubtree(node)
                detachedCount++
                emitTrace(TraceEvent.Detach(seq = lastSeq, id = patch.id))
                // DevTools: report the removed node (mutation_kind 1) so the
                // component tree drops it from the live node graph.
                if (TelemetryBridge.sink != null) {
                    TelemetryBridge.emit(
                        TelemetryEvent.ViewMutation(
                            nodeId = patch.id,
                            nativeViewId = node.view.nodeId.toULong(),
                            parentId = parents[patch.id] ?: 0u,
                            mutationKind = 1u.toUByte(),
                            frame = null,
                        ),
                    )
                }
            }
            0x06 -> { // Handler
                val node = nodes[patch.id] ?: return
                val closure: ClosureRef = patch.closure ?: return
                node.view.setProperty("closureRef", closure)
                // If this handler is a node's prop thunk, re-materialise the
                // node's dynamic props so a thunk-body edit (e.g. a changed
                // string literal) takes effect immediately, without waiting for
                // a signal change (FR hot-reload). Non-thunk handlers (e.g.
                // onClick) need no view mutation here.
                val nodeId = thunkHandlerToNode[patch.id] ?: return
                val target = nodes[nodeId] ?: return
                val newProps = materializeProps(target.wireProps.fields, target.id)
                withAdapter(target.kind, target.componentId, target.view) { a, v -> a.update(v, newProps) }
                updatedCount++
                emitTrace(TraceEvent.Update(seq = lastSeq, id = nodeId))
            }
            0x07 -> { // Reattach (roadmap Phase 3): preserve the live instance's
                // state across a structural edit that changed the node id but not
                // its component identity (e.g. Column -> Row, or a re-spanned
                // subtree). Re-key the built ShadowNode from oldId to newId instead
                // of tearing it down and rebuilding (which would reset state).
                val wire = patch.node ?: return
                val existing = nodes.remove(patch.oldId) ?: run {
                    // No live instance to preserve: build the replacement fresh
                    // rather than going blank.
                    val built = build(wire, patchIndex + (wire.id to wire), executor, depth = 0u)
                    nodes[built.id] = built
                    collect(built)
                    return
                }
                // Re-key the SAME live instance from oldId to newId so its signal
                // state, refs and scroll/focus survive (ShadowNode is a class with
                // an immutable `id`, so we re-key the map entry, not the node).
                nodes[patch.newId] = existing
                // Keep the parent/child + signal-dep maps coherent for dirty walks.
                parents[patch.oldId]?.let { parents[patch.newId] = it }
                parents.remove(patch.oldId)
                signalDeps[patch.oldId]?.let { signalDeps[patch.newId] = it }
                signalDeps.remove(patch.oldId)
                reconciled[patch.newId] = (reconciled[patch.newId] ?: 0) + 1
                // Re-materialise props against the new node shape and push the
                // delta to the (preserved) native view; handler bindings are
                // retained (handler ids are stable across the reattach).
                val newProps = materializeProps(wire.props, patch.newId)
                withAdapter(existing.kind, existing.componentId, existing.view) { a, v ->
                    a.update(v, newProps)
                }
                existing.wireProps = WireProps(wire.props, childIdList(wire))
                existing.props = newProps
                // Rebuild the child subtree under the preserved instance so a
                // nested dirty child lands correctly (children reuse build()).
                val childIndex = patchIndex + (wire.id to wire)
                existing.children.clear()
                for (child in wire.children) {
                    val childId = childIdOf(child) ?: continue
                    val childWire = childIndex[childId] ?: continue
                    existing.children.add(build(childWire, childIndex, executor, depth = 1u))
                }
                withAdapter(existing.kind, existing.componentId, existing.view) { a, v ->
                    a.setChildren(v, existing.children.map { it.id }, existing.children.map { it.view })
                }
                emitTrace(TraceEvent.Update(seq = lastSeq, id = patch.newId))
                updatedCount++
            }
            else -> { /* Reorder/unknown tags are no-ops for the MLP host */ }
        }
    }

    /** Extracts the child node id from a [WireChild] (used by Reattach). */
    private fun childIdOf(child: WireChild): UInt? =
        when (child) {
            is WireChild.Node -> child.id
            is WireChild.Splice -> child.items.firstOrNull()?.second
        }

    /** Applies [diff] on top of [base], returning a new wire-prop bag. */
    private fun mergeProps(
        base: WireProps,
        diff: PropDiff,
    ): WireProps {
        val fields = base.fields.toMutableList()
        for ((idx, value) in diff.changes) {
            val pos = fields.indexOfFirst { it.first == idx }
            if (pos >= 0) fields[pos] = idx to value else fields.add(idx to value)
        }
        fields.removeIf { (idx, _) -> diff.removals.any { it == idx } }
        return WireProps(fields, base.childIds)
    }

    private fun parentOf(id: UInt): ShadowNode? {
        val pid = parents[id] ?: return null
        return nodes[pid]
    }

    /**
     * Builds a [ShadowNode] (and its subtree) from [wire], resolving children by
     * id. Records signal dependencies (R1) and a raw prop hash (T5) so later
     * reconciles can skip unchanged subtrees without re-materializing kits.
     */
    private fun build(
        wire: WireNode,
        index: Map<UInt, WireNode>,
        executor: FluxExecutor,
        depth: UInt,
    ): ShadowNode {
        val adapter = adapterFor(wire.kind, wire.componentId)
        // ADR-0027 (FA-IRWIRE): materialise dynamic props (interpolations, signal
        // reads) by running the node's prop thunk against the live graph. Stored
        // `wireProps` keep the shipped (raw) fields for diffing; the adapter
        // receives the materialised kit.
        val props = materializeProps(wire.props, wire.id)
        val view =
            adapter?.create(wire.id)
                ?: error(
                    "no adapter registered for component ${wire.componentId} " +
                        "(kind \"${wire.kind}\", node ${wire.id})",
                )
        propMaterializations++
        withAdapter(wire.kind, wire.componentId, view) { a, v -> a.update(v, props) }
        val childIds = childIdList(wire)
        // R1 signal dependencies come from two sources: the explicit
        // `signal_meta` section the dev server ships (the authoritative record
        // of which signals a prop thunk reads — e.g. the `count` signal the
        // interpolated Text reads) and any legacy `IntVal`-based deps inferred
        // from the raw props. The `signal_meta` deps are required: in the
        // counter example the Text node's `count` dependency is carried only in
        // `signal_meta`, never as an `IntVal` prop, so ignoring it leaves the
        // node with an empty dependency set and it is never re-materialised on
        // a tap (the label freezes at "tapped 0 times").
        // A `Router` node must re-reconcile whenever its navigation target
        // changes, so it subscribes to the `Router.navigate` result signal
        // (97, ADR-0045). The reconciler reads that signal to pick which child
        // `Screen` is visible (see `routerActiveChild`).
        val isRouter = adapter?.kind == ROUTER_KIND
        val deps =
            (signalMeta[wire.id]?.deps?.toMutableSet() ?: mutableSetOf()).apply {
                addAll(signalDepsFrom(wire.props))
                if (isRouter) add(NAVIGATION_ROUTE_SIGNAL_ID)
            }
        val node =
            ShadowNode(
                id = wire.id,
                // Resolve the render kind to the adapter's tag (e.g. "text",
                // "column", "container"); the wire carries the raw NodeKind enum
                // (0 = component, 1 = primitive), which the Compose renderer
                // cannot switch on directly.
                kind = adapter.kind,
                componentId = wire.componentId,
                key = null,
                isPure = wire.isPure,
                wireProps = WireProps(wire.props, childIds),
                propsState = propsStateFactory(props),
                view = view,
                signalDeps = deps,
            )
        reconciled[wire.id] = (reconciled[wire.id] ?: 0) + 1
        builtCount++
        emitTrace(TraceEvent.Build(seq = lastSeq, id = wire.id))
        for (child in wire.children) {
            val childId =
                when (child) {
                    is WireChild.Node -> child.id
                    is WireChild.Splice -> child.items.firstOrNull()?.second ?: 0u
                }
            val childWire = index[childId] ?: continue
            node.children.add(build(childWire, index, executor, depth + 1u))
        }
        withAdapter(wire.kind, wire.componentId, view) { a, v ->
            if (adapter?.kind == ROUTER_KIND) {
                val active = routerActiveChild(node)
                if (active != null) {
                    a.setChildren(v, listOf(active.id), listOf(active.view))
                }
            } else {
                a.setChildren(v, node.children.map { it.id }, node.children.map { it.view })
            }
        }
        withAdapter(wire.kind, wire.componentId, view) { a, v -> a.bindHandler(v, props, WeakReference(executor)) }
        (executor as? HostExecutor)?.onNodeCreated(wire.id)
        emitTrace(
            TraceEvent.SetChildren(
                seq = lastSeq,
                id = wire.id,
                n = node.children.size.toUInt(),
            ),
        )
        emitTrace(TraceEvent.Mount(seq = lastSeq, id = wire.id))
        return node
    }

    /** Tears down [node] and its entire subtree (destroy views + clear state). */
    private fun destroySubtree(node: ShadowNode) {
        for (child in node.children) destroySubtree(child)
        signalDeps.remove(node.id)
        nodes.remove(node.id)
        parents.remove(node.id)
        reconciled.remove(node.id)
        withAdapter(node.kind, node.componentId, node.view) { adapter, view -> adapter.destroy(view) }
    }

    private fun childIdList(wire: WireNode): List<UInt> =
        wire.children.map {
            when (it) {
                is WireChild.Node -> it.id
                is WireChild.Splice -> it.items.firstOrNull()?.second ?: 0u
            }
        }

    /** True when [node]'s resolved child id list differs from [fresh] (T5). */
    private fun childListChanged(
        node: ShadowNode,
        fresh: List<UInt>,
    ): Boolean = node.wireProps.childIds != fresh

    /** The int-valued props of [props] are treated as reads of those signal ids (R1, iOS parity). */
    private fun signalDepsFrom(props: List<Pair<UShort, dev.flux.host.wire.WireValue>>): MutableSet<UInt> {
        val set = LinkedHashSet<UInt>()
        for ((_, value) in props) {
            if (value is dev.flux.host.wire.WireValue.IntVal) set.add(value.value.toUInt())
        }
        signalDepsFromWireInto(props, set)
        return set
    }

    private fun signalDepsFromWireInto(
        props: List<Pair<UShort, dev.flux.host.wire.WireValue>>,
        set: MutableSet<UInt>,
    ) {
        for ((_, value) in props) {
            when (value) {
                is dev.flux.host.wire.WireValue.IntVal -> set.add(value.value.toUInt())
                is dev.flux.host.wire.WireValue.ListVal -> for (item in value.items) collectInts(item, set)
                is dev.flux.host.wire.WireValue.RecordVal -> for (f in value.fields) collectInts(f.value, set)
                else -> Unit
            }
        }
    }

    private fun collectInts(
        value: dev.flux.host.wire.WireValue,
        set: MutableSet<UInt>,
    ) {
        when (value) {
            is dev.flux.host.wire.WireValue.IntVal -> set.add(value.value.toUInt())
            is dev.flux.host.wire.WireValue.ListVal -> value.items.forEach { collectInts(it, set) }
            is dev.flux.host.wire.WireValue.RecordVal -> value.fields.forEach { collectInts(it.value, set) }
            else -> Unit
        }
    }

    /** Materializes a kit [Props] from raw wire values. */
    internal fun kitFromWire(
        fields: List<Pair<UShort, dev.flux.host.wire.WireValue>>,
        stringLookup: (UInt) -> String? = { null },
    ): Props =
        Props(
            fields.map {
                dev.flux.ui.Props
                    .Field(it.first, it.second.toKitValue(stringLookup))
            },
        )

    /**
     * ADR-0027 (FA-IRWIRE) prop-thunk materialisation. For a dynamic node whose
     * [signalMeta] carries a `thunk`, runs that thunk against the live signal
     * graph (via the executor's VM) and maps its result `Record` (in `r1`) into a
     * kit [Props] using the captured `layout` (record position → `PropIdx`).
     *
     * Strings produced by the thunk's `TO_STRING`/`STR_CONCAT` are interned into
     * the VM's own [StringResolver] (not the frame's string table), so they must
     * be resolved through that resolver here — passing the VM-interned id through
     * to [kitFromWire] would render interpolated text as a raw numeric id. Falls
     * back to [kitFromWire] over the shipped [wireProps] when the node has no
     * thunk or the thunk cannot be evaluated.
     */
    internal fun materializeProps(
        wireProps: List<Pair<UShort, dev.flux.host.wire.WireValue>>,
        nodeId: UInt,
    ): Props {
        val meta = signalMeta[nodeId]
        val thunk = meta?.thunk
        val bytecode = thunk?.let { thunkBlobs[thunkKey(it.hash)] }
        val host = executorRef as? HostExecutor
        val base: Props =
            if (meta != null && thunk != null && bytecode != null && host != null) {
                val result =
                    FluxBytecodeVM.run(
                        bytecode,
                        host.materializationSignals,
                        dev.flux.host.vm.FluxValue.NullVal,
                        host.materializationStrings,
                    )
                val record =
                    (result as? VmResult.Success)
                        ?.outcome
                        ?.registers
                        ?.getOrNull(1) as? dev.flux.host.vm.FluxValue.RecordVal
                if (record != null) {
                    val fields = ArrayList<dev.flux.ui.Props.Field>(meta.layout.size)
                    for ((pos, propIdx) in meta.layout.withIndex()) {
                        if (pos >= record.fields.size) break
                        fields.add(
                            dev.flux.ui.Props.Field(
                                propIdx,
                                record.fields[pos].value.toKitValue(host.materializationStrings),
                            ),
                        )
                    }
                    Props(fields)
                } else {
                    kitFromWire(wireProps, stringLookup)
                }
            } else {
                kitFromWire(wireProps, stringLookup)
            }
        val deps = meta?.deps ?: emptyList()
        return base
    }

    /**
     * Resolves the active route string from the `Router.navigate` signal (97).
     *
     * `Router.navigate(target)` writes the argument **record** to signal 97
     * (ADR-0045); the route is that record's first field, an interned string id.
     * Returns the resolved route literal, or `null` when the signal is unset or
     * malformed (in which case callers fall back to the first child screen).
     */
    private fun activeRouteFromSignal(): String? {
        val host = executorRef as? HostExecutor ?: return null
        val raw = host.materializationSignals.read(97u) ?: return null
        // `Router.navigate(target)` writes the target to signal 97 (ADR-0045). The
        // compiler emits `LOAD_STR_CONST` + `CALL_CAP`, so a real tap stores a raw
        // `StrVal` holding the interned route-string id; some seeds wrap it in a
        // `RecordVal` (first field = the id). Accept BOTH shapes, mirroring the iOS
        // `RouterAdapter.routerActiveChildId`, so navigation never silently no-ops
        // (the reported "go to settings does nothing" bug).
        val routeId = when (raw) {
            is dev.flux.host.vm.FluxValue.StrVal -> raw.id
            is dev.flux.host.vm.FluxValue.RecordVal -> {
                val field = raw.fields.firstOrNull()?.value ?: return null
                (field as? dev.flux.host.vm.FluxValue.StrVal)?.id ?: return null
            }
            else -> return null
        }
        return stringLookup(routeId)
    }

    /**
     * Reads a `Screen` node's `route` prop (an interned string id) as a literal.
     * Returns `null` when the node is not a screen or carries no `route`.
     */
    private fun routeOf(node: ShadowNode): String? {
        val field = node.wireProps.fields.firstOrNull { it.first == ROUTE_PROP_INDEX } ?: return null
        val strId = (field.second as? dev.flux.host.wire.WireValue.StrVal)?.id ?: return null
        return stringLookup(strId)
    }

    /**
     * For a `Router` node, returns the single child `Screen` whose `route` prop
     * matches the active navigation signal (97). When no screen matches — or the
     * signal is unset — returns the first child so the stack always shows a
     * screen. The result drives `setChildren`, which reconciles the visible stack
     * to exactly this screen (Appendix F.6, keyed by node id, no view recreation).
     */
    internal fun routerActiveChild(node: ShadowNode): ShadowNode? {
        val active = activeRouteFromSignal()
        val children = node.children
        if (active != null) {
            for (child in children) {
                if (routeOf(child) == active) return child
            }
        }
        return children.firstOrNull()
    }

    /**
     * The single `Screen` child a `Router` node should render, chosen by the
     * active navigation signal (97, ADR-0045). Returns `null` for a non-router
     * node or a router with no screens. The Android Compose renderer
     * ([dev.flux.app.ShadowTreeRenderer]) uses this to show exactly one screen
     * and to re-render when a `Router.navigate` swaps the visible route — the
     * host side already drives the same query through the frozen adapter's
     * `setChildren`, but the Compose projection must consult it too, otherwise
     * every screen stacks in a column and tapping navigate does nothing.
     */
    public fun activeChildOf(node: ShadowNode): ShadowNode? =
        if (node.kind == ROUTER_KIND) routerActiveChild(node) else null

    private companion object {
        /** FNV-1a prop-index for the `route` prop name (matches the wire encoder). */
        val ROUTE_PROP_INDEX: UShort = fnv1aPropIndexForName("route")

        /** FNV-1a (32-bit) hash of [name], matching the wire's `prop_index_for_name`. */
        fun fnv1aPropIndexForName(name: String): UShort {
            var h: UInt = 0x811c9dc5u
            for (b in name.toByteArray(Charsets.UTF_8)) {
                h = (h xor b.toUInt()) * 0x1000193u
            }
            return h.toUShort()
        }
    }

    /** Converts a VM [FluxValue] (thunk result) into the kit [dev.flux.ui.FluxValue],
     *  resolving `StrVal` ids through [resolver] so interpolated text survives. */
    private fun dev.flux.host.vm.FluxValue.toKitValue(resolver: dev.flux.host.vm.StringResolver): dev.flux.ui.FluxValue =
        when (this) {
            is dev.flux.host.vm.FluxValue.IntVal ->
                dev.flux.ui.FluxValue
                    .Int(value)
            is dev.flux.host.vm.FluxValue.FloatVal ->
                dev.flux.ui.FluxValue
                    .Float(value)
            is dev.flux.host.vm.FluxValue.BoolVal ->
                dev.flux.ui.FluxValue
                    .Bool(value)
            is dev.flux.host.vm.FluxValue.StrVal ->
                dev.flux.ui.FluxValue
                    .Str(resolver.resolve(id))
            dev.flux.host.vm.FluxValue.NullVal -> dev.flux.ui.FluxValue.Null
            is dev.flux.host.vm.FluxValue.HandlerRefVal ->
                dev.flux.ui.FluxValue
                    .HandlerRef(handlerId)
            is dev.flux.host.vm.FluxValue.ListVal ->
                dev.flux.ui.FluxValue
                    .List(items.map { it.toKitValue(resolver) })
            is dev.flux.host.vm.FluxValue.RecordVal ->
                dev.flux.ui.FluxValue.Record(
                    fields.map {
                        dev.flux.ui.FluxValue
                            .Field(it.index, it.value.toKitValue(resolver))
                    },
                )
        }

    internal fun emitStepEnd() {
        stepCount++
        emitTrace(
            TraceEvent.StepEnd(
                seq = lastSeq,
                i = stepCount,
                built = builtCount,
                updated = updatedCount,
                skippedUnchanged = skippedUnchangedCount,
                skippedPure = skippedPureCount,
                detached = detachedCount,
                propMaterializations = propMaterializations,
            ),
        )
        // Counters are per-script-step (reconcile-trace-format.md goldens assert
        // e.g. `prop_materializations: 2` for a single dispatch), so reset after
        // each step_end. `stepCount` stays monotonic to label the step.
        builtCount = 0u
        updatedCount = 0u
        skippedUnchangedCount = 0u
        skippedPureCount = 0u
        detachedCount = 0u
        propMaterializations = 0u
    }

    /**
     * Invokes [block] on the adapter for [componentId]/[kind] (if present),
     * erasing the `out`-projection so `update`/`setChildren`/`destroy`/
     * `bindHandler` can be called. [view] is the view that adapter [create]d.
     */
    @Suppress("UNCHECKED_CAST")
    internal fun withAdapter(
        kind: String,
        componentId: UInt,
        view: FluxNativeView,
        block: (FluxAdapter<FluxNativeView>, FluxNativeView) -> Unit,
    ) {
        val adapter = adapterFor(kind, componentId) ?: return
        block(adapter as FluxAdapter<FluxNativeView>, view)
    }

    /** Resolves the adapter for [componentId], falling back to the raw [kind] tag.
     * A component-kind node (kind "0", i.e. [NodeKind.component]) that resolves to
     * no primitive adapter is backed by the container adapter, which simply hosts
     * its children (mirrors the iOS dev runtime). */
    internal fun adapterFor(
        kind: String,
        componentId: UInt,
    ): FluxAdapter<*>? {
        val resolved = registry.resolve(componentId) ?: registry.adapterForKind(kind)
        if (resolved != null) return resolved
        // A component-kind node (kind "0", i.e. NodeKind.component) that resolves
        // to no primitive adapter is backed by the container adapter, which
        // simply hosts its children (mirrors the iOS dev runtime).
        return if (kind == "0") {
            FluxUiKit.adapters["container"]?.create()
        } else {
            null
        }
    }
}
