package dev.flux.ui

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class LinearAdapterTest {
    @Test
    fun `column sets vertical orientation and gap`() {
        val adapter = ColumnAdapter.create()
        val view = adapter.create(10u)
        adapter.update(view, propsOf(PropsIndex.STACK_GAP to FluxValue.Float(12.0)))
        assertEquals("vertical", view.getProperty(FluxLinearAdapter.PROP_ORIENTATION))
        assertEquals(12.0, view.getProperty(FluxLinearAdapter.PROP_GAP))
    }

    @Test
    fun `row sets horizontal orientation`() {
        val adapter = RowAdapter.create()
        val view = adapter.create(11u)
        adapter.update(view, propsOf(PropsIndex.STACK_GAP to FluxValue.Float(8.0)))
        assertEquals("horizontal", view.getProperty(FluxLinearAdapter.PROP_ORIENTATION))
    }

    @Test
    fun `column reconciles children by stable id preserving order`() {
        val adapter = ColumnAdapter.create()
        val view = adapter.create(12u)
        val a = FluxNativeViewImpl(100u, "text")
        val b = FluxNativeViewImpl(101u, "text")
        val c = FluxNativeViewImpl(102u, "text")
        adapter.setChildren(view, listOf(100u, 101u, 102u), listOf(a, b, c))
        assertEquals(listOf(100u, 101u, 102u), view.children().map { it.nodeId })

        // Reorder: existing views must NOT be recreated, only reordered.
        adapter.setChildren(view, listOf(102u, 100u), listOf(c, a))
        assertEquals(listOf(102u, 100u), view.children().map { it.nodeId })
        assertTrue(view.children()[0] === c)
        assertTrue(view.children()[1] === a)
    }

    @Test
    fun `column removes orphaned children`() {
        val adapter = ColumnAdapter.create()
        val view = adapter.create(13u)
        val a = FluxNativeViewImpl(10u, "text")
        val b = FluxNativeViewImpl(11u, "text")
        adapter.setChildren(view, listOf(10u, 11u), listOf(a, b))
        adapter.setChildren(view, listOf(10u), listOf(a))
        assertEquals(listOf(10u), view.children().map { it.nodeId })
    }

    @Test
    fun `destroy clears children`() {
        val adapter = ColumnAdapter.create()
        val view = adapter.create(14u)
        val a = FluxNativeViewImpl(20u, "text")
        adapter.setChildren(view, listOf(20u), listOf(a))
        adapter.destroy(view)
        assertEquals(0, view.childCount())
    }
}
