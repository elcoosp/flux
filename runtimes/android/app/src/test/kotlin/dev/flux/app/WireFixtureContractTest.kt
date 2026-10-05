package dev.flux.app

import dev.flux.host.wire.FluxFrame
import dev.flux.host.wire.FrameDeserializer
import dev.flux.host.wire.WireError
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNotNull
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
        assertEquals(0x02, bytes[4].toInt(), "version must be 2 (0x02)")
        val decoded = FrameDeserializer.deserialize(bytes)
        val root = decoded.root
        assertNotNull(root, "init_v2 must carry a root node")
        // The fixture's root references its two child ids (Button/Text); the
        // child NodeRefs are encoded inline in `children`, not as flat extras.
        assertEquals(2, root!!.children.size, "init_v2 root should reference 2 child ids")
    }

    @Test
    fun `delta_v2 decodes one patch`() {
        val bytes = loadFixture("delta_v2.bin")
        assertEquals(0x02, bytes[4].toInt(), "version must be 2 (0x02)")
        val decoded = FrameDeserializer.deserialize(bytes)
        assertTrue(decoded.patches.isNotEmpty(), "delta_v2 must carry patches")
    }

    @Test
    fun `init_v3 decodes root and two children`() {
        val bytes = loadFixture("init_v3.bin")
        assertEquals(0x03, bytes[4].toInt(), "version must be 3 (0x03)")
        val decoded = FrameDeserializer.deserialize(bytes)
        val root = decoded.root
        assertNotNull(root, "init_v3 must carry a root node")
        assertEquals(2, root!!.children.size, "init_v3 root should reference 2 child ids")
    }

    @Test
    fun `delta_v3 decodes one patch`() {
        val bytes = loadFixture("delta_v3.bin")
        assertEquals(0x03, bytes[4].toInt(), "version must be 3 (0x03)")
        val decoded = FrameDeserializer.deserialize(bytes)
        assertTrue(decoded.patches.isNotEmpty(), "delta_v3 must carry patches")
    }

    @Test
    fun `unsupported version fixture is rejected`() {
        val bytes = loadFixture("unsupported-version.bin")
        assertEquals(0x04, bytes[4].toInt(), "fixture must carry unsupported version 4")
        try {
            FrameDeserializer.deserialize(bytes)
            error("unsupported version must be rejected")
        } catch (e: WireError) {
            // Expected — fail-closed.
            assertTrue(e.message?.contains("version") == true)
        }
    }
}
