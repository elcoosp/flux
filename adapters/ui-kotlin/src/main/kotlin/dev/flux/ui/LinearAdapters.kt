package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Dev adapter for `Column` (Appendix F.3).
 *
 * Maps a Flux `Column` node to a vertical linear container. The [gap] spacing
 * is declared as a view property the host applies between children; the child
 * list is reconciled by stable [node id][FluxNativeView.nodeId] so reorders
 * preserve child state.
 *
 * Each node gets its own adapter instance via [create] (FLUX-007).
 */
public class ColumnAdapter private constructor() : FluxLinearAdapter(orientation = "vertical", kind = KIND) {
    internal companion object {
        /** The kind tag this adapter handles. Exposed for the factory map. */
        const val KIND: String = "column"

        /** Builds a fresh [ColumnAdapter] for one IR node (FLUX-007). */
        fun create(): FluxAdapter<FluxNativeView> = ColumnAdapter()
    }
}

/**
 * Dev adapter for `Row` (Appendix F.4).
 *
 * Maps a Flux `Row` node to a horizontal linear container. Identical contract
 * to [ColumnAdapter] modulo orientation.
 *
 * Each node gets its own adapter instance via [create] (FLUX-007).
 */
public class RowAdapter private constructor() : FluxLinearAdapter(orientation = "horizontal", kind = KIND) {
    internal companion object {
        /** The kind tag this adapter handles. Exposed for the factory map. */
        const val KIND: String = "row"

        /** Builds a fresh [RowAdapter] for one IR node (FLUX-007). */
        fun create(): FluxAdapter<FluxNativeView> = RowAdapter()
    }
}

/**
 * Shared base for the two linear-container adapters ([ColumnAdapter],
 * [RowAdapter]). Holds the orientation and implements child reconciliation
 * once so the two subclasses differ only by their [kind] tag and axis.
 */
public open class FluxLinearAdapter(
    private val orientation: String,
    override val kind: String,
) : FluxAdapter<FluxNativeView> {
    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val gap = props.getFloat(PropsIndex.STACK_GAP) ?: 0.0
        if (view.getProperty(PROP_GAP) != gap) view.setProperty(PROP_GAP, gap)
        if (view.getProperty(PROP_ORIENTATION) != orientation) view.setProperty(PROP_ORIENTATION, orientation)
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        // Reconcile the container's existing children to the desired target
        // ids; new views arrive already built in [children] and resolved here.
        val byId = children.associateBy { it.nodeId }
        reconcileChildren(view, childIds) { byId[it] }
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
        // Linear containers have no handlers of their own.
    }

    override fun destroy(view: FluxNativeView) {
        view.clearChildren()
    }

    internal companion object {
        const val PROP_GAP = "gap"
        const val PROP_ORIENTATION = "orientation"
    }
}
