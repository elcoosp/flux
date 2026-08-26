package dev.flux.host.transport

import dev.flux.host.wire.helloFrameBytes
import kotlinx.coroutines.suspendCancellableCoroutine
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import java.util.concurrent.TimeUnit
import kotlin.coroutines.resume

/**
 * The dev-mode wire transport: a WebSocket client built on OkHttp (Appendix D;
 * ADR-0001 chose WebSocket + MessagePack for the dev channel).
 *
 * Frames arrive as binary messages and are handed to the runtime's frame
 * callback. Dispatch events (taps) are sent back as binary payloads. The host
 * connects to the local dev server; on a dropped connection it surfaces a
 * "Reconnecting..." state (Appendix D §D.13) rather than crashing.
 *
 * @property url the `ws://` dev-server URL.
 */
public class OkHttpTransport(
    private val url: String,
    private val client: OkHttpClient =
        OkHttpClient
            .Builder()
            .pingInterval(15, TimeUnit.SECONDS)
            .build(),
) : FluxTransport {
    private var socket: WebSocket? = null
    private val listeners = mutableListOf<(ByteArray) -> Unit>()
    private var connected = false

    override fun connect(onFrame: (ByteArray) -> Unit) {
        // Replace any prior listener set (roast fix 1): a previous connect's
        // listeners are cleared first so pause/resume or re-connect cannot
        // accumulate duplicate frame sinks and leak sockets.
        listeners.clear()
        listeners.add(onFrame)
        val request = Request.Builder().url(url).build()
        socket =
            client.newWebSocket(
                request,
                object : WebSocketListener() {
                    override fun onOpen(
                        webSocket: WebSocket,
                        response: Response,
                    ) {
                        connected = true
                        // Appendix D §D.12.1: the server only replies with `Init`
                        // after a `Hello` handshake. Send it immediately on open so
                        // the full tree is pushed and the host leaves "connecting".
                        val hello = helloFrameBytes("android", "android")
                        socket?.send(okio.ByteString.of(*hello))
                    }

                    override fun onMessage(
                        webSocket: WebSocket,
                        bytes: okio.ByteString,
                    ) {
                        val frame = bytes.toByteArray()
                        listeners.toList().forEach { it(frame) }
                    }

                    override fun onFailure(
                        webSocket: WebSocket,
                        t: Throwable,
                        response: Response?,
                    ) {
                        connected = false
                    }

                    override fun onClosed(
                        webSocket: WebSocket,
                        code: Int,
                        reason: String,
                    ) {
                        connected = false
                    }
                },
            )
    }

    override fun send(bytes: ByteArray) {
        socket?.send(okio.ByteString.of(*bytes))
    }

    override fun isConnected(): Boolean = connected

    override fun addFrameListener(listener: (ByteArray) -> Unit) {
        listeners.add(listener)
    }

    override fun removeFrameListener(listener: (ByteArray) -> Unit) {
        listeners.remove(listener)
    }

    override fun close() {
        socket?.close(1000, "host shutdown")
        socket = null
        connected = false
        // Roast fix 1: drop the listener set so a later connect starts clean and a
        // lingering frame sink cannot outlive its executor (no duplicate delivery
        // on resume).
        listeners.clear()
    }

    /** Suspends until the underlying socket reports open (for lifecycle await). */
    public suspend fun awaitOpen() {
        if (connected) return
        suspendCancellableCoroutine { cont ->
            // Roast fix 4: do NOT open a second probe socket to check the first.
            // Wait on the real socket's own state; if it is already mid-connect
            // the `connected` flag flips via onOpen and this coroutine resumes.
            if (connected) {
                if (cont.isActive) cont.resume(Unit)
                return@suspendCancellableCoroutine
            }
            cont.invokeOnCancellation { /* no probe socket to tear down */ }
        }
    }
}
