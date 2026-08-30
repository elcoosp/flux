package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Declarative adapter for `Picker` (FLUX-040, unified tier; AGENTS.md §3.5).
 *
 * Maps a Flux `Picker` node to a native single-selection control. The
 * controlled [value][PropsIndex.PICKER_VALUE] (the selected option index/key)
 * is pushed on every [update]; when the user selects an option, the view
 * dispatches the `onChange` handler (carrying the new value) through the
 * weakly-held executor. The candidate [items][PropsIndex.PICKER_ITEMS] are
 * surfaced as a host-render-only list property; [enabled] gates interaction.
 *
 * Each node gets its own adapter instance via [create] (FLUX-007).
 */
public class PickerAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val value = props.getInt(PropsIndex.PICKER_VALUE) ?: 0L
        if (view.getProperty(PROP_VALUE) != value) view.setProperty(PROP_VALUE, value)

        props.get(PropsIndex.PICKER_ITEMS)?.let { items ->
            if (view.getProperty(PROP_ITEMS) != items) view.setProperty(PROP_ITEMS, items)
        }

        val enabled = props.getBool(PropsIndex.PICKER_ENABLED, true)
        if (view.getProperty(PROP_ENABLED) != enabled) view.setProperty(PROP_ENABLED, enabled)
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        // Picker has no children.
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
        val handlerId = props.getHandler(PropsIndex.PICKER_ON_CHANGE)
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
        const val KIND: String = "picker"

        /** Builds a fresh [PickerAdapter] for one IR node (FLUX-007). */
        fun create(): FluxAdapter<FluxNativeView> = PickerAdapter()

        const val PROP_VALUE = "value"
        const val PROP_ITEMS = "items"
        const val PROP_ENABLED = "enabled"
        const val PROP_HANDLER = "onChangeHandler"
        const val PROP_EXECUTOR = "executor"
    }
}
