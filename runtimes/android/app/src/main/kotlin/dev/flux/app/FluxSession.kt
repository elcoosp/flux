package dev.flux.app

import androidx.lifecycle.ViewModel
import dev.flux.host.AdapterRegistry
import dev.flux.host.FluxExecutor
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
 * The session is created once (via `viewModels()`); the executor is started by the
 * composable layer ([FluxRoot]) and only explicitly disposed when the activity is
 * truly finishing (roast fix 5: teardown is explicit, never implicit on pause).
 */
public class FluxSession : ViewModel() {
    /** The live signal graph (also the VM's [dev.flux.host.vm.SignalStore]). */
    public val signals: SignalGraph = SignalGraph()

    /** The render tree the executor drives. */
    public val shadowTree: ShadowTree =
        ShadowTree(AdapterRegistry.fromStringTable(emptyList()))

    /** The dev-mode frame transport. */
    public val transport: FluxTransport = OkHttpTransport("ws://127.0.0.1:7331")

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
}
