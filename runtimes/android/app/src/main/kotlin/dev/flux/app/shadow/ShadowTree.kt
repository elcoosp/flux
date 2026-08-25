package dev.flux.app.shadow

import dev.flux.app.AdapterRegistry
import dev.flux.app.StringTableEntry
import dev.flux.app.wire.ClosureRef
import dev.flux.app.wire.Frame
import dev.flux.app.wire.Patch
import dev.flux.app.wire.PropDiff
import dev.flux.app.wire.WireChild
import dev.flux.app.wire.WireNode
import dev.flux.app.wire.toKitValue
import dev.flux.ui.FluxAdapter
import dev.flux.ui.FluxExecutor
import dev.flux.ui.FluxNativeView
import dev.flux.ui.Props
import java.lang.ref.WeakReference
import dev.flux.app.FluxExecutor as HostExecutor

/**
 * The host render tree: a map of [ShadowNode]s keyed by id, plus the adapter
 * registry that translates IR nodes into native views.
 *
 * The shadow tree is the source of truth for *structure* (the dev server owns
 * the IR; the host owns the signal graph — ADR-0002). Patches from the wire (or
 * a full-tree Init frame) mutate it through [applyFrame]; the reconciler then
 * drives the adapters so the native view subtree matches.
 *
 * Resolution now goes through the [AdapterRegistry], which maps an interned
 * `ComponentId` (carried on every wire node) to a dev adapter from the
 * `adapters/ui-kotlin` kit (FLUX-017). The registry is seeded from the string
 * table delivered in each `Init` frame, so a node's `componentId` resolves to
 * the correct adapter without re-deriving the kind from the raw wire byte.
 *
 * @property registry the adapter registry, keyed by `ComponentId`. Mutated in
 *   place as string-table deltas arrive so the tree always resolves against the
 *   latest `Init` frame.
 */
