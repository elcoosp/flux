package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Dev adapter for `TextField` (Appendix F.5).
 *
 * Maps a Flux `TextField` node to a native editable text view. The controlled
 * [text][PropsIndex.TEXT_FIELD_TEXT] is pushed on every [update]; when the
 * user edits, the view dispatches the `onChange` handler (carrying the new
 * string) through the weakly-held executor. A secure flag swaps the view to a
 * password field; [enabled] gates editing.
 */
public class TextFieldAdapter : FluxAdapter<FluxNativeView> {
    override val kind: String = "text_field"

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val text = props.getString(PropsIndex.TEXT_FIELD_TEXT).orEmpty()
        if (view.getProperty(PROP_TEXT) != text) view.setProperty(PROP_TEXT, text)

        val enabled = props.getBool(PropsIndex.TEXT_FIELD_ENABLED, true)
        if (view.getProperty(PROP_ENABLED) != enabled) view.setProperty(PROP_ENABLED, enabled)

        val secure = props.getBool(PropsIndex.TEXT_FIELD_SECURE, false)
        if (view.getProperty(PROP_SECURE) != secure) view.setProperty(PROP_SECURE, secure)

        props.getString(PropsIndex.TEXT_FIELD_PLACEHOLDER)?.let { hint ->
            if (view.getProperty(PROP_PLACEHOLDER) != hint) view.setProperty(PROP_PLACEHOLDER, hint)
        }
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        // TextField has no children.
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
        val handlerId = props.getHandler(PropsIndex.TEXT_FIELD_ON_CHANGE)
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
        const val PROP_SECURE = "secure"
        const val PROP_PLACEHOLDER = "placeholder"
        const val PROP_HANDLER = "onChangeHandler"
        const val PROP_EXECUTOR = "executor"
    }
}
