package dev.flux.ui

/**
 * Public surface of the Flux Kotlin adapter kit (FLUX-009).
 *
 * Mirrors the Swift `FluxUIKit` (FLUX-008). This object re-exports the kit's
 * public types and the adapter-contract version so consumers and tests import
 * everything from one place. Adapters translate Flux IR nodes into native
 * the props (read through [Props]) are the contract (AGENTS.md §3.5).
 *
 * The kit hands out **factories**, not shared adapter instances. Every IR node
 * resolves a fresh [FluxAdapter] through [factoryFor]/[adapterFor] so per-node
 * view state (a bound `WeakReference<FluxExecutor>`, a handler id) never leaks
 * into a sibling node — matching the Swift dev-runtime memory model and fixing
 * the delegate-leak brittleness (FLUX-007).
 */
public object FluxUiKit {
    /** The adapter contract version this kit implements (contract version 1, per AGENTS.md §3.5). */
    public const val ADAPTER_CONTRACT_VERSION: Int = 1

    /** The 9 declarative adapters, keyed by their IR node-kind tag, as factories. */
    public val adapters: Map<String, FluxAdapterFactory> =
        mapOf(
            TextAdapter.KIND to FluxAdapterFactory(TextAdapter::create),
            ButtonAdapter.KIND to FluxAdapterFactory(ButtonAdapter::create),
            ColumnAdapter.KIND to FluxAdapterFactory(ColumnAdapter::create),
            RowAdapter.KIND to FluxAdapterFactory(RowAdapter::create),
            TextInputAdapter.KIND to FluxAdapterFactory(TextInputAdapter::create),
            ScreenAdapter.KIND to FluxAdapterFactory(ScreenAdapter::create),
            RouterAdapter.KIND to FluxAdapterFactory(RouterAdapter::create),
            ImageAdapter.KIND to FluxAdapterFactory(ImageAdapter::create),
            ContainerAdapter.KIND to FluxAdapterFactory(ContainerAdapter::create),
            // FLUX-037 layout primitives.
            StackAdapter.KIND to FluxAdapterFactory(StackAdapter::create),
            GridAdapter.KIND to FluxAdapterFactory(GridAdapter::create),
            SpacerAdapter.KIND to FluxAdapterFactory(SpacerAdapter::create),
            SafeAreaAdapter.KIND to FluxAdapterFactory(SafeAreaAdapter::create),
            // FLUX-038 overlay containers + FLUX-042 animation wrapper.
            ModalAdapter.KIND to FluxAdapterFactory(ModalAdapter::create),
            SheetAdapter.KIND to FluxAdapterFactory(SheetAdapter::create),
            DialogAdapter.KIND to FluxAdapterFactory(DialogAdapter::create),
            AnimateAdapter.KIND to FluxAdapterFactory(AnimateAdapter::create),
            // FLUX-040 form primitives (PRD-N family).
            SwitchAdapter.KIND to FluxAdapterFactory(SwitchAdapter::create),
            // FLUX-077 — `Toggle` (data-driven two-state control, FLUX-072).
            ToggleAdapter.KIND to FluxAdapterFactory(ToggleAdapter::create),
            CheckboxAdapter.KIND to FluxAdapterFactory(CheckboxAdapter::create),
            SliderAdapter.KIND to FluxAdapterFactory(SliderAdapter::create),
            PickerAdapter.KIND to FluxAdapterFactory(PickerAdapter::create),
            DatePickerAdapter.KIND to FluxAdapterFactory(DatePickerAdapter::create),
            TextAreaAdapter.KIND to FluxAdapterFactory(TextAreaAdapter::create),
            // FLUX-041 gesture primitive (PRD-N family).
            GestureAdapter.KIND to FluxAdapterFactory(GestureAdapter::create),
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
