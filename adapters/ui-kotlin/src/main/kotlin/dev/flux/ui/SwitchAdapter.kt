package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Declarative adapter for `Switch` (FLUX-040, unified tier; AGENTS.md §3.5).
 *
 * Maps a Flux `Switch` node to a native two-state toggle. The controlled
 * [value][PropsIndex.SWITCH_VALUE] is pushed on every [update]; when the user
 * flips the switch, the view dispatches the `onChange` handler (carrying the
 * new boolean) through the weakly-held executor. [enabled] gates interaction.
 *
 * Each node gets its own adapter instance via [create], so the bound
 * `WeakReference<FluxExecutor>` and handler id never leak into a sibling node
 * (FLUX-007).
 */
public class SwitchAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val value = props.getBool(PropsIndex.SWITCH_VALUE, false)
        if (view.getProperty(PROP_VALUE) != value) view.setProperty(PROP_VALUE, value)

        val enabled = props.getBool(PropsIndex.SWITCH_ENABLED, true)
        if (view.getProperty(PROP_ENABLED) != enabled) view.setProperty(PROP_ENABLED, enabled)
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        // Switch has no children.
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
        val handlerId = props.getHandler(PropsIndex.SWITCH_ON_CHANGE)
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
        const val KIND: String = "switch"

        /** Builds a fresh [SwitchAdapter] for one IR node (FLUX-007). */
        fun create(): FluxAdapter<FluxNativeView> = SwitchAdapter()

        const val PROP_VALUE = "value"
        const val PROP_ENABLED = "enabled"
        const val PROP_HANDLER = "onChangeHandler"
        const val PROP_EXECUTOR = "executor"
    }
}
