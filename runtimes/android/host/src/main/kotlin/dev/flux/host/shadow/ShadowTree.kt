package dev.flux.host.shadow

import dev.flux.host.AdapterRegistry
import dev.flux.host.StringTableEntry
import dev.flux.host.vm.FluxBytecodeVM
import dev.flux.host.vm.VmResult
import dev.flux.host.wire.ClosureRef
import dev.flux.host.wire.Frame
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
    // real strings instead of raw ids (Appendix D §D.9).
    internal var stringLookup: (UInt) -> String? = { null }

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
    internal var thunkBlobs: Map<ByteArray, ByteArray> = emptyMap()

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

    /** The current root node, or `null` before an Init frame is applied. */
    public val rootNode: ShadowNode? get() = root

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
        frame: Frame,
        executor: FluxExecutor,
    ): ShadowNode? {
        executorRef = executor
        lastSeq = frame.seq
        if (frame.strings.isNotEmpty()) {
            registry = registry.withEntries(frame.strings.map { StringTableEntry(it.id, it.text) })
            val table = frame.strings.associate { it.id to it.text }
            stringLookup = { id -> table[id] }
        }
        // Appendix D §D.9: the Init frame's `component_names` section binds each
        // `ComponentId` to its adapter name. These are a SEPARATE id space from
        // the string literals in `frame.strings` and must not leak into the
        // string resolver (a ComponentId and a StringId can share a numeric
        // value). Feed them only to the registry.
        if (frame.componentNames.isNotEmpty()) {
            registry = registry.withEntries(frame.componentNames.map { StringTableEntry(it.id, it.text) })
        }
        // ADR-0027 (FA-IRWIRE): cache the per-node signal metadata and slice each
        // prop-thunk's bytecode from the shared handler blob so dirty reconciles
        // can re-materialise dynamic props without a full frame.
        signalMeta = frame.signalMeta
        val blob = frame.bytecodeBlob
        val thunks = LinkedHashMap<ByteArray, ByteArray>()
        if (blob != null && blob.len > 0) {
            for (meta in frame.signalMeta.values) {
                val thunk = meta.thunk ?: continue
                val start = thunk.bytecodeOffset.toInt()
                val len = thunk.bytecodeLen.toInt()
                val absStart = blob.offset + start
                if (start >= 0 && len >= 0 && absStart + len <= blob.data.size) {
                    thunks[thunk.hash] = blob.data.copyOfRange(absStart, absStart + len)
                }
            }
        }
        thunkBlobs = thunks
        if (frame.fullTree && frame.root != null) {
            val index = LinkedHashMap<UInt, WireNode>()
            index[frame.root.id] = frame.root
            for (n in frame.extraNodes) index[n.id] = n
            trace?.invoke(
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
        trace?.invoke(
            TraceEvent.Frame(
                seq = frame.seq,
                full = false,
                root = null,
                nodes = 0u,
                patches = frame.patches.size.toUInt(),
            ),
        )
        if (frame.patches.isNotEmpty()) {
            for (patch in frame.patches) applyPatch(patch, executor)
            trace?.invoke(TraceEvent.ApplyPatch(seq = frame.seq, patches = frame.patches.size.toUInt()))
            emitStepEnd()
        }
        return root
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
        executor: FluxExecutor,
    ) {
        executorRef = executor
        when (patch.tag.toInt()) {
            0x01 -> { // Replace
                val wire = patch.node ?: return
                val built = build(wire, mapOf(wire.id to wire), executor, depth = 0u)
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
                    trace?.invoke(TraceEvent.SkipUnchanged(seq = lastSeq, id = patch.id))
                    return
                }
                // T5/R2: skip the adapter update when raw props AND the child-id
                // list are identical — no native mutation is required.
                if (merged.fields == node.wireProps.fields && !childListChanged(node, merged.childIds)) {
                    skippedUnchangedCount++
                    trace?.invoke(TraceEvent.SkipUnchanged(seq = lastSeq, id = patch.id))
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
                trace?.invoke(TraceEvent.Update(seq = lastSeq, id = patch.id))
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
                trace?.invoke(
                    TraceEvent.SetChildren(
                        seq = lastSeq,
                        id = parent.id,
                        n = parent.children.size.toUInt(),
                    ),
                )
                withAdapter(parent.kind, parent.componentId, parent.view) { adapter, view ->
                    adapter.setChildren(view, parent.children.map { it.id }, parent.children.map { it.view })
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
                trace?.invoke(TraceEvent.Detach(seq = lastSeq, id = patch.id))
            }
            0x06 -> { // Handler
                val node = nodes[patch.id] ?: return
                val closure: ClosureRef = patch.closure ?: return
                node.view.setProperty("closureRef", closure)
            }
            else -> { /* Reorder/unknown tags are no-ops for the MLP host */ }
        }
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
        val deps = signalDepsFrom(wire.props)
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
                props = props,
                view = view,
                signalDeps = deps,
            )
        reconciled[wire.id] = (reconciled[wire.id] ?: 0) + 1
        builtCount++
        trace?.invoke(TraceEvent.Build(seq = lastSeq, id = wire.id))
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
            a.setChildren(v, node.children.map { it.id }, node.children.map { it.view })
        }
        withAdapter(wire.kind, wire.componentId, view) { a, v -> a.bindHandler(v, props, WeakReference(executor)) }
        (executor as? HostExecutor)?.onNodeCreated(wire.id)
        trace?.invoke(
            TraceEvent.SetChildren(
                seq = lastSeq,
                id = wire.id,
                n = node.children.size.toUInt(),
            ),
        )
        trace?.invoke(TraceEvent.Mount(seq = lastSeq, id = wire.id))
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
        val meta = signalMeta[nodeId] ?: return kitFromWire(wireProps, stringLookup)
        val thunk = meta.thunk ?: return kitFromWire(wireProps, stringLookup)
        val bytecode = thunkBlobs[thunk.hash] ?: return kitFromWire(wireProps, stringLookup)
        val host = executorRef as? HostExecutor ?: return kitFromWire(wireProps, stringLookup)
        val result =
            FluxBytecodeVM.run(
                bytecode,
                host.materializationSignals,
                dev.flux.host.vm.FluxValue.NullVal,
                host.materializationStrings,
            )
        if (result !is VmResult.Success) return kitFromWire(wireProps, stringLookup)
        val record =
            result.outcome.registers.getOrNull(1) as? dev.flux.host.vm.FluxValue.RecordVal
                ?: return kitFromWire(wireProps, stringLookup)
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
        return Props(fields)
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
        trace?.invoke(
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
        return if (kind == "0") FluxUiKit.adapters["container"] else null
    }
}
