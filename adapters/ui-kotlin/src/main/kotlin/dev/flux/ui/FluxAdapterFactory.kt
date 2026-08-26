package dev.flux.ui

/**
 * A per-node factory for a single adapter kind.
 *
 * The kit historically handed out shared singleton adapter instances (one per
 * kind). That model leaks: delegates and view-bound state captured in an
 * adapter survive across nodes that reuse the same instance, so a stale
 * `WeakReference<FluxExecutor>` or handler id from one node can bleed into
 * another. `FluxUiKit` therefore exposes one [FluxAdapterFactory] per kind
 * instead of a single instance; every node resolves a *fresh* adapter through
 * [create] (mirrors the Swift dev-runtime memory model, FLUX-008).
 *
 * Implementations are cheap and side-effect-free; building one adapter per
 * resolution is intended and keeps per-node state isolated.
 */
public fun interface FluxAdapterFactory {
    /** Builds a brand-new adapter instance for one IR node. */
    public fun create(): FluxAdapter<FluxNativeView>
}
