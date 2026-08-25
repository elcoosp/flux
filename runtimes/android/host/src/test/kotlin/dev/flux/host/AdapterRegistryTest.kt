package dev.flux.host

import dev.flux.ui.FluxAdapter
import dev.flux.ui.FluxNativeView
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * Tests for [AdapterRegistry]: the `ComponentId` → adapter mapping built from
 * the `Init` frame's string table (FLUX-017).
 *
 * The registry is the load-bearing bridge between the production adapter kit
 * (`adapters/ui-kotlin`, FLUX-009) and the runtime: a wire node carries an
 * interned `componentId`, and the registry resolves it to the dev adapter that
 * produces its native view. These tests pin that resolution without any real
 * `android.view.View` — the kit's `FluxNativeViewImpl` doubles as the view.
 */
class AdapterRegistryTest {
    @Test
    fun `resolves adapter by component id from string table`() {
        val registry =
            AdapterRegistry.fromStringTable(
                listOf(
                    StringTableEntry(100u, "column"),
                    StringTableEntry(200u, "text"),
                    StringTableEntry(300u, "button"),
                ),
            )
        assertEquals("text", registry.resolve(200u)?.kind)
        assertEquals("column", registry.resolve(100u)?.kind)
        assertEquals("button", registry.resolve(300u)?.kind)
    }

    @Test
    fun `returns null for unknown component id`() {
        val registry = AdapterRegistry.fromStringTable(emptyList())
        assertNull(registry.resolve(999u))
    }

    @Test
    fun `resolves every stdlib component the Init frame can declare`() {
        val ids = (100u..106u).toList()
        val kinds = listOf("column", "text", "button", "row", "text_field", "screen", "router")
        val registry =
            AdapterRegistry.fromStringTable(
                ids.zip(kinds) { id, kind -> StringTableEntry(id, kind) },
            )
        for (id in ids) {
            val adapter: FluxAdapter<out FluxNativeView>? = registry.resolve(id)
            assertTrue(adapter != null, "component id $id should resolve to an adapter")
        }
    }

    @Test
    fun `kind adapter identity matches component resolution`() {
        val registry = AdapterRegistry.fromStringTable(listOf(StringTableEntry(200u, "text")))
        val viaComponent = registry.resolve(200u)
        val viaKind = registry.adapterForKind("text")
        assertEquals(viaKind, viaComponent, "resolving by component id and by kind must yield the same adapter instance")
    }
}
