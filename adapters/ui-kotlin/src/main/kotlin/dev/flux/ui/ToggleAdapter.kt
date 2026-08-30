package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Declarative adapter for `Toggle` (FLUX-077, unified tier; AGENTS.md §3.5).
 *
 * `Toggle` is the two-state boolean control used by the data-driven surface
 * (FLUX-072) — `examples/todo` renders `TaskRow` with
 * `Toggle value: task.done, onValueChange: fn(v) { … }`. The controlled
 * [value][PropsIndex.TOGGLE_VALUE] is pushed on every [update]; when the user
 * flips the switch, the view dispatches the `onValueChange` handler (carrying
 * the new boolean) through the weakly-held executor. [enabled] gates
 * interaction.
 *
 * This is the Android half of the FLUX-077 parity set. The kit previously had
 * no `toggle` kind (the prelude/codegen seed it but no adapter existed on
 * either platform), so the same node degraded to a blank container on iOS. We
 * mirror the [SwitchAdapter] contract exactly — `value` + `onChange` +
 * `enabled` — but read the `onValueChange` handler prop the `Toggle` compo
 * actually emits (see `flux-codegen-core/src/primitives.rs` + `examples/todo`).
 *
 * Each node gets its own adapter instance via [create], so the bound
 * `WeakReference<FluxExecutor>` and handler id never leak into a sibling node
 * (FLUX-007).
 */
public class ToggleAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val value = props.getBool(PropsIndex.TOGGLE_VALUE, false)
        if (view.getProperty(PROP_VALUE) != value) view.setProperty(PROP_VALUE, value)

        val enabled = props.getBool(PropsIndex.TOGGLE_ENABLED, true)
        if (view.getProperty(PROP_ENABLED) != enabled) view.setProperty(PROP_ENABLED, enabled)
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        // Toggle has no children.
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
        val handlerId = props.getHandler(PropsIndex.TOGGLE_ON_VALUE_CHANGE)
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
        const val KIND: String = "toggle"

        /** Builds a fresh [ToggleAdapter] for one IR node (FLUX-007). */
        fun create(): FluxAdapter<FluxNativeView> = ToggleAdapter()

        const val PROP_VALUE = "value"
        const val PROP_ENABLED = "enabled"
        const val PROP_HANDLER = "onValueChangeHandler"
        const val PROP_EXECUTOR = "executor"
    }
}
