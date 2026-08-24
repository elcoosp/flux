package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Dev adapter for `Button` (Appendix F.2).
 *
 * Maps a Flux `Button` node to a native button view. Tapping the view
 * dispatches the `onClick` handler through the weakly-held executor. The
 * handler id is read fresh in [bindHandler] so a hot-swapped closure table is
 * used for the next tap.
 */
public class ButtonAdapter : FluxAdapter<FluxNativeView> {
    override val kind: String = "button"

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
        val handlerId = props.getHandler(PropsIndex.BUTTON_ON_CLICK)
        view.setProperty(PROP_HANDLER, handlerId)
        view.setProperty(PROP_EXECUTOR, executor)
    }

    override fun destroy(view: FluxNativeView) {
        view.setProperty(PROP_EXECUTOR, null)
        view.setProperty(PROP_HANDLER, 0u)
        view.clearChildren()
    }

    internal companion object {
        const val PROP_TEXT = "text"
        const val PROP_ENABLED = "enabled"
        const val PROP_COLOR = "color"
        const val PROP_HANDLER = "onClickHandler"
        const val PROP_EXECUTOR = "executor"
    }
}
