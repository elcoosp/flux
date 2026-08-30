package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Declarative adapter for `DatePicker` (FLUX-040, unified tier; AGENTS.md §3.5).
 *
 * Maps a Flux `DatePicker` node to a native date selector. The controlled
 * [value][PropsIndex.DATE_PICKER_VALUE] (an epoch-millis integer) is pushed on
 * every [update]; when the user confirms a date, the view dispatches the
 * `onChange` handler (carrying the new value) through the weakly-held
 * executor. Optional [min][PropsIndex.DATE_PICKER_MIN]/
 * [max][PropsIndex.DATE_PICKER_MAX] bounds are surfaced as host-render-only
 * properties; [enabled] gates interaction.
 *
 * Each node gets its own adapter instance via [create] (FLUX-007).
 */
public class DatePickerAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val value = props.getInt(PropsIndex.DATE_PICKER_VALUE) ?: 0L
        if (view.getProperty(PROP_VALUE) != value) view.setProperty(PROP_VALUE, value)

        val min = props.getInt(PropsIndex.DATE_PICKER_MIN) ?: 0L
        if (view.getProperty(PROP_MIN) != min) view.setProperty(PROP_MIN, min)

        val max = props.getInt(PropsIndex.DATE_PICKER_MAX) ?: 0L
        if (view.getProperty(PROP_MAX) != max) view.setProperty(PROP_MAX, max)

        val enabled = props.getBool(PropsIndex.DATE_PICKER_ENABLED, true)
        if (view.getProperty(PROP_ENABLED) != enabled) view.setProperty(PROP_ENABLED, enabled)
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        // DatePicker has no children.
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
        val handlerId = props.getHandler(PropsIndex.DATE_PICKER_ON_CHANGE)
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
        const val KIND: String = "datepicker"

        /** Builds a fresh [DatePickerAdapter] for one IR node (FLUX-007). */
        fun create(): FluxAdapter<FluxNativeView> = DatePickerAdapter()

        const val PROP_VALUE = "value"
        const val PROP_MIN = "min"
        const val PROP_MAX = "max"
        const val PROP_ENABLED = "enabled"
        const val PROP_HANDLER = "onChangeHandler"
        const val PROP_EXECUTOR = "executor"
    }
}
