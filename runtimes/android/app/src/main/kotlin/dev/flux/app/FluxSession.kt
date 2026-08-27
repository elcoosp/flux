package dev.flux.app

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import dev.flux.host.AdapterRegistry
import dev.flux.host.FluxExecutor
import dev.flux.host.ReactiveDispatcher
import dev.flux.host.shadow.ShadowTree
import dev.flux.host.signal.SignalGraph
import dev.flux.host.transport.FluxTransport
import dev.flux.host.transport.OkHttpTransport

/**
 * Retained host session (roast fix 3 / ADR-0027 lifecycle).
 *
 * Holds the reactive core — [SignalGraph], [ShadowTree], [FluxTransport] and the
 * [FluxExecutor] that binds them — in a `ViewModel` so it survives Android
 * configuration changes (rotation) instead of being rebuilt on every `onCreate`.
 * Rebuilding these on rotation would tear down the live signal graph and drop the
 * WebSocket, forcing a full frame re-fetch; retaining them keeps the session
 * seamless across rotations.
 *
 * The session is created once via a [ViewModelProvider.Factory] that injects the
 * dev-server WebSocket URL (read from the `flux_ws_url` string resource, so the
 * endpoint is configurable per build without code changes). The executor is
 * started by the composable layer ([FluxRoot]) and only explicitly disposed when
 * the activity is truly finishing (roast fix 5: teardown is explicit, never
 * implicit on pause).
 *
 * @param wsUrl the dev-server WebSocket URL, e.g. `ws://127.0.0.1:7331` for the
 *   local loopback or `ws://192.168.x.x:7331` for a physical device on the LAN.
 */
public class FluxSession(
    private val wsUrl: String = "ws://127.0.0.1:7331",
) : ViewModel() {
    /** The live signal graph (also the VM's [dev.flux.host.vm.SignalStore]). */
    public val signals: SignalGraph = SignalGraph()

    /** The render tree the executor drives. */
    public val shadowTree: ShadowTree =
        ShadowTree(AdapterRegistry.fromStringTable(emptyList())).also {
            // The app's nodes hold their materialized props in a Compose
            // `MutableState` (the same object the renderer reads), so when the
            // executor re-materialises props in place the UI re-composes
            // automatically — mirroring SwiftUI, which observes the tree mutation
            // directly. No manual recomposition counter is threaded through the
            // render functions.
            it.propsStateFactory = { initial -> androidx.compose.runtime.mutableStateOf(initial) }
        }

    /** The dev-mode frame transport. */
    public val transport: FluxTransport = OkHttpTransport(wsUrl)

    /** The executor that ties the above together. */
    public val executor: FluxExecutor = FluxExecutor(shadowTree, signals, transport)

    /** Starts (or re-binds) the transport → executor frame feed. Idempotent. */
    public fun start() {
        if (!transport.isConnected()) executor.start()
    }

    /** Explicit teardown (roast fix 5): called only when the activity is finishing. */
    public fun dispose() {
        executor.dispose()
    }

    override fun onCleared() {
        super.onCleared()
        executor.dispose()
    }

    /** Factory injecting the configured [wsUrl] into [FluxSession]. */
    public class Factory(
        private val wsUrl: String,
    ) : ViewModelProvider.Factory {
        @Suppress("UNCHECKED_CAST")
        override fun <T : ViewModel> create(modelClass: Class<T>): T =
            FluxSession(wsUrl) as T
    }
}
