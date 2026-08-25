package dev.flux.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.lifecycle.viewmodel.compose.viewModel

/**
 * The Flux dev-mode host activity (FLUX-007).
 *
 * In dev mode this app is precompiled once and thereafter receives binary patches
 * over WebSocket; it is never rebuilt to see a UI change. In release mode the same
 * IR is code-generated to Jetpack Compose ahead of time.
 *
 * The reactive core (signal graph, shadow tree, transport, executor) is owned by a
 * retained [FluxSession] `ViewModel`, so it survives configuration changes such as
 * rotation (roast fix 3). `onPause` deliberately does **not** tear the session down
 * (roast fix 5): the socket stays open and the executor keeps its state; only a
 * genuine finish disposes it. `onResume` re-binds the frame feed idempotently.
 */
class FluxHostActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            MaterialTheme {
                // `viewModel()` returns the retained session across rotations.
                val session: FluxSession = viewModel()
                FluxRoot(session)
            }
        }
    }

    // Roast fix 5: `onPause` intentionally does NOT close the transport. The socket
    // and executor state are retained by [FluxSession], so a brief backgrounding
    // keeps the live connection; teardown is explicit on finish only (via the
    // ViewModel's `onCleared`).

    // `onResume` re-binds the frame feed idempotently through [FluxRoot]'s lifecycle
    // effect (which calls [FluxSession.start]); no extra work is needed here.
}
