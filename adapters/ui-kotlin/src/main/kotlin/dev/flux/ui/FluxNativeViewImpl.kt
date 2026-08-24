package dev.flux.ui

import kotlin.collections.List as KList

/**
 * A reference implementation of [FluxNativeView] used by the dev adapters and
 * by the kit's own tests.
 *
 * In the Android runtime this role is filled by a host-backed wrapper around a
 * real `android.view.View`; here, on plain JVM, it is a pure in-memory tree of
 * children plus a property bag. Adapters drive it through the [FluxNativeView]
 * primitives, so the same adapter code paths are exercised against a fake that
 * the acceptance criteria (FLUX-009) require.
 */
public class FluxNativeViewImpl(
    override val nodeId: UInt,
    override val kind: String,
) : FluxNativeView {
    private val childViews = mutableListOf<FluxNativeView>()
    private val properties = mutableMapOf<String, Any?>()

    override fun setChildAt(
        index: Int,
        view: FluxNativeView?,
    ) {
        require(index in 0..childViews.size) {
            "child index $index out of range for $kind#$nodeId (size ${childViews.size})"
        }
        if (view == null) {
            childViews.removeAt(index)
        } else if (index == childViews.size) {
            childViews.add(view)
        } else {
            childViews[index] = view
        }
    }

    override fun addChild(view: FluxNativeView) {
        childViews.add(view)
    }

    override fun removeChildAt(index: Int): FluxNativeView? {
        if (index < 0 || index >= childViews.size) return null
        return childViews.removeAt(index)
    }

    override fun children(): KList<FluxNativeView> = childViews.toList()

    override fun setProperty(
        property: String,
        value: Any?,
    ): Boolean {
        val previous = properties[property]
        if (previous == value) return false
        properties[property] = value
        return true
    }

    override fun getProperty(property: String): Any? = properties[property]

    /** Whether a child with [childId] currently exists in this view. */
    public fun hasChild(childId: UInt): Boolean = childViews.any { it.nodeId == childId }
}
