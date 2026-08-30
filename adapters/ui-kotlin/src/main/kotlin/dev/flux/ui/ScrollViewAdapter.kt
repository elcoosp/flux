package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

// Declarative adapter for the FLUX-056 `ScrollView` primitive (PRD-N family),
// unified tier (AGENTS.md §3.5).
//
// A scrollable viewport for a single scrollable child subtree. The adapter maps
// a Flux `ScrollView` node to a platform-neutral `FluxNativeView` carrying the
// recorded `orientation` prop; the host renderer wraps the children in a
// vertical (or horizontal) scroll container. Children are reconciled by stable
// node id (keyed reconciliation, §3.5) so reorders preserve child state. Each
// node gets its own adapter instance via create (FLUX-007).

/**
 * `ScrollView` — a scrollable viewport for its children (SwiftUI `ScrollView` /
 * Compose `verticalScroll`/`horizontalScroll`). The [PropsIndex.SCROLL_ORIENTATION]
 * prop selects the scroll axis (`"vertical"` default, `"horizontal"` otherwise);
 * it is recorded on the node so the host renderer applies the matching scroll
 * modifier. The children themselves are laid out by the host's normal container
 * flow inside the scroll viewport.
 */
public class ScrollViewAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val orientation = props.getString(PropsIndex.SCROLL_ORIENTATION) ?: "vertical"
        if (view.getProperty(PROP_ORIENTATION) != orientation) {
            view.setProperty(PROP_ORIENTATION, orientation)
        }
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        val byId = children.associateBy { it.nodeId }
        reconcileChildren(view, childIds) { byId[it] }
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
    }

    override fun destroy(view: FluxNativeView) {
        view.clearChildren()
    }

    internal companion object {
        const val KIND: String = "scrollview"
        const val PROP_ORIENTATION = "orientation"

        fun create(): FluxAdapter<FluxNativeView> = ScrollViewAdapter()
    }
}
