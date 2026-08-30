package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Declarative adapter for `Slider` (FLUX-040, unified tier; AGENTS.md §3.5).
 *
 * Maps a Flux `Slider` node to a native continuous-value selector. The
 * controlled [value][PropsIndex.SLIDER_VALUE] is pushed on every [update];
 * when the user drags the thumb, the view dispatches the `onChange` handler
 * (carrying the new float) through the weakly-held executor. The
 * [min][PropsIndex.SLIDER_MIN]/[max][PropsIndex.SLIDER_MAX]/
 * [step][PropsIndex.SLIDER_STEP] bounds are surfaced as host-render-only
 * properties; [enabled] gates interaction.
 *
 * Each node gets its own adapter instance via [create] (FLUX-007).
 */
public class SliderAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val value = props.getFloat(PropsIndex.SLIDER_VALUE) ?: 0.0
        if (view.getProperty(PROP_VALUE) != value) view.setProperty(PROP_VALUE, value)

        val min = props.getFloat(PropsIndex.SLIDER_MIN) ?: 0.0
        if (view.getProperty(PROP_MIN) != min) view.setProperty(PROP_MIN, min)

        val max = props.getFloat(PropsIndex.SLIDER_MAX) ?: 1.0
        if (view.getProperty(PROP_MAX) != max) view.setProperty(PROP_MAX, max)

        val step = props.getFloat(PropsIndex.SLIDER_STEP) ?: 0.0
        if (view.getProperty(PROP_STEP) != step) view.setProperty(PROP_STEP, step)

        val enabled = props.getBool(PropsIndex.SLIDER_ENABLED, true)
        if (view.getProperty(PROP_ENABLED) != enabled) view.setProperty(PROP_ENABLED, enabled)
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        // Slider has no children.
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
        val handlerId = props.getHandler(PropsIndex.SLIDER_ON_CHANGE)
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
        const val KIND: String = "slider"

        /** Builds a fresh [SliderAdapter] for one IR node (FLUX-007). */
        fun create(): FluxAdapter<FluxNativeView> = SliderAdapter()

        const val PROP_VALUE = "value"
        const val PROP_MIN = "min"
        const val PROP_MAX = "max"
        const val PROP_STEP = "step"
        const val PROP_ENABLED = "enabled"
        const val PROP_HANDLER = "onChangeHandler"
        const val PROP_EXECUTOR = "executor"
    }
}
