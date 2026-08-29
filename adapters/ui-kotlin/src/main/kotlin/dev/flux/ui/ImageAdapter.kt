package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Declarative adapter for `Image` (unified tier; AGENTS.md §3.5).
 *
 * Maps a Flux `Image` node to a native image view, loading the bitmap from the
 * dev server's asset route (`http://localhost:7332/assets/<src>`). The adapter
 * is platform-neutral: it does not touch `android.*` directly, instead recording
 * the resolved request as [PROP_SRC] (and the optional `width`/`height`/
 * `contentMode` props) onto the [FluxNativeView]. The Android runtime (FLUX-007)
 * reads these properties and renders the actual `Image`/`painter`, handling the
 * HTTP fetch and graceful fallback to a placeholder (BR-003) on failure.
 *
 * Each node gets its own adapter instance via [create], so the resolved source
 * and dimensions never bleed into a sibling image (FLUX-007).
 *
 * Props are read by name; the index is the FNV-1a-32 digest of the name masked
 * to `u16` ([PropsIndex.propIndexForName]), derived identically on server and
 * client (AGENTS.md §3.2) — never a hardcoded positional index. Fields:
 * - `source: String` (required) — asset path relative to the project root.
 * - `width: Option[Float]`
 * - `height: Option[Float]`
 * - `resizeMode: Option[String]` — `"fill"` (default), `"fit"`, `"stretch"`.
 */
public class ImageAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val src = props.getString(PropsIndex.IMAGE_SOURCE)
        if (src.isNullOrEmpty()) {
            // A missing/empty `source` is a load failure: clear the source so the
            // host shows its placeholder rather than a stale bitmap (BR-003).
            if (view.getProperty(PROP_SRC) != null) view.setProperty(PROP_SRC, null)
            if (view.getProperty(PROP_HAS_SRC) != false) view.setProperty(PROP_HAS_SRC, false)
            return
        }
        if (view.getProperty(PROP_SRC) != src) view.setProperty(PROP_SRC, src)
        if (view.getProperty(PROP_HAS_SRC) != true) view.setProperty(PROP_HAS_SRC, true)

        val width = props.getFloat(PropsIndex.IMAGE_WIDTH)
        if (view.getProperty(PROP_WIDTH) != width) view.setProperty(PROP_WIDTH, width)

        val height = props.getFloat(PropsIndex.IMAGE_HEIGHT)
        if (view.getProperty(PROP_HEIGHT) != height) view.setProperty(PROP_HEIGHT, height)

        val mode = props.getString(PropsIndex.IMAGE_RESIZE_MODE) ?: DEFAULT_RESIZE_MODE
        if (view.getProperty(PROP_RESIZE_MODE) != mode) view.setProperty(PROP_RESIZE_MODE, mode)
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        // Image is a leaf; the runtime never sends children.
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
        // Image has no handlers.
    }

    override fun destroy(view: FluxNativeView) {
        view.setProperty(PROP_SRC, null)
        view.setProperty(PROP_HAS_SRC, false)
        view.clearChildren()
    }

    internal companion object {
        /** The kind tag this adapter handles. Exposed for the factory map. */
        const val KIND: String = "image"

        /** Builds a fresh [ImageAdapter] for one IR node (FLUX-007). */
        fun create(): FluxAdapter<FluxNativeView> = ImageAdapter()

        const val PROP_SRC = "imageSource"
        const val PROP_HAS_SRC = "hasImageSource"
        const val PROP_WIDTH = "imageWidth"
        const val PROP_HEIGHT = "imageHeight"
        const val PROP_RESIZE_MODE = "imageResizeMode"
        const val DEFAULT_RESIZE_MODE = "fill"
    }
}