public class ShadowTree(
    private var registry: AdapterRegistry,
) {
    private val nodes = LinkedHashMap<UInt, ShadowNode>()
    private var root: ShadowNode? = null
    private var executorRef: FluxExecutor? = null

    // How many times each node's view has been reconciled (built or updated).
    // Used by the `@pure` skip (§18.10) and observable in tests.
    private val reconciled = LinkedHashMap<UInt, Int>()

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

    /** Looks up a shadow node by id. */
    public fun node(id: UInt): ShadowNode? = nodes[id]

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
        if (frame.strings.isNotEmpty()) {
            registry = registry.withEntries(frame.strings.map { StringTableEntry(it.id, it.text) })
        }
        if (frame.fullTree && frame.root != null) {
            val index = LinkedHashMap<UInt, WireNode>()
            index[frame.root.id] = frame.root
            for (n in frame.extraNodes) index[n.id] = n
            val built = build(frame.root, index, executor)
            root = built
            nodes.clear()
            collect(built)
            return built
        }
        for (patch in frame.patches) applyPatch(patch, executor)
        return root
    }

    private fun collect(node: ShadowNode) {
        nodes[node.id] = node
        for (child in node.children) collect(child)
    }

    private fun applyPatch(
        patch: Patch,
        executor: FluxExecutor,
    ) {
        executorRef = executor
        when (patch.tag.toInt()) {
            0x01 -> { // Replace
                val wire = patch.node ?: return
                val built = build(wire, mapOf(wire.id to wire), executor)
                val existing = nodes[patch.id]
                if (existing != null) {
                    parentOf(patch.id)?.let { parent ->
                        val idx = parent.children.indexOfFirst { it.id == patch.id }
                        if (idx >= 0) parent.children[idx] = built
                    }
                }
                nodes[patch.id] = built
                collect(built)
            }
            0x02 -> { // Update
                val node = nodes[patch.id] ?: return
                val diff = patch.diff ?: return
                val merged = mergeProps(node.props, diff)
                // `@pure` skip (§18.10): a pure node is a function of its props,
                // so when the merged props are referentially equal to the prior
                // props there is nothing to reconcile — skip the adapter update
                // and its subtree entirely. This is what lets an unrelated
                // signal update elsewhere in the tree leave a stable `@pure`
                // subtree untouched.
                if (node.isPure && merged.fields == node.props.fields) return
                node.props = merged
                reconciled[patch.id] = (reconciled[patch.id] ?: 0) + 1
                withAdapter(node.kind, node.componentId, node.view) { adapter, view ->
                    adapter.update(view, merged)
                }
            }
            0x03 -> { // Insert
                val wire = patch.node ?: return
                val parent = nodes[patch.parentId] ?: return
                val built = build(wire, mapOf(wire.id to wire), executor)
                val idx = patch.index.toInt().coerceIn(0, parent.children.size)
                parent.children.add(idx, built)
                nodes[built.id] = built
                collect(built)
                withAdapter(parent.kind, parent.componentId, parent.view) { adapter, view ->
                    adapter.setChildren(view, parent.children.map { it.id }, parent.children.map { it.view })
                }
            }
            0x04 -> { // Remove
                val node = nodes.remove(patch.id) ?: return
                parentOf(patch.id)?.children?.removeIf { it.id == patch.id }
                // Fire the node's `onCleanup` lifecycle hook (§18.4) before the
                // view is torn down, so teardown side effects (close socket,
                // unsubscribe) run against a still-live node.
                (executor as? HostExecutor)?.onNodeRemoved(patch.id)
                withAdapter(node.kind, node.componentId, node.view) { adapter, view -> adapter.destroy(view) }
            }
            0x06 -> { // Handler
                val node = nodes[patch.id] ?: return
                val closure: ClosureRef = patch.closure ?: return
                node.view.setProperty("closureRef", closure)
            }
            else -> { /* Reorder/unknown tags are no-ops for the MLP host */ }
        }
    }

    /** Applies [diff] on top of [base], returning a new [Props]. */
    private fun mergeProps(
        base: Props,
        diff: PropDiff,
    ): Props {
        val fields = base.fields.toMutableList()
        for ((idx, value) in diff.changes) {
            val field =
                dev.flux.ui.Props
                    .Field(idx, value.toKitValue())
            val pos = fields.indexOfFirst { it.index == idx }
            if (pos >= 0) fields[pos] = field else fields.add(field)
        }
        fields.removeIf { field -> diff.removals.any { it == field.index } }
        return base.copy(fields = fields)
    }

    private fun parentOf(id: UInt): ShadowNode? {
        for (node in nodes.values) {
            if (node.children.any { it.id == id }) return node
        }
        return null
    }

    /** Builds a [ShadowNode] (and its subtree) from [wire], resolving children by id. */
    private fun build(
        wire: WireNode,
        index: Map<UInt, WireNode>,
        executor: FluxExecutor,
    ): ShadowNode {
        val adapter = adapterFor(wire.kind, wire.componentId)
        val props =
            Props(
                wire.props.map {
                    dev.flux.ui.Props
                        .Field(it.first, it.second.toKitValue())
                },
            )
        val view =
            adapter?.create(wire.id)
                ?: error("no adapter registered for component ${wire.componentId} (kind \"${wire.kind}\", node ${wire.id})")
        withAdapter(wire.kind, wire.componentId, view) { a, v -> a.update(v, props) }
        val node = ShadowNode(wire.id, wire.kind, wire.componentId, key = null, isPure = wire.isPure, props, view)
        reconciled[wire.id] = (reconciled[wire.id] ?: 0) + 1
        for (child in wire.children) {
            val childId =
                when (child) {
                    is WireChild.Node -> child.id
                    is WireChild.Splice -> child.items.firstOrNull()?.second ?: 0u
                }
            val childWire = index[childId] ?: continue
            node.children.add(build(childWire, index, executor))
        }
        withAdapter(wire.kind, wire.componentId, view) { a, v ->
            a.setChildren(v, node.children.map { it.id }, node.children.map { it.view })
        }
        withAdapter(wire.kind, wire.componentId, view) { a, v -> a.bindHandler(v, props, WeakReference(executor)) }
        (executor as? HostExecutor)?.onNodeCreated(wire.id)
        return node
    }

    /**
     * Invokes [block] on the adapter for [componentId]/[kind] (if present),
     * erasing the `out`-projection so `update`/`setChildren`/`destroy`/
     * `bindHandler` can be called. The contract guarantees every adapter's `V`
     * is a `FluxNativeView`, so the unchecked cast is sound (see adapter kit
     * docs). [view] is supplied by the caller and is always the view that
     * adapter [create]d for the node.
     */
    @Suppress("UNCHECKED_CAST")
    private fun withAdapter(
        kind: String,
        componentId: UInt,
        view: FluxNativeView,
        block: (FluxAdapter<FluxNativeView>, FluxNativeView) -> Unit,
    ) {
        val adapter = adapterFor(kind, componentId) ?: return
        block(adapter as FluxAdapter<FluxNativeView>, view)
    }

    /** Resolves the adapter for [componentId], falling back to the raw [kind] tag. */
    private fun adapterFor(
        kind: String,
        componentId: UInt,
    ): FluxAdapter<*>? = registry.resolve(componentId) ?: registry.adapterForKind(kind)
}
