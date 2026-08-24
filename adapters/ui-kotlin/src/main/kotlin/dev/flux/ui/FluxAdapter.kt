package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * The contract every dev adapter implements (Appendix F).
 *
 * An adapter maps one IR node kind to a native view: it [create]s the backing
 * [FluxNativeView], applies decoded [Props] in [update], manages the
 * native child list in [setChildren], binds user events through an executor
 * [WeakReference] in [bindHandler], and tears down in [destroy].
 *
 * The weak executor reference is load-bearing: adapters are owned by the host
 * shadow tree, and the shadow tree outlives individual executor instances
 * across hot-swaps. A strong reference here would pin a stale executor and
 * leak the entire signal graph.
 *
 * @param V The concrete native-view type this adapter produces.
 */
public interface FluxAdapter<V : FluxNativeView> {
    /** The component-local kind tag this adapter handles (e.g. "text"). */
    val kind: String

    /**
     * Creates a fresh native view for [nodeId]. The returned view is empty;
     * [update] and [setChildren] populate it.
     */
    fun create(nodeId: UInt): V

    /**
     * Applies [props] to [view], transitioning it from its previous state.
     * Implementations should compare against the previous props and skip
     * no-op writes so hot-swapped patches do not thrash native state.
     */
    fun update(
        view: V,
        props: Props,
    )

    /**
     * Reconciles [view]'s child list to match [children] using stable
     * [childIds] for keyed diffing. Adapters delegate to [reconcileChildren];
     * the adapter supplies the factory that builds a [FluxNativeView] for a
     * child id the reconciler has not seen.
     */
    fun setChildren(
        view: V,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    )

    /**
     * Binds the handler identified in [props] to native events on [view].
     * [executor] is held weakly; adapters must consult it through
     * [WeakReference.get] and no-op when it is `null` (executor disposed).
     */
    fun bindHandler(
        view: V,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    )

    /** Releases native resources owned by [view]. */
    fun destroy(view: V)
}
