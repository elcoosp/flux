package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Declarative adapter for `Checkbox` (FLUX-040, unified tier; AGENTS.md §3.5).
 *
 * Maps a Flux `Checkbox` node to a native tri-state-capable checkbox (boolean
 * in the MLP contract). The controlled [value][PropsIndex.CHECKBOX_VALUE] is
 * pushed on every [update]; when the user toggles it, the view dispatches the
 * `onChange` handler (carrying the new boolean) through the weakly-held
 * executor. An optional [label][PropsIndex.CHECKBOX_LABEL] is surfaced as a
 * host-render-only text property; [enabled] gates interaction.
 *
 * Each node gets its own adapter instance via [create] (FLUX-007).
 */
public class CheckboxAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val value = props.getBool(PropsIndex.CHECKBOX_VALUE, false)
        if (view.getProperty(PROP_VALUE) != value) view.setProperty(PROP_VALUE, value)

        val enabled = props.getBool(PropsIndex.CHECKBOX_ENABLED, true)
        if (view.getProperty(PROP_ENABLED) != enabled) view.setProperty(PROP_ENABLED, enabled)

        props.getString(PropsIndex.CHECKBOX_LABEL)?.let { label ->
            if (view.getProperty(PROP_LABEL) != label) view.setProperty(PROP_LABEL, label)
        }
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        // Checkbox has no children.
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
        val handlerId = props.getHandler(PropsIndex.CHECKBOX_ON_CHANGE)
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
        const val KIND: String = "checkbox"

        /** Builds a fresh [CheckboxAdapter] for one IR node (FLUX-007). */
        fun create(): FluxAdapter<FluxNativeView> = CheckboxAdapter()

        const val PROP_VALUE = "value"
        const val PROP_ENABLED = "enabled"
        const val PROP_LABEL = "label"
        const val PROP_HANDLER = "onChangeHandler"
        const val PROP_EXECUTOR = "executor"
    }
}
