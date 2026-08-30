package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Declarative adapter for `Gesture` (FLUX-041, unified tier; AGENTS.md §3.5).
 *
 * Maps a Flux `Gesture` wrapper node to a native gesture surface. A `Gesture`
 * is a **container**: it hosts a subtree and attaches one gesture recognizer
 * (selected by [kind][PropsIndex.GESTURE_KIND]) to the whole surface. When the
 * gesture fires, the view dispatches the `onGesture` handler through the
 * weakly-held executor. Drag/pinch recognizers may additionally surface a
 * continuous [threshold][PropsIndex.GESTURE_THRESHOLD] (the activation delta)
 * as a host-render-only property.
 *
 * The native recognizer attach/detach is host-side; this adapter only declares
 * the intent (the gesture kind + handler) and reconciles children by stable
 * node id (keyed reconciliation preserves child state across diffs, FLUX-007).
 *
 * Each node gets its own adapter instance via [create], so the bound
 * `WeakReference<FluxExecutor>` and handler id never leak into a sibling node.
 */
public class GestureAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val kind = props.getString(PropsIndex.GESTURE_KIND).orEmpty()
        if (view.getProperty(PROP_KIND) != kind) view.setProperty(PROP_KIND, kind)

        props.getFloat(PropsIndex.GESTURE_THRESHOLD)?.let { threshold ->
            if (view.getProperty(PROP_THRESHOLD) != threshold) view.setProperty(PROP_THRESHOLD, threshold)
        }
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        // A Gesture is a container: reconcile its subtree by stable node id so
        // reorder/patch never recreates a child and loses native state.
        val byId = children.associateBy { it.nodeId }
        reconcileChildren(view, childIds) { byId[it] }
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
        val handlerId = props.getHandler(PropsIndex.GESTURE_ON_GESTURE)
        view.setProperty(PROP_HANDLER, handlerId)
        view.setProperty(PROP_EXECUTOR, executor)
    }

    override fun destroy(view: FluxNativeView) {
        view.setProperty(PROP_EXECUTOR, null)
        view.setProperty(PROP_HANDLER, 0u)
        view.clearChildren()
    }

    internal companion object {
        /** The kind tag this adapter handles. Exposed for the factory map. */
        const val KIND: String = "gesture"

        /** Builds a fresh [GestureAdapter] for one IR node (FLUX-007). */
        fun create(): FluxAdapter<FluxNativeView> = GestureAdapter()

        const val PROP_KIND = "gestureKind"
        const val PROP_THRESHOLD = "threshold"
        const val PROP_HANDLER = "onGestureHandler"
        const val PROP_EXECUTOR = "executor"
    }
}
