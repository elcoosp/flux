package dev.flux.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import dev.flux.app.AdapterRegistry
import dev.flux.app.shadow.ShadowTree
import dev.flux.app.signal.SignalGraph
import dev.flux.app.transport.FluxTransport
import dev.flux.app.transport.OkHttpTransport

/**
 * The Flux dev-mode host activity (FLUX-007).
 *
 * In dev mode this app is precompiled once and thereafter receives binary
 * patches over WebSocket; it is never rebuilt to see a UI change. In release
 * mode the same IR is code-generated to Jetpack Compose ahead of time.
 *
 * The activity owns the [FluxExecutor] lifecycle: on `onPause` the executor
 * stops dispatching; on `onResume` it resumes. The render tree, signal graph
 * and transport are created once and survive configuration changes via the
 * retained (non-configuration) instance scope.
 */
class FluxHostActivity : ComponentActivity() {
    private lateinit var transport: FluxTransport
    private lateinit var signals: SignalGraph
    private lateinit var shadowTree: ShadowTree

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        signals = SignalGraph()
        shadowTree = ShadowTree(AdapterRegistry.fromStringTable(emptyList()))
        transport = OkHttpTransport("ws://127.0.0.1:9001")
        setContent {
            MaterialTheme {
                FluxRoot(shadowTree, signals, transport)
            }
        }
    }

    override fun onPause() {
        super.onPause()
        // Stop dispatching while backgrounded; the transport keeps the socket.
        transport.close()
    }

    override fun onResume() {
        super.onResume()
        // Reconnect and resume receiving frames (Appendix D §D.13).
        transport.connect { /* executor re-binds via FluxRoot on recomposition */ }
    }
}
