package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Declarative adapter for `TextArea` (FLUX-040, unified tier; AGENTS.md §3.5).
 *
 * Maps a Flux `TextArea` node to a native multi-line editable text view. The
 * controlled [value][PropsIndex.TEXT_AREA_VALUE] is pushed on every [update];
 * when the user edits, the view dispatches the `onChange` handler (carrying
 * the new string) through the weakly-held executor. A [placeholder] hint and a
 * [maxLines] soft cap are surfaced as host-render-only properties; [enabled]
 * gates editing.
 *
 * Each node gets its own adapter instance via [create] (FLUX-007).
 */
public class TextAreaAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val value = props.getString(PropsIndex.TEXT_AREA_VALUE).orEmpty()
        if (view.getProperty(PROP_VALUE) != value) view.setProperty(PROP_VALUE, value)

        props.getString(PropsIndex.TEXT_AREA_PLACEHOLDER)?.let { hint ->
            if (view.getProperty(PROP_PLACEHOLDER) != hint) view.setProperty(PROP_PLACEHOLDER, hint)
        }

        val maxLines = props.getInt(PropsIndex.TEXT_AREA_MAX_LINES)
        if (maxLines != null) {
            if (view.getProperty(PROP_MAX_LINES) != maxLines) view.setProperty(PROP_MAX_LINES, maxLines)
        }

        val enabled = props.getBool(PropsIndex.TEXT_AREA_ENABLED, true)
        if (view.getProperty(PROP_ENABLED) != enabled) view.setProperty(PROP_ENABLED, enabled)
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        // TextArea has no children.
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
        val handlerId = props.getHandler(PropsIndex.TEXT_AREA_ON_CHANGE)
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
        const val KIND: String = "textarea"

        /** Builds a fresh [TextAreaAdapter] for one IR node (FLUX-007). */
        fun create(): FluxAdapter<FluxNativeView> = TextAreaAdapter()

        const val PROP_VALUE = "value"
        const val PROP_PLACEHOLDER = "placeholder"
        const val PROP_MAX_LINES = "maxLines"
        const val PROP_ENABLED = "enabled"
        const val PROP_HANDLER = "onChangeHandler"
        const val PROP_EXECUTOR = "executor"
    }
}
