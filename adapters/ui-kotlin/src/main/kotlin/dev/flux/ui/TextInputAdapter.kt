package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Declarative adapter for `TextInput` (unified tier; AGENTS.md §3.5).
 *
 * Maps a Flux `TextInput` node to a native editable text view. The controlled
 * [text][PropsIndex.TEXT_INPUT_TEXT] is pushed on every [update]; when the
 * user edits, the view dispatches the `onChangeText` handler (carrying the new
 * string) through the weakly-held executor. A secure flag swaps the view to a
 * password field; [enabled] gates editing.
 *
 * Each node gets its own adapter instance via [create], so the bound
 * `WeakReference<FluxExecutor>` and handler id never leak into a sibling node
 * (FLUX-007).
 */
public class TextInputAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val text = props.getString(PropsIndex.TEXT_INPUT_TEXT).orEmpty()
        if (view.getProperty(PROP_TEXT) != text) view.setProperty(PROP_TEXT, text)

        val enabled = props.getBool(PropsIndex.TEXT_INPUT_ENABLED, true)
        if (view.getProperty(PROP_ENABLED) != enabled) view.setProperty(PROP_ENABLED, enabled)

        val secure = props.getBool(PropsIndex.TEXT_INPUT_SECURE_TEXT_ENTRY, false)
        if (view.getProperty(PROP_SECURE) != secure) view.setProperty(PROP_SECURE, secure)

        props.getString(PropsIndex.TEXT_INPUT_PLACEHOLDER)?.let { hint ->
            if (view.getProperty(PROP_PLACEHOLDER) != hint) view.setProperty(PROP_PLACEHOLDER, hint)
        }
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        // TextInput has no children.
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
        val handlerId = props.getHandler(PropsIndex.TEXT_INPUT_ON_CHANGE_TEXT)
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
        const val KIND: String = "textinput"

        /** Builds a fresh [TextInputAdapter] for one IR node (FLUX-007). */
        fun create(): FluxAdapter<FluxNativeView> = TextInputAdapter()

        const val PROP_TEXT = "text"
        const val PROP_ENABLED = "enabled"
        const val PROP_SECURE = "secure"
        const val PROP_PLACEHOLDER = "placeholder"
        const val PROP_HANDLER = "onChangeTextHandler"
        const val PROP_EXECUTOR = "executor"
    }
}
