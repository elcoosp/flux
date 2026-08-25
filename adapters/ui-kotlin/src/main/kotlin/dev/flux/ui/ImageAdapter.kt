package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Dev adapter for `Image` (Appendix F.8).
 *
 * Maps a Flux `Image` node to a native image view, loading the bitmap from the
 * dev server's asset route (`http://localhost:7332/assets/<src>`). The adapter
 * is platform-neutral: it does not touch `android.*` directly, instead recording
 * the resolved request as [PROP_SRC] (and the optional `width`/`height`/
 * `contentMode` props) onto the [FluxNativeView]. The Android runtime (FLUX-007)
 * reads these properties and renders the actual `Image`/`painter`, handling the
 * HTTP fetch and graceful fallback to a placeholder (BR-003) on failure.
 *
 * Prop fields and their `PropIdx` (Appendix F.8 contract):
 * - `0 src: String` (required) — asset path relative to the project root.
 * - `1 width: Option[Float]`
 * - `2 height: Option[Float]`
 * - `3 contentMode: Option[String]` — `"fill"` (default), `"fit"`, `"stretch"`.
 */
public class ImageAdapter : FluxAdapter<FluxNativeView> {
    override val kind: String = "image"

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val src = props.getString(PropsIndex.IMAGE_SRC)
        if (src.isNullOrEmpty()) {
            // A missing/empty `src` is a load failure: clear the source so the
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

        val mode = props.getString(PropsIndex.IMAGE_CONTENT_MODE) ?: DEFAULT_CONTENT_MODE
        if (view.getProperty(PROP_CONTENT_MODE) != mode) view.setProperty(PROP_CONTENT_MODE, mode)
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
        const val PROP_SRC = "imageSrc"
        const val PROP_HAS_SRC = "hasImageSrc"
        const val PROP_WIDTH = "imageWidth"
        const val PROP_HEIGHT = "imageHeight"
        const val PROP_CONTENT_MODE = "imageContentMode"
        const val DEFAULT_CONTENT_MODE = "fill"
    }
}
