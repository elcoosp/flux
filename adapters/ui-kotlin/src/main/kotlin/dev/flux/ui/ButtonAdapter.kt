package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Declarative adapter for `Button` (unified tier; AGENTS.md §3.5).
 *
 * Maps a Flux `Button` node to a native button view. Tapping the view
 * dispatches the `onClick` handler through the weakly-held executor. The
 * handler id is read fresh in [bindHandler] so a hot-swapped closure table is
 * used for the next tap.
 *
 * Each node gets its own adapter instance via [create], so the bound
 * `WeakReference<FluxExecutor>` and handler id never leak into a sibling node
 * (FLUX-007).
 */
public class ButtonAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val text = props.getString(PropsIndex.BUTTON_TEXT).orEmpty()
        if (view.getProperty(PROP_TEXT) != text) view.setProperty(PROP_TEXT, text)

        val enabled = props.getBool(PropsIndex.BUTTON_ENABLED, true)
        if (view.getProperty(PROP_ENABLED) != enabled) view.setProperty(PROP_ENABLED, enabled)

        props.getColor(PropsIndex.BUTTON_COLOR)?.let { color ->
            if (view.getProperty(PROP_COLOR) != color) view.setProperty(PROP_COLOR, color)
        }

        // FLUX-044: surface host-render-only a11y props to the native view.
        view.applyAccessibility(props)
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        // Button has no children.
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
        val handlerId = props.getHandler(PropsIndex.BUTTON_ON_PRESS)
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
        const val KIND: String = "button"

        /** Builds a fresh [ButtonAdapter] for one IR node (FLUX-007). */
        fun create(): FluxAdapter<FluxNativeView> = ButtonAdapter()

        const val PROP_TEXT = "text"
        const val PROP_ENABLED = "enabled"
        const val PROP_COLOR = "color"
        const val PROP_HANDLER = "onPressHandler"
        const val PROP_EXECUTOR = "executor"
    }
}
