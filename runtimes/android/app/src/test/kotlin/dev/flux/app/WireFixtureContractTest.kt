package dev.flux.app

import dev.flux.host.wire.FluxFrame
import dev.flux.host.wire.FrameDeserializer
import dev.flux.host.wire.WireError
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class WireFixtureContractTest {
    /**
     * FLUX-083: decode the committed `fixtures/wire/` binaries using the
     * Kotlin host decoder and assert node counts + the rejected-version path.
     */
    private fun loadFixture(name: String): ByteArray {
        val resource = javaClass.classLoader.getResource("wire/$name")
            ?: error("fixture $name not found on classpath")
        return resource.readBytes()
    }

    @Test
    fun `init_v2 decodes root and two children`() {
        val bytes = loadFixture("init_v2.bin")
        assertEquals(0x02u, bytes[4].toUByte(), "version must be 2 (0x02)")
        val decoded = FrameDeserializer.deserialize(bytes)
        val extra = decoded.extraNodes
        assertEquals(2, extra.size, "init_v2 should have 2 extra nodes (Text + Button)")
    }

    @Test
    fun `delta_v2 decodes one patch`() {
        val bytes = loadFixture("delta_v2.bin")
        assertEquals(0x02u, bytes[4].toUByte(), "version must be 2 (0x02)")
        val decoded = FrameDeserializer.deserialize(bytes)
        assertTrue(decoded.patches.isNotEmpty(), "delta_v2 must carry patches")
    }

    @Test
    fun `unsupported version fixture is rejected`() {
        val bytes = loadFixture("unsupported-version.bin")
        assertEquals(0x03u, bytes[4].toUByte(), "fixture must carry unsupported version 3")
        try {
            FrameDeserializer.deserialize(bytes)
            error("unsupported version must be rejected")
        } catch (e: WireError) {
            // Expected — fail-closed.
            assertTrue(e.message?.contains("version") == true)
        }
    }
}
