package dev.flux.ui

import kotlin.collections.List as KList

/**
 * The platform-neutral native-view abstraction that every dev adapter drives.
 *
 * The adapter kit compiles on plain JVM (the frozen `build.gradle.kts` has no
 * Android dependency), so it cannot reference `android.view.View` directly.
 * Instead, adapters operate on this interface. In the Android runtime
 * (FLUX-007) `FluxNativeView` is backed by a `FluxViewHost` wrapping a real
 * `android.view.View`; the dev-server emits these wrappers and the adapter
 * mutates them through the typed primitives below. All view state changes
 * funnel through here so the host owns the actual `View` subtree and can keep
 * it in sync with the signal graph.
 */
public interface FluxNativeView {
    /** The stable IR node id this view was created from. */
    val nodeId: UInt

    /** The component-local kind tag (e.g. "text", "button", "screen"). */
    val kind: String

    /** Replaces [view] in this view's child list at [index]. */
    fun setChildAt(
        index: Int,
        view: FluxNativeView?,
    )

    /** Appends [view] as the last child. */
    fun addChild(view: FluxNativeView)

    /** Removes the child at [index], returning it (or `null` if out of range). */
    fun removeChildAt(index: Int): FluxNativeView?

    /** The view's children in visual order. */
    fun children(): KList<FluxNativeView>

    /** Current child count. */
    fun childCount(): Int = children().size

    /** Removes all children. */
    fun clearChildren() {
        while (childCount() > 0) {
            removeChildAt(childCount() - 1)
        }
    }

    /**
     * Applies a primitive mutation. Adapters declare intent (text, color, ...)
     * and the host translates it onto the backing native view. Returns `true`
     * if the value actually changed, `false` if it was already equal — adapters
     * use the result to short-circuit redundant downstream work.
     */
    fun setProperty(
        property: String,
        value: Any?,
    ): Boolean

    /** Reads the last value set for [property], or `null`. */
    fun getProperty(property: String): Any?

    /**
     * Applies FLUX-044 accessibility props (`label`, `role`, `focusOrder`) to
     * the native view's accessibility element.
     *
     * These props are host-render-only (no wire field); the dev server never
     * sends them as a distinct field, so they are resolved by name from the
     * same FNV-1a index space the server uses for every prop (AGENTS.md §3.2).
     * Missing props are no-ops (degrade to default, never throw — §3.5).
     */
    fun applyAccessibility(props: Props) {
        props.getString(PropsIndex.A11Y_LABEL)?.let { setProperty(PROP_ACCESSIBILITY_LABEL, it) }
        props.getString(PropsIndex.A11Y_ROLE)?.let { setProperty(PROP_ACCESSIBILITY_ROLE, it) }
        props.getString(PropsIndex.A11Y_FOCUS_ORDER)?.let { setProperty(PROP_ACCESSIBILITY_FOCUS_ORDER, it) }
    }

    /** Accessibility element property keys written by [applyAccessibility]. */
    companion object {
        const val PROP_ACCESSIBILITY_LABEL: String = "accessibilityLabel"
        const val PROP_ACCESSIBILITY_ROLE: String = "accessibilityRole"
        const val PROP_ACCESSIBILITY_FOCUS_ORDER: String = "accessibilityFocusOrder"
    }
}
