package dev.flux.ui

/**
 * Public surface of the Flux Kotlin adapter kit (FLUX-009).
 *
 * Mirrors the Swift `FluxUIKit` (FLUX-008). This object re-exports the kit's
 * public types and the adapter-contract version so consumers and tests import
 * everything from one place. Adapters translate Flux IR nodes into native
 * views; the props (read through [Props]) are the contract (Appendix F).
 *
 * The kit hands out **factories**, not shared adapter instances. Every IR node
 * resolves a fresh [FluxAdapter] through [factoryFor]/[adapterFor] so per-node
 * view state (a bound `WeakReference<FluxExecutor>`, a handler id) never leaks
 * into a sibling node — matching the Swift dev-runtime memory model and fixing
 * the delegate-leak brittleness (FLUX-007).
 */
public object FluxUiKit {
    /** The adapter contract version this kit implements (Appendix F). */
    public const val ADAPTER_CONTRACT_VERSION: Int = 1

    /** The 8 dev adapters, keyed by their IR node-kind tag, as factories. */
    public val adapters: Map<String, FluxAdapterFactory> =
        mapOf(
            TextAdapter.KIND to FluxAdapterFactory(TextAdapter::create),
            ButtonAdapter.KIND to FluxAdapterFactory(ButtonAdapter::create),
            ColumnAdapter.KIND to FluxAdapterFactory(ColumnAdapter::create),
            RowAdapter.KIND to FluxAdapterFactory(RowAdapter::create),
            TextFieldAdapter.KIND to FluxAdapterFactory(TextFieldAdapter::create),
            ScreenAdapter.KIND to FluxAdapterFactory(ScreenAdapter::create),
            RouterAdapter.KIND to FluxAdapterFactory(RouterAdapter::create),
            ImageAdapter.KIND to FluxAdapterFactory(ImageAdapter::create),
            ContainerAdapter.KIND to FluxAdapterFactory(ContainerAdapter::create),
        )

    /** Returns the factory registered for [kind], or `null`. */
    public fun factoryFor(kind: String): FluxAdapterFactory? = adapters[kind]

    /**
     * Resolves a fresh adapter for [kind], or `null` when no adapter handles
     * that kind tag. Each call builds a new instance.
     */
    public fun adapterFor(kind: String): FluxAdapter<FluxNativeView>? = adapters[kind]?.create()

    /** The set of kind tags the kit can render. */
    public fun kinds(): Set<String> = adapters.keys
}
