package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Adapter for component nodes (user components such as `Counter`) and any
 * non-primitive container.
 *
 * Components carry no native presentation of their own — they are a named
 * subtree whose children do the real rendering — so this adapter backs them
 * with a neutral [FluxNativeView] that simply hosts their children
 * (a component is a container; AGENTS.md §3.5). This mirrors the iOS `ContainerAdapter`
 * (FLUX-008) and the SwiftUI dev runtime, which treats every `Component` node
 * as a plain container view.
 *
 * Without this fallback a component root (e.g. `Counter`) would fail adapter
 * resolution and the tree would not mount.
 *
 * Each node gets its own adapter instance via [create], so no per-node view
 * state leaks between sibling component roots (FLUX-007). `create` is the
 * degrade-to fallback the host uses when an unbound id resolves to no
 * primitive adapter.
 */
public class ContainerAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, "container")

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        // Containers have no presentational props; nothing to apply.
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        view.clearChildren()
        for (child in children) {
            view.addChild(child)
        }
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
        // Containers bind no native events.
    }

    override fun destroy(view: FluxNativeView) {
        view.clearChildren()
    }

    internal companion object {
        /** The kind tag this adapter handles. Exposed for the factory map. */
        const val KIND: String = "container"

        /** Builds a fresh [ContainerAdapter] for one IR node (FLUX-007). */
        fun create(): FluxAdapter<FluxNativeView> = ContainerAdapter()
    }
}
