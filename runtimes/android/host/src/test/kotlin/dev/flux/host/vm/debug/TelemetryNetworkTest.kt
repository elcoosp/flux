package dev.flux.host.vm.debug

import java.io.ByteArrayOutputStream
import org.junit.jupiter.api.Assertions.assertArrayEquals
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

/**
 * FLUX-060 follow-up: the host emits `NetworkRequest` / `NetworkResponse`
 * telemetry around the `Http` capability. These tests pin the byte layout so it
 * stays bit-identical to `flux_ir_serde` (the Rust dev server decoder) and to the
 * iOS `Telemetry.swift` encoder. The canonical array here is the SAME one the
 * Rust `host_network_telemetry_decodes_to_network_events` test decodes — changing
 * either side must change both.
 */
class TelemetryNetworkTest {
    @Test
    fun networkRequest_encodes_to_canonical_bytes() {
        val event =
            TelemetryEvent.NetworkRequest(
                requestId = 7u,
                method = "GET",
                url = "https://api.example.com/users",
                body = null,
                capabilityId = 14u,
            )
        val bytes = event.toFrameBytes()
        // header MAGIC(58 55 5c 46) version(02) kind(10) event_count(01 00)
        val expected =
            byteArrayOf(
                0x58, 0x55, 0x5c, 0x46, 0x02, 0x10, 0x01, 0x00,
                0x2e, 0x00, 0x00, 0x00, // length prefix = 0x2e (46)
                0x05, // tag
                0x07, 0x00, 0x00, 0x00, // request_id = 7
                0x03, 0x00, 0x47, 0x45, 0x54, // method = "GET"
                0x1d, 0x00, // url len = 29
                0x68, 0x74, 0x74, 0x70, 0x73, 0x3a, 0x2f, 0x2f, 0x61, 0x70, 0x69, 0x2e, 0x65, 0x78, 0x61,
                0x6d, 0x70, 0x6c, 0x65, 0x2e, 0x63, 0x6f, 0x6d, 0x2f, 0x75, 0x73, 0x65, 0x72, 0x73,
                0x00, // no body
                0x0e, 0x00, 0x00, 0x00, // capability_id = 14
            )
        assertEquals(expected.size, bytes.size)
        assertArrayEquals(expected, bytes)
    }

    @Test
    fun networkResponse_encodes_to_canonical_bytes() {
        val event =
            TelemetryEvent.NetworkResponse(
                requestId = 7u,
                statusCode = 200u,
                latencyMs = 42u,
                body = "{\"ok\":true}",
                resultKind = 1u,
            )
        val bytes = event.toFrameBytes()
        val expected =
            byteArrayOf(
                0x58, 0x55, 0x5c, 0x46, 0x02, 0x10, 0x01, 0x00,
                0x1a, 0x00, 0x00, 0x00, // length prefix = 0x1a (26)
                0x06, // tag
                0x07, 0x00, 0x00, 0x00, // request_id = 7
                0xc8.toByte(), 0x00, // status_code = 200
                0x2a, 0x00, 0x00, 0x00, // latency_ms = 42
                0x01, // body present
                0x0b, 0x00, // body len = 11
                0x7b, 0x22, 0x6f, 0x6b, 0x22, 0x3a, 0x74, 0x72, 0x75, 0x65,
                0x7d, // body = `{"ok":true}`
                0x01, // result_kind = 1
            )
        assertEquals(expected.size, bytes.size)
        assertArrayEquals(expected, bytes)
    }

    @Test
    fun networkEvents_batch_into_one_frame() {
        // The bridge batches events into a single telemetry frame; assert the
        // multi-event encode path the Rust decoder consumes.
        val out = ByteArrayOutputStream()
        out.writeUIntLE(MAGIC)
        out.write(0x01)
        out.write(FRAME_TELEMETRY.toInt())
        out.writeUShortLE(2u)
        TelemetryEvent
            .NetworkRequest(7u, "GET", "https://api.example.com/users", null, 14u)
            .encodeBody(out)
        TelemetryEvent
            .NetworkResponse(7u, 200u, 42u, "{\"ok\":true}", 1u)
            .encodeBody(out)
        val frame = out.toByteArray()
        // event_count(2) lives at offset 6.
        assertEquals(0x02, frame[6].toUByte().toInt())
    }
}
