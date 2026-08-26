package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Dev adapter for `Screen` (Appendix F.7).
 *
 * Maps a Flux `Screen` node to a native screen container holding exactly one
 * content subtree (the screen's child). The screen's [nodeId] is its stable
 * route key: across router push/pop the [FluxNativeView] instance is preserved
 * so any local view state (scroll position, entered text in nested fields)
 * survives being popped and re-pushed.
 *
 * Each node gets its own adapter instance via [create] (FLUX-007).
 */
public class ScreenAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        // A screen carries no visual props of its own beyond its content child.
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        // A screen hosts a single content child; reconcile to at most one.
        when {
            childIds.isEmpty() -> view.clearChildren()
            else -> reconcileChildren(view, childIds) { id -> children.firstOrNull { it.nodeId == id } }
        }
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
        // Screens do not bind their own handlers.
    }

    override fun destroy(view: FluxNativeView) {
        view.clearChildren()
    }

    internal companion object {
        /** The kind tag this adapter handles. Exposed for the factory map. */
        const val KIND: String = "screen"

        /** Builds a fresh [ScreenAdapter] for one IR node (FLUX-007). */
        fun create(): FluxAdapter<FluxNativeView> = ScreenAdapter()
    }
}
