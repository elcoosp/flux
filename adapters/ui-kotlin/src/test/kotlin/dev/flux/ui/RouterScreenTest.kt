package dev.flux.ui

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertSame
import org.junit.jupiter.api.Test

class RouterScreenTest {
    @Test
    fun `screen hosts a single content child`() {
        val adapter = ScreenAdapter.create()
        val view = adapter.create(50u)
        val content = FluxNativeViewImpl(500u, "text")
        adapter.setChildren(view, listOf(500u), listOf(content))
        assertEquals(listOf(500u), view.children().map { it.nodeId })

        // Replacing the content child swaps it out.
        val content2 = FluxNativeViewImpl(501u, "text")
        adapter.setChildren(view, listOf(501u), listOf(content2))
        assertEquals(listOf(501u), view.children().map { it.nodeId })
    }

    @Test
    fun `router preserves existing screen view across push`() {
        val adapter = RouterAdapter.create()
        val router = adapter.create(60u)
        val home = FluxNativeViewImpl(600u, "screen")
        val settings = FluxNativeViewImpl(601u, "screen")

        // Initial stack: [home]
        adapter.setChildren(router, listOf(600u), listOf(home))
        assertEquals(listOf(600u), router.children().map { it.nodeId })

        // Push settings: home must keep its SAME instance (state preservation).
        adapter.setChildren(router, listOf(600u, 601u), listOf(home, settings))
        assertEquals(listOf(600u, 601u), router.children().map { it.nodeId })
        assertSame(home, router.children()[0])
    }

    @Test
    fun `router pop preserves pushed screen instance for re-push`() {
        val adapter = RouterAdapter.create()
        val router = adapter.create(61u)
        val home = FluxNativeViewImpl(610u, "screen")
        val detail = FluxNativeViewImpl(611u, "screen")

        adapter.setChildren(router, listOf(610u, 611u), listOf(home, detail))
        // Pop back to home.
        adapter.setChildren(router, listOf(610u), listOf(home))
        assertEquals(listOf(610u), router.children().map { it.nodeId })

        // Re-push detail: it must be the SAME view instance (state preserved).
        adapter.setChildren(router, listOf(610u, 611u), listOf(home, detail))
        assertSame(detail, router.children()[1])
    }

    @Test
    fun `router reconciliation reorders without recreating screens`() {
        val adapter = RouterAdapter.create()
        val router = adapter.create(62u)
        val a = FluxNativeViewImpl(620u, "screen")
        val b = FluxNativeViewImpl(621u, "screen")
        adapter.setChildren(router, listOf(620u, 621u), listOf(a, b))
        // Reorder only.
        adapter.setChildren(router, listOf(621u, 620u), listOf(b, a))
        assertEquals(listOf(621u, 620u), router.children().map { it.nodeId })
        assertSame(b, router.children()[0])
        assertSame(a, router.children()[1])
    }

    @Test
    fun `router destroy clears screen stack`() {
        val adapter = RouterAdapter.create()
        val router = adapter.create(63u)
        val home = FluxNativeViewImpl(630u, "screen")
        adapter.setChildren(router, listOf(630u), listOf(home))
        adapter.destroy(router)
        assertEquals(0, router.childCount())
    }
}
