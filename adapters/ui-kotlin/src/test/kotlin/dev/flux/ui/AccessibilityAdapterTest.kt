package dev.flux.ui

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

/**
 * FLUX-044 — accessibility props (`label`, `role`, `focusOrder`) are
 * host-render-only and resolved by name from the same FNV-1a index space the
 * dev server uses for every prop (AGENTS.md §3.2). They carry no wire field of
 * their own, so a leaf adapter must surface them onto the native view's
 * accessibility element and never throw when they are absent.
 */
class AccessibilityAdapterTest {
    @Test
    fun `text adapter surfaces a11y label role and focusOrder`() {
        val adapter = TextAdapter.create()
        val view = adapter.create(10u)
        val props = propsOf(
            PropsIndex.TEXT_TEXT to FluxValue.Str("Count"),
            PropsIndex.A11Y_LABEL to FluxValue.Str("Tap count"),
            PropsIndex.A11Y_ROLE to FluxValue.Str("header"),
            PropsIndex.A11Y_FOCUS_ORDER to FluxValue.Str("3"),
        )
        adapter.update(view, props)
        assertEquals("Tap count", view.getProperty(FluxNativeView.PROP_ACCESSIBILITY_LABEL))
        assertEquals("header", view.getProperty(FluxNativeView.PROP_ACCESSIBILITY_ROLE))
        assertEquals("3", view.getProperty(FluxNativeView.PROP_ACCESSIBILITY_FOCUS_ORDER))
    }

    @Test
    fun `missing a11y props are a no-op`() {
        val view = FluxNativeViewImpl(11u, "text")
        view.applyAccessibility(propsOf(PropsIndex.TEXT_TEXT to FluxValue.Str("Hi")))
        assertNull(view.getProperty(FluxNativeView.PROP_ACCESSIBILITY_LABEL))
        assertNull(view.getProperty(FluxNativeView.PROP_ACCESSIBILITY_ROLE))
        assertNull(view.getProperty(FluxNativeView.PROP_ACCESSIBILITY_FOCUS_ORDER))
    }

    @Test
    fun `a11y indices are derived from prop names not positions`() {
        // The dev server hashes the prop *name*, so the constant must match
        // `flux_ir::lower::prop_index_for_name("label" | "role" | "focusOrder")`.
        assertEquals(PropsIndex.propIndexForName("label"), PropsIndex.A11Y_LABEL)
        assertEquals(PropsIndex.propIndexForName("role"), PropsIndex.A11Y_ROLE)
        assertEquals(PropsIndex.propIndexForName("focusOrder"), PropsIndex.A11Y_FOCUS_ORDER)
    }

    @Test
    fun `button adapter surfaces a11y props`() {
        val adapter = ButtonAdapter.create()
        val view = adapter.create(12u)
        val props = propsOf(
            PropsIndex.BUTTON_TEXT to FluxValue.Str("Go"),
            PropsIndex.A11Y_LABEL to FluxValue.Str("Navigate"),
        )
        adapter.update(view, props)
        assertEquals("Navigate", view.getProperty(FluxNativeView.PROP_ACCESSIBILITY_LABEL))
    }
}
