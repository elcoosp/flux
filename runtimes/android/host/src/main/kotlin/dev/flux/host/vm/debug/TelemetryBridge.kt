package dev.flux.host.vm.debug

import java.io.ByteArrayOutputStream
import java.util.concurrent.ConcurrentLinkedQueue

/**
 * Receiver for host VM telemetry (spec §3.1).
 *
 * The DevTools transport sets [sink]; the VM and signal graph call
 * [emit] from their evaluation context. Emission is a cheap queue append so
 * instrumentation never blocks the VM or UI thread (spec §3.3).
 */
public fun interface VMTelemetrySink {
    public fun emit(event: TelemetryEvent)
}

/**
 * Thread-safe batching bridge for DevTools telemetry (spec §3.3).
 *
 * The host VM appends events; the bridge batches them (on a 10-event threshold)
 * and hands the encoded `Telemetry` frame to [onBatch], which the host
 * transport wires to the WebSocket. Guarded by `BuildConfig.DEBUG` at every
 * call site (spec §3 Key Principle 1).
 */
public object TelemetryBridge {
    private val pending: ConcurrentLinkedQueue<TelemetryEvent> = ConcurrentLinkedQueue()

    /** The active DevTools sink, or `null` when DevTools is disconnected. */
    public var sink: VMTelemetrySink? = null

    /** Hook the host transport sets to transmit a batched frame. */
    public var onBatch: ((ByteArray) -> Unit)? = null

    /** Appends an event, flushing when the batch threshold is reached. */
    public fun emit(event: TelemetryEvent) {
        pending.add(event)
        if (pending.size >= 10) flush()
    }

    /** Encodes and forwards all pending events as one `Telemetry` frame. */
    public fun flush() {
        if (pending.isEmpty()) return
        val batch = mutableListOf<TelemetryEvent>()
        while (pending.isNotEmpty()) batch.add(pending.poll()!!)
        val out =
            ByteArrayOutputStream().apply {
                writeUIntLE(MAGIC)
                write(0x01)
                write(FRAME_TELEMETRY.toInt())
                writeUShortLE(batch.size.toUShort())
                for (event in batch) event.encodeBody(this)
            }
        onBatch?.invoke(out.toByteArray())
    }

    /** Drains pending events without flushing (test/transport helper). */
    public fun takePending(): List<TelemetryEvent> {
        val drained = mutableListOf<TelemetryEvent>()
        while (pending.isNotEmpty()) drained.add(pending.poll()!!)
        return drained
    }

    private fun ByteArrayOutputStream.writeUIntLE(v: UInt) {
        write((v.toInt() and 0xFF))
        write((v.toInt() ushr 8) and 0xFF)
        write((v.toInt() ushr 16) and 0xFF)
        write((v.toInt() ushr 24) and 0xFF)
    }

    private fun ByteArrayOutputStream.writeUShortLE(v: UShort) {
        write(v.toInt() and 0xFF)
        write((v.toInt() ushr 8) and 0xFF)
    }
}
