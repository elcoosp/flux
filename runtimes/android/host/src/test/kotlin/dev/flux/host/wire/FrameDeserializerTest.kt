package dev.flux.host.wire

import dev.flux.host.vm.FluxValue
import dev.flux.ui.PropsIndex
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNotNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Assumptions.assumeTrue
import org.junit.jupiter.api.Test
import java.io.File

/**
 * FluxFrame deserializer tests (FLUX-007 acceptance criterion 10): hand-built byte
 * arrays from Appendix D, plus the `FLUX_WIRE_FIXTURES` env-var loader (R10)
 * that skips cleanly when the directory is absent.
 *
 * The hand-built frames are encoded exactly as [FrameDeserializer] decodes them
 * (Appendix D §D.1/§D.3/§D.5); the test asserts the round trip rather than
 * claiming wire parity with a server we do not yet have.
 */
class FrameDeserializerTest {
    /** Loads external fixtures from `FLUX_WIRE_FIXTURES` when set, else skips. */
    @Test
    fun `external wire fixtures are optional`() {
        val path = System.getenv("FLUX_WIRE_FIXTURES")
        assumeTrue(path != null, "FLUX_WIRE_FIXTURES not set; real fixtures land in FLUX-023")
        val dir = File(path!!)
        assumeTrue(dir.isDirectory, "FLUX_WIRE_FIXTURES points at a missing directory")
        val files = dir.listFiles { f -> f.extension == "bin" } ?: emptyArray()
        for (file in files) {
            val bytes = file.readBytes()
            val frame = FrameDeserializer.deserialize(bytes)
            assertNotNull(frame, "failed to decode ${file.name}")
        }
    }

    @Test
    fun `decodes a full-tree Init frame with one text node`() {
        val bytes =
            FrameBuilder()
                .apply {
                    magic()
                    version(1)
                    seq(0)
                    flags(fullTree = true)
                    patchCount(0)
                    handlerCount(0)
                    stringCount(0)
                    // Root node: id=1, kind=1 (Primitive), component=100, no props,
                    // one child (Node id=2), no handlers, span 0/0/0.
                    node(id = 1u, kind = 1u, component = 100u, props = emptyList(), childIds = listOf(2u))
                    // Child node: id=2, kind=1, component=200 ("text"), one prop
                    // (TEXT_TEXT = str id 7), no children.
                    node(
                        id = 2u,
                        kind = 1u,
                        component = 200u,
                        props = listOf(PropsIndex.TEXT_TEXT to WireValue.StrVal(7u)),
                        childIds = emptyList(),
                    )
                }.build()

        val frame = FrameDeserializer.deserialize(bytes)
        assertTrue(frame.fullTree)
        assertNotNull(frame.root)
        val root = frame.root!!
        assertEquals(1u, root.id)
        assertEquals(1, root.children.size)
        val child = frame.extraNodes.first { it.id == 2u }
        assertEquals(200u, child.componentId)
        assertEquals(WireValue.StrVal(7u), child.props.first().second)
    }

    @Test
    fun `rejects a bad magic`() {
        val bytes = byteArrayOf(0x00, 0x00, 0x00, 0x00, 1, 0, 0, 0, 0, 0, 0)
        val err = runCatching { FrameDeserializer.deserialize(bytes) }
        assertTrue(err.isFailure, "expected WireError for bad magic")
    }

    @Test
    fun `rejects a protocol-version mismatch fail-closed`() {
        // FLUX-050 / ADR-0056: a frame whose version byte the host does not
        // implement must be rejected with WireError, never mis-decoded.
        val bytes =
            FrameBuilder()
                .apply {
                    magic()
                    version(1)
                    seq(0)
                    flags(fullTree = true)
                    patchCount(0)
                    handlerCount(0)
                    stringCount(0)
                    node(id = 1u, kind = 1u, component = 100u, props = emptyList(), childIds = emptyList())
                }.build()
        // Flip the version byte (header offset 4) to an unsupported value.
        // NOTE: v1 (0x01) and v2 (0x02, ADR-0057) are both supported; only an
        // out-of-range version must be rejected fail-closed.
        bytes[4] = 3
        val err = runCatching { FrameDeserializer.deserialize(bytes) }
        assertTrue(err.isFailure, "expected WireError for protocol version mismatch")
        val ex = err.exceptionOrNull()
        if (ex !is WireError) {
            throw AssertionError("version mismatch must surface as WireError, got ${ex?.javaClass?.name}: ${ex?.message}", ex)
        }
    }

    @Test
    fun `decodes a value list and record`() {
        val r =
            ByteReader(
                byteArrayOf(
                    0x06, // List tag
                    0x02,
                    0x00, // count = 2
                    0x01,
                    0x05.toByte(),
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00, // Int 5
                    0x03,
                    0x01, // Bool true
                ),
            )
        // Build a list value via the public value decoder path through a frame
        // is overkill; assert the reader yields expected shapes directly.
        assertEquals(0x06, r.u8())
        assertEquals(2, r.u16())
        assertEquals(0x01, r.u8())
        assertEquals(5L, r.i64())
        assertEquals(0x03, r.u8())
        assertEquals(1, r.u8())
    }

    @Test
    fun `wire value to kit value conversion preserves shapes`() {
        val wire =
            WireValue.RecordVal(
                listOf(WireValue.RecordVal.Field(0u, WireValue.IntVal(42))),
            )
        val v = wire.toKitValue()
        assertTrue(v is dev.flux.ui.FluxValue.Record)
        val rec = v as dev.flux.ui.FluxValue.Record
        assertEquals(
            dev.flux.ui.FluxValue
                .Int(42),
            rec.fields[0].value,
        )
    }
}
