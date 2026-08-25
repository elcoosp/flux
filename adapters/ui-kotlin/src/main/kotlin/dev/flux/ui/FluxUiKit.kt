package dev.flux.ui

/**
 * Public surface of the Flux Kotlin adapter kit (FLUX-009).
 *
 * Mirrors the Swift `FluxUIKit` (FLUX-008). This object re-exports the kit's
 * public types and the adapter-contract version so consumers and tests import
 * everything from one place. Adapters translate Flux IR nodes into native
 * views; the props (read through [Props]) are the contract (Appendix F).
 */
public object FluxUiKit {
    /** The adapter contract version this kit implements (Appendix F). */
    public const val ADAPTER_CONTRACT_VERSION: Int = 1

    /** The 7 dev adapters, keyed by their IR node-kind tag. */
    public val adapters: Map<String, FluxAdapter<out FluxNativeView>> =
        mapOf(
            TextAdapter().kind to TextAdapter(),
            ButtonAdapter().kind to ButtonAdapter(),
            ColumnAdapter().kind to ColumnAdapter(),
            RowAdapter().kind to RowAdapter(),
            TextFieldAdapter().kind to TextFieldAdapter(),
            ScreenAdapter().kind to ScreenAdapter(),
            RouterAdapter().kind to RouterAdapter(),
            ImageAdapter().kind to ImageAdapter(),
        )

    /** Returns the dev adapter registered for [kind], or `null`. */
    public fun adapterFor(kind: String): FluxAdapter<out FluxNativeView>? = adapters[kind]
}
