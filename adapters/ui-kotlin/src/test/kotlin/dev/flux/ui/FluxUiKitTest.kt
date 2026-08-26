package dev.flux.ui

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNotSame
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

class FluxUiKitTest {
    @Test
    fun `adapter contract version matches appendix F`() {
        assertEquals(1, FluxUiKit.ADAPTER_CONTRACT_VERSION)
    }

    @Test
    fun `kit exposes a factory map, not shared singletons`() {
        // The public surface is now a factory map keyed by kind tag.
        val factory: FluxAdapterFactory? = FluxUiKit.adapters["text"]
        assertEquals(true, factory != null, "text kind must be registered")
        // Resolving the same kind twice yields distinct adapter instances.
        val first = FluxUiKit.adapterFor("text")
        val second = FluxUiKit.adapterFor("text")
        assertNotSame(first, second, "each resolve must build a fresh adapter (FLUX-007)")
        assertEquals("text", first?.kind)
    }

    @Test
    fun `two resolves for the same id return distinct adapter instances`() {
        // The brittleness fix: no shared singleton state across nodes.
        val a = FluxUiKit.adapterFor("button")
        val b = FluxUiKit.adapterFor("button")
        assertEquals(true, a != null && b != null)
        assertNotSame(a, b, "same kind id must not share an adapter instance")
        // Distinct instances still carry the same contract.
        assertEquals(a?.kind, b?.kind)
    }

    @Test
    fun `unknown kind resolves to null`() {
        assertNull(FluxUiKit.adapterFor("does-not-exist"))
    }

    @Test
    fun `every registered kind builds a fresh instance`() {
        for (kind in FluxUiKit.kinds()) {
            val x = FluxUiKit.adapterFor(kind)
            val y = FluxUiKit.adapterFor(kind)
            assertEquals(true, x != null, "kind $kind must resolve")
            assertNotSame(x, y, "kind $kind must not share an instance")
        }
    }
}
