package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Dev adapter for `Router` (Appendix F.6).
 *
 * Maps a Flux `Router` node to a native navigation-frame container. Its
 * children are `Screen` nodes; the ordered child list is the back-stack.
 *
 * The router is the load-bearing piece of dev/parity for navigation: when the
 * screen list changes (a push or pop), [setChildren] reconciles by stable
 * [node id][FluxNativeView.nodeId] and **never recreates a screen view that
 * already exists**. A popped-then-pushed screen therefore keeps its view
 * instance and any nested state, exactly matching release `NavHost` semantics.
 * The host owns the actual `FrameLayout`/`NavHost`; this adapter only orders
 * and inserts/removes screen views through [FluxNativeView].
 *
 * Each node gets its own adapter instance via [create] (FLUX-007).
 */
public class RouterAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        // Router has no props (Appendix F.6).
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        // Preserve existing screen views across push/pop: only reorder/remove
        // what is already present, and pull brand-new screens from [children].
        val byId = children.associateBy { it.nodeId }
        reconcileChildren(view, childIds) { byId[it] }
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
        // The router itself has no bound handlers; navigation is driven by
        // signal graph changes that re-order its child list.
    }

    override fun destroy(view: FluxNativeView) {
        view.clearChildren()
    }

    internal companion object {
        /** The kind tag this adapter handles. Exposed for the factory map. */
        const val KIND: String = "router"

        /** Builds a fresh [RouterAdapter] for one IR node (FLUX-007). */
        fun create(): FluxAdapter<FluxNativeView> = RouterAdapter()
    }
}
