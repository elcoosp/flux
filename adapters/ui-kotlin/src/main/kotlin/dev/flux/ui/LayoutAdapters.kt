package dev.flux.ui

import java.lang.ref.WeakReference
import kotlin.collections.List as KList

/**
 * Declarative adapters for the FLUX-037 layout primitives (`Stack`, `Grid`,
 * `Spacer`, `SafeArea`), unified tier (AGENTS.md §3.5).
 *
 * Each maps a Flux layout node to a platform-neutral [FluxNativeView] carrying
 * the props the host renderer consumes. Children are reconciled by stable node
 * id (keyed reconciliation, §3.5) so reorders preserve child state. Each node
 * gets its own adapter instance via [create] (FLUX-007).
 */

/** `Stack` — z-order overlay of children (SwiftUI `ZStack` / Compose `Box`). */
public class StackAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val gap = props.getFloat(PropsIndex.STACK_GAP) ?: 0.0
        if (view.getProperty(PROP_GAP) != gap) view.setProperty(PROP_GAP, gap)
        // Stack paints children back-to-front; the host renderer uses `zOrder`.
        if (view.getProperty(PROP_Z_ORDER) != true) view.setProperty(PROP_Z_ORDER, true)
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        val byId = children.associateBy { it.nodeId }
        reconcileChildren(view, childIds) { byId[it] }
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
        const val KIND: String = "stack"
        const val PROP_GAP = "gap"
        const val PROP_Z_ORDER = "zOrder"

        fun create(): FluxAdapter<FluxNativeView> = StackAdapter()
    }
}

/**
 * `Grid` — two-dimensional responsive grid of children (Compose
 * `LazyVerticalGrid`). The [PropsIndex.GRID_COLUMNS] prop drives the column
 * count; the host renderer lays cells out in row-major order.
 */
public class GridAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val columns = props.getInt(PropsIndex.GRID_COLUMNS) ?: 2L
        if (view.getProperty(PROP_COLUMNS) != columns) view.setProperty(PROP_COLUMNS, columns)
        val gap = props.getFloat(PropsIndex.STACK_GAP) ?: 0.0
        if (view.getProperty(PROP_GAP) != gap) view.setProperty(PROP_GAP, gap)
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        val byId = children.associateBy { it.nodeId }
        reconcileChildren(view, childIds) { byId[it] }
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
        const val KIND: String = "grid"
        const val PROP_COLUMNS = "columns"
        const val PROP_GAP = "gap"

        fun create(): FluxAdapter<FluxNativeView> = GridAdapter()
    }
}

/**
 * `Spacer` — an elastic gap that grows to fill available space along the
 * parent's main axis (SwiftUI `Spacer` / Compose `Spacer`). The [PropsIndex
 * .SPACER_FLEX] prop is the relative grow weight.
 */
public class SpacerAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        val flex = props.getFloat(PropsIndex.SPACER_FLEX) ?: 1.0
        if (view.getProperty(PROP_FLEX) != flex) view.setProperty(PROP_FLEX, flex)
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        // A spacer carries no children.
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
        const val KIND: String = "spacer"
        const val PROP_FLEX = "flex"

        fun create(): FluxAdapter<FluxNativeView> = SpacerAdapter()
    }
}

/**
 * `SafeArea` — insets its children within the platform safe area (SwiftUI
 * `SafeArea` / Compose `Scaffold` content padding). The [PropsIndex
 * .SAFEAREA_EDGES] prop selects which edges to inset (`"top"` / `"bottom"` /
 * …); absent means all edges.
 */
public class SafeAreaAdapter private constructor() : FluxAdapter<FluxNativeView> {
    override val kind: String = KIND

    override fun create(nodeId: UInt): FluxNativeView = FluxNativeViewImpl(nodeId, kind)

    override fun update(
        view: FluxNativeView,
        props: Props,
    ) {
        props.getString(PropsIndex.SAFEAREA_EDGES)?.let { edges ->
            if (view.getProperty(PROP_EDGES) != edges) view.setProperty(PROP_EDGES, edges)
        } ?: run {
            if (view.getProperty(PROP_EDGES) != null) view.setProperty(PROP_EDGES, null)
        }
    }

    override fun setChildren(
        view: FluxNativeView,
        childIds: KList<UInt>,
        children: KList<FluxNativeView>,
    ) {
        val byId = children.associateBy { it.nodeId }
        reconcileChildren(view, childIds) { byId[it] }
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
        const val KIND: String = "safearea"
        const val PROP_EDGES = "edges"

        fun create(): FluxAdapter<FluxNativeView> = SafeAreaAdapter()
    }
}
