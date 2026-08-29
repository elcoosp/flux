package dev.flux.ui

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

/**
 * Parity test for [WebViewAdapter] (FLUX-048): `update` must record the `src`
 * prop onto the [FluxNativeView] so the runtime mounts a sandboxed web view,
 * and must clear it on a missing/empty `src`. The prop index is derived by name
 * (FNV-1a, AGENTS.md §3.2) — never hardcoded.
 */
class WebViewAdapterTest {
    private val srcIndex = PropsIndex.propIndexForName("src")

    @Test
    fun `records resolved src onto the view`() {
        val adapter = WebViewAdapter.create()
        val view = adapter.create(1u)
        adapter.update(view, stringProps(srcIndex, "https://example.com"))
        assertEquals("https://example.com", view.getProperty(WebViewAdapter.PROP_SRC))
        assertEquals(true, view.getProperty(WebViewAdapter.PROP_HAS_SRC))
    }

    @Test
    fun `clears src when missing or empty`() {
        val adapter = WebViewAdapter.create()
        val view = adapter.create(2u)
        // Set, then clear.
        adapter.update(view, stringProps(srcIndex, "https://example.com"))
        adapter.update(view, Props.EMPTY)
        assertNull(view.getProperty(WebViewAdapter.PROP_SRC))
        assertEquals(false, view.getProperty(WebViewAdapter.PROP_HAS_SRC))
    }
}
