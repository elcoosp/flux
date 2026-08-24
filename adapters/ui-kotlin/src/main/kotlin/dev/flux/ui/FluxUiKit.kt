package dev.flux.ui

/**
 * Marker for the Flux Kotlin adapter kit.
 *
 * Adapters translate Flux IR nodes into native views; the props are the
 * contract (Appendix F). This placeholder exists only so the module compiles
 * before the kotlin-adapters agent (FLUX-009) lands the real adapters.
 */
public object FluxUiKit {
    /** The adapter contract version this kit implements (Appendix F). */
    public const val ADAPTER_CONTRACT_VERSION: Int = 1
}
