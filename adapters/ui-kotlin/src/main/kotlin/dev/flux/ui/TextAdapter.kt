package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Declarative adapter for `Text` (unified tier; AGENTS.md §3.5).
 *
 * Maps a Flux `Text` node to a native text view. In the Android runtime the
 * backing [FluxNativeView] wraps an `android.widget.TextView`; this adapter
 * only declares intent through [FluxNativeView.setProperty] so the host
 * translates `text`, `color`, `fontSize`, `textAlignment`, `maxLines` onto the
 * real view. Button presses are not bound here — text is non-interactive.
 *
 * Each node gets its own adapter instance via [create], so no per-node view
 * state or bound handlers leak between nodes (FLUX-007).
 */
public class TextAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val text = props.getString(PropsIndex.TEXT_TEXT).orEmpty()
        if (view.getProperty(PROP_TEXT) != text) view.setProperty(PROP_TEXT, text)

        props.getColor(PropsIndex.TEXT_COLOR)?.let { color ->
            if (view.getProperty(PROP_COLOR) != color) view.setProperty(PROP_COLOR, color)
        }

        props.getFont(PropsIndex.TEXT_FONT)?.let { font ->
            if (view.getProperty(PROP_FONT_SIZE) != font.size) view.setProperty(PROP_FONT_SIZE, font.size)
        }

        props.getFloat(PropsIndex.TEXT_SIZE)?.let { size ->
            if (view.getProperty(PROP_FONT_SIZE) != size) view.setProperty(PROP_FONT_SIZE, size)
        }

        props.getString(PropsIndex.TEXT_ALIGNMENT)?.let { align ->
            if (view.getProperty(PROP_ALIGNMENT) != align) view.setProperty(PROP_ALIGNMENT, align)
        }

        props.getInt(PropsIndex.TEXT_MAX_LINES)?.let { maxLines ->
            val v = maxLines.toInt()
            if (view.getProperty(PROP_MAX_LINES) != v) view.setProperty(PROP_MAX_LINES, v)
        }
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        // Text has no children.
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
        // Text has no interactive handlers.
    }

    override fun destroy(view: FluxNativeView) {
        view.clearChildren()
    }

    internal companion object {
        /** The kind tag this adapter handles. Exposed for the factory map. */
        const val KIND: String = "text"

        /** Builds a fresh [TextAdapter] for one IR node (FLUX-007). */
        fun create(): FluxAdapter<FluxNativeView> = TextAdapter()

        const val PROP_TEXT = "text"
        const val PROP_COLOR = "color"
        const val PROP_FONT_SIZE = "fontSize"
        const val PROP_ALIGNMENT = "textAlignment"
        const val PROP_MAX_LINES = "maxLines"
    }
}
