package dev.flux.ui

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class LayoutOverlayAdapterTest {
    @Test
    fun `stack sets z-order and gap`() {
        val adapter = StackAdapter.create()
        val view = adapter.create(20u)
        adapter.update(view, propsOf(PropsIndex.STACK_GAP to FluxValue.Float(8.0)))
        assertEquals(8.0, view.getProperty(StackAdapter.PROP_GAP))
        assertEquals(true, view.getProperty(StackAdapter.PROP_Z_ORDER))
    }

    @Test
    fun `grid sets column count and gap`() {
        val adapter = GridAdapter.create()
        val view = adapter.create(21u)
        adapter.update(
            view,
            propsOf(
                PropsIndex.GRID_COLUMNS to FluxValue.Int(3L),
                PropsIndex.STACK_GAP to FluxValue.Float(4.0),
            ),
        )
        assertEquals(3L, view.getProperty(GridAdapter.PROP_COLUMNS))
        assertEquals(4.0, view.getProperty(GridAdapter.PROP_GAP))
    }

    @Test
    fun `spacer sets flex weight`() {
        val adapter = SpacerAdapter.create()
        val view = adapter.create(22u)
        adapter.update(view, propsOf(PropsIndex.SPACER_FLEX to FluxValue.Float(2.0)))
        assertEquals(2.0, view.getProperty(SpacerAdapter.PROP_FLEX))
    }

    @Test
    fun `safearea records selected edges`() {
        val adapter = SafeAreaAdapter.create()
        val view = adapter.create(23u)
        adapter.update(view, propsOf(PropsIndex.SAFEAREA_EDGES to FluxValue.Str("top")))
        assertEquals("top", view.getProperty(SafeAreaAdapter.PROP_EDGES))
    }

    @Test
    fun `stack reconciles children by stable id`() {
        val adapter = StackAdapter.create()
        val view = adapter.create(24u)
        val a = FluxNativeViewImpl(200u, "text")
        val b = FluxNativeViewImpl(201u, "text")
        adapter.setChildren(view, listOf(200u, 201u), listOf(a, b))
        assertEquals(listOf(200u, 201u), view.children().map { it.nodeId })
        adapter.setChildren(view, listOf(201u), listOf(b))
        assertEquals(listOf(201u), view.children().map { it.nodeId })
    }

    @Test
    fun `modal records onDismiss handler id`() {
        val adapter = ModalAdapter.create()
        val view = adapter.create(25u)
        adapter.update(view, propsOf(PropsIndex.OVERLAY_ON_DISMISS to FluxValue.HandlerRef(7u)))
        assertEquals(7u, view.getProperty(ModalAdapter.PROP_ON_DISMISS))
    }

    @Test
    fun `animate records signal curve and duration`() {
        val adapter = AnimateAdapter.create()
        val view = adapter.create(26u)
        adapter.update(
            view,
            propsOf(
                PropsIndex.ANIMATE_CURVE to FluxValue.Str("spring"),
                PropsIndex.ANIMATE_DURATION to FluxValue.Float(0.3),
            ),
        )
        assertEquals("spring", view.getProperty(AnimateAdapter.PROP_CURVE))
        assertEquals(0.3, view.getProperty(AnimateAdapter.PROP_DURATION))
    }

    @Test
    fun `scrollview records orientation axis`() {
        val adapter = ScrollViewAdapter.create()
        val view = adapter.create(27u)
        adapter.update(view, propsOf(PropsIndex.SCROLL_ORIENTATION to FluxValue.Str("horizontal")))
        assertEquals("horizontal", view.getProperty(ScrollViewAdapter.PROP_ORIENTATION))
    }

    @Test
    fun `every flux-037-042-056 adapter resolves from the kit registry`() {
        for (kind in listOf("stack", "grid", "spacer", "safearea", "modal", "sheet", "dialog", "animate", "scrollview")) {
            assertTrue(FluxUiKit.adapterFor(kind) != null, "registry must resolve $kind")
        }
    }
}
