package dev.flux.host.wire

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

/**
 * Perf task 8 (P2 follow-on): the deserializer must not change behavior when
 * allocation is reduced. The handler-bytecode blob is now a zero-copy window
 * over the frame buffer; this test locks in that a frame decodes identically
 * before/after and that the windowed blob still yields the correct handler
 * bytecode when sliced.
 */
class FrameDeserializerPerfTest {
    @Test
    fun `frame decodes identically on repeated deserialization`() {
        val bytes = frameWithHandler()
        val a = FrameDeserializer.deserialize(bytes)
        val b = FrameDeserializer.deserialize(bytes)
        assertEquals(a.handlers.size, b.handlers.size)
        assertEquals(a.bytecodeBlob?.len, b.bytecodeBlob?.len)
        assertEquals(a.extraNodes, b.extraNodes)
        assertEquals(a.strings, b.strings)
    }

    @Test
    fun `windowed blob slices handler bytecode without copying on decode`() {
        val bytes = frameWithHandler()
        val frame = FrameDeserializer.deserialize(bytes)
        val blob = frame.bytecodeBlob!!
        // The window references the original buffer (zero-copy at decode time).
        assertEquals(bytes.size, blob.data.size)
        // Slicing the handler out of the window yields the same bytecode the
        // executor would register.
        val def = frame.handlers.first()
        val start = blob.offset + def.closure.bytecodeOffset.toInt()
        val len = def.closure.bytecodeLen.toInt()
        val sliced = blob.data.copyOfRange(start, start + len)
        assertEquals(expectedHandlerPrefix.toList(), sliced.take(expectedHandlerPrefix.size).toList())
    }

    private fun frameWithHandler(): ByteArray {
        val closure =
            byteArrayOf(
                0xB0.toByte(),
                0,
                1,
                0,
                0,
                0,
                0,
                0,
                0,
                0, // LOAD_INT_CONST r0, 1
                0x11.toByte(),
                1,
                0,
                0,
                0,
                0, // WRITE_SIGNAL 1, r0
                0x00,
            )
        val ref =
            ClosureRef(
                hash = ByteArray(8),
                bytecodeOffset = 0u,
                bytecodeLen = closure.size.toUShort(),
                signals = emptyList(),
                span = null,
                excerpt = null,
            )
        return FrameBuilder()
            .apply {
                magic()
                version(1)
                seq(0)
                flags(fullTree = true)
                patchCount(0)
                handlerCount(1)
                stringCount(0)
                handlerSection(closure, listOf(5u to ref))
                node(id = 1u, kind = 0x12u, component = 100u, props = emptyList(), childIds = listOf(2u))
                node(id = 2u, kind = 0x10u, component = 200u, props = emptyList(), childIds = emptyList())
            }.build()
    }

    private val expectedHandlerPrefix = byteArrayOf(0xB0.toByte(), 0, 1, 0, 0, 0, 0, 0, 0, 0)
}
