package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Declarative adapter for `WebHost` (FLUX-048, unified tier; AGENTS.md §3.5).
 *
 * Maps a Flux `WebHost` node to a native web view. The capability layer
 * (cap 12, `WebView.load`) writes the requested URL into signal 82; this
 * adapter is the declarative view that reads the same `src` prop and records it
 * onto the [FluxNativeView]. The Android runtime (FLUX-007) reads `PROP_SRC` and
 * mounts a sandboxed `android.webkit.WebView`, handling the navigation and
 * graceful fallback on failure. The two paths share one contract: the prop name
 * `src`, resolved by the FNV-1a index (AGENTS.md §3.2) — never a hardcoded
 * positional index.
 *
 * Security (FLUX-048 / ADR-0057): the web view is sandbox-contained — it runs in
 * its own process, cannot reach host APIs, and requires no OS permission
 * (`PermissionKind.None`).
 *
 * Each node gets its own adapter instance via [create], so the resolved source
 * never bleeds into a sibling (FLUX-007).
 */
public class WebViewAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val src = props.getString(PropsIndex.propIndexForName("src"))
        if (src.isNullOrEmpty()) {
            // A missing/empty `src` clears the source so the host hides the view.
            if (view.getProperty(PROP_SRC) != null) view.setProperty(PROP_SRC, null)
            if (view.getProperty(PROP_HAS_SRC) != false) view.setProperty(PROP_HAS_SRC, false)
            return
        }
        // Avoid re-recording an unchanged URL (the runtime skips no-op mounts).
        if (view.getProperty(PROP_SRC) != src) view.setProperty(PROP_SRC, src)
        if (view.getProperty(PROP_HAS_SRC) != true) view.setProperty(PROP_HAS_SRC, true)
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        // WebHost is a leaf; the runtime never sends children.
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
        // WebHost has no handlers in the MLP.
    }

    override fun destroy(view: FluxNativeView) {
        view.setProperty(PROP_SRC, null)
        view.setProperty(PROP_HAS_SRC, false)
        view.clearChildren()
    }

    internal companion object {
        /** The kind tag this adapter handles. Exposed for the factory map. */
        const val KIND: String = "webhost"

        /** Builds a fresh [WebViewAdapter] for one IR node (FLUX-007). */
        fun create(): FluxAdapter<FluxNativeView> = WebViewAdapter()

        const val PROP_SRC = "webSrc"
        const val PROP_HAS_SRC = "hasWebSrc"
    }
}
