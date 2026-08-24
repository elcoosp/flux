package dev.flux.app.transport

import java.io.Closeable

/**
 * The bidirectional wire transport the runtime receives frames over.
 *
 * Mirrors the Swift host's `FluxTransport` protocol (FLUX-006): the runtime
 * subscribes to decoded frame bytes and pushes dispatch events back to the dev
 * server. The real implementation ([OkHttpTransport]) uses OkHttp over
 * WebSocket; tests use [MockTransport] with an in-memory channel (real sockets
 * are exercised in FLUX-023).
 */
public interface FluxTransport : Closeable {
    /** Starts the connection, invoking [onFrame] for each received frame. */
    public fun connect(onFrame: (ByteArray) -> Unit)

    /** Sends a raw dispatch message (tap/event) back to the server. */
    public fun send(bytes: ByteArray)

    /** True once [connect] has completed. */
    public fun isConnected(): Boolean
}

/**
 * An in-memory transport for unit tests: frames are injected via [deliver] and
 * dispatched calls captured in [sent]. No network, no real socket — the
 * contract required for deterministic runtime tests (FLUX-007 acceptance).
 */
public class MockTransport : FluxTransport {
    private val frameSink: MutableList<(ByteArray) -> Unit> = mutableListOf()
    private var connected = false

    /** Frames that were sent back to the (pretend) server via [send]. */
    public val sent: MutableList<ByteArray> = mutableListOf()

    public fun addFrameListener(listener: (ByteArray) -> Unit) {
        frameSink.add(listener)
    }

    /** Injects a raw frame into the transport as if received from the server. */
    public fun deliver(frame: ByteArray) {
        frameSink.toList().forEach { it(frame) }
    }

    override fun connect(onFrame: (ByteArray) -> Unit) {
        frameSink.add(onFrame)
        connected = true
    }

    override fun send(bytes: ByteArray) {
        sent.add(bytes)
    }

    override fun isConnected(): Boolean = connected

    override fun close() {
        frameSink.clear()
        connected = false
    }
}
