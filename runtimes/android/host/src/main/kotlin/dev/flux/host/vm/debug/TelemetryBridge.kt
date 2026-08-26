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

    /**
     * Opens the host → DevTools `:7333` channel and installs the batch sender so
     * emitted VM/signal events flow to the dev server, which enriches them with
     * source spans and broadcasts to connected DevTools clients. Call once at
     * host startup. Safe when no dev server is running: sends before the socket
     * opens are dropped, and a closed socket simply stops delivering (spec §3
     * Key Principle 1 — zero release impact, no crash).
     */
    public fun connectDevtools(
        host: String = "127.0.0.1",
        port: Int = 7333,
    ) {
        val client =
            okhttp3.OkHttpClient
                .Builder()
                .pingInterval(15, java.util.concurrent.TimeUnit.SECONDS)
                .build()
        val request =
            okhttp3.Request
                .Builder()
                .url("ws://$host:$port/devtools")
                .build()
        val ws =
            client.newWebSocket(
                request,
                object : okhttp3.WebSocketListener() {
                    override fun onOpen(
                        webSocket: okhttp3.WebSocket,
                        response: okhttp3.Response,
                    ) = Unit

                    override fun onFailure(
                        webSocket: okhttp3.WebSocket,
                        t: Throwable,
                        response: okhttp3.Response?,
                    ) = Unit

                    override fun onClosed(
                        webSocket: okhttp3.WebSocket,
                        code: Int,
                        reason: String,
                    ) = Unit
                },
            )
        onBatch = { bytes -> ws.send(okio.ByteString.of(*bytes)) }
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
