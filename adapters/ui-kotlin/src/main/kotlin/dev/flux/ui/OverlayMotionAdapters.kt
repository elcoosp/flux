package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Declarative adapters for the FLUX-038 overlay containers (`Modal` / `Sheet` /
 * `Dialog`) and the FLUX-042 signal-graph animation wrapper (`Animate`),
 * unified tier (AGENTS.md §3.5).
 *
 * Each hosts its `content` / animated subtree as children and binds any
 * `onDismiss` handler. The native *presentation* (a hosted sheet / alert /
 * dialog) and the native *animation* (`withAnimation`) are gated on the ADR-0048
 * iOS dev-tier convergence decision — until then these adapters degrade to a
 * plain container carrying the children, so a Flux app can author and render the
 * primitives today without a blank screen (the dev/release parity mapping is
 * already pinned by `flux-parity`).
 *
 * Each node gets its own adapter instance via [create] (FLUX-007).
 */

/** `Modal` — centered modal over a scrim (Compose `Dialog`). */
public class ModalAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        // Presentation is host-native (ADR-0048); we record the onDismiss handler
        // id so `bindHandler` can wire it once the native surface lands.
        val onDismiss = props.getHandler(PropsIndex.OVERLAY_ON_DISMISS)
        if (view.getProperty(PROP_ON_DISMISS) != onDismiss) view.setProperty(PROP_ON_DISMISS, onDismiss)
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        reconcileChildren(view, childIds) { id -> children.firstOrNull { it.nodeId == id } }
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
        // No native event binding yet; the onDismiss callback is invoked by the
        // host presentation layer once ADR-0048 lands.
    }

    override fun destroy(view: FluxNativeView) {
        view.clearChildren()
    }

    internal companion object {
        const val KIND: String = "modal"
        const val PROP_ON_DISMISS = "onDismiss"

        fun create(): FluxAdapter<FluxNativeView> = ModalAdapter()
    }
}

/** `Sheet` — bottom-anchored sheet that slides up (Compose `ModalBottomSheet`). */
public class SheetAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val onDismiss = props.getHandler(PropsIndex.OVERLAY_ON_DISMISS)
        if (view.getProperty(PROP_ON_DISMISS) != onDismiss) view.setProperty(PROP_ON_DISMISS, onDismiss)
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        reconcileChildren(view, childIds) { id -> children.firstOrNull { it.nodeId == id } }
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
    }

    override fun destroy(view: FluxNativeView) {
        view.clearChildren()
    }

    internal companion object {
        const val KIND: String = "sheet"
        const val PROP_ON_DISMISS = "onDismiss"

        fun create(): FluxAdapter<FluxNativeView> = SheetAdapter()
    }
}

/** `Dialog` — modal dialog with a dimmed scrim (Compose `AlertDialog`). */
public class DialogAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val onDismiss = props.getHandler(PropsIndex.OVERLAY_ON_DISMISS)
        if (view.getProperty(PROP_ON_DISMISS) != onDismiss) view.setProperty(PROP_ON_DISMISS, onDismiss)
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        reconcileChildren(view, childIds) { id -> children.firstOrNull { it.nodeId == id } }
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
    }

    override fun destroy(view: FluxNativeView) {
        view.clearChildren()
    }

    internal companion object {
        const val KIND: String = "dialog"
        const val PROP_ON_DISMISS = "onDismiss"

        fun create(): FluxAdapter<FluxNativeView> = DialogAdapter()
    }
}

/**
 * `Animate` — signal-graph animation wrapper (FLUX-042). Hosts its child subtree
 * and records the `signal` / `curve` / `duration` data the host consumes to
 * drive the native `withAnimation` (ADR-0048). Until the native animation API is
 * wired, the children render unchanged — the node resolves (no blank screen)
 * and the curve data is carried on the view for the host layer.
 */
public class AnimateAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val signal = props.getHandler(PropsIndex.ANIMATE_SIGNAL)
        if (view.getProperty(PROP_SIGNAL) != signal) view.setProperty(PROP_SIGNAL, signal)
        props.getString(PropsIndex.ANIMATE_CURVE)?.let { curve ->
            if (view.getProperty(PROP_CURVE) != curve) view.setProperty(PROP_CURVE, curve)
        }
        props.getFloat(PropsIndex.ANIMATE_DURATION)?.let { duration ->
            if (view.getProperty(PROP_DURATION) != duration) view.setProperty(PROP_DURATION, duration)
        }
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        reconcileChildren(view, childIds) { id -> children.firstOrNull { it.nodeId == id } }
    }

    override fun bindHandler(
        view: FluxNativeView,
        props: Props,
        executor: WeakReference<FluxExecutor>,
    ) {
    }

    override fun destroy(view: FluxNativeView) {
        view.clearChildren()
    }

    internal companion object {
        const val KIND: String = "animate"
        const val PROP_SIGNAL = "signal"
        const val PROP_CURVE = "curve"
        const val PROP_DURATION = "duration"

        fun create(): FluxAdapter<FluxNativeView> = AnimateAdapter()
    }
}
