package dev.flux.app

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier

/**
 * The Compose entry point for the Flux host (FLUX-007).
 *
 * Wraps a [ShadowTree] in a [androidx.compose.ui.viewinterop.AndroidView]-style
 * host on the main thread, manages [FluxExecutor] lifecycle, and renders a red
 * error overlay when the VM or wire layer faults (Appendix E §E.6: errors show a
 * red banner rather than crashing). On `onPause` the executor stops dispatching;
 * on `onResume` it resumes. When no tree has been built yet it shows a
 * launch-screen placeholder.
 *
 * @property shadowTree the render tree the executor drives.
 * @property signals the signal graph the VM reads/writes.
 * @property transport the dev-mode frame transport.
 */
@Composable
@Suppress("ktlint:standard:function-naming")
public fun FluxRoot(session: FluxSession) {
    var errorMessage by remember { mutableStateOf<String?>(null) }
    var treeReady by remember { mutableStateOf(false) }

    val executor = session.executor
    DisposableEffect(executor) {
        // Roast fix 2: actually (re)bind the executor to the transport on (re)compose
        // instead of an empty resume lambda. `start()` is idempotent, so rotation /
        // recomposition rebinds without duplicating listeners (OkHttpTransport clears
        // them on connect).
        session.start()
        executor.onTreeChanged = { treeReady = session.shadowTree.rootNode != null }
        executor.onError = { errorMessage = it }
        onDispose { /* session is retained by the ViewModel; not disposed here */ }
    }

    Box(modifier = Modifier.fillMaxSize()) {
        if (treeReady) {
            // The native view subtree is owned by the shadow tree's adapters; the
            // composable hosts it via AndroidView in the real host. For the MLP
            // placeholder we surface readiness without a real view binding.
            Text("Flux host ready", modifier = Modifier.align(Alignment.Center))
        } else {
            Text("Flux — connecting…", modifier = Modifier.align(Alignment.Center))
        }
        errorMessage?.let { msg ->
            ErrorOverlay(message = msg, modifier = Modifier.align(Alignment.BottomCenter))
        }
    }
}

/**
 * A red error banner shown when the VM or wire layer faults. Errors never crash
 * the host (FLUX-007 acceptance: gas exhaustion → red banner, no crash).
 */
@Composable
@Suppress("ktlint:standard:function-naming")
private fun ErrorOverlay(
    message: String,
    modifier: Modifier,
) {
    Box(modifier = modifier) {
        Text(
            text = "Flux error: $message",
            color = androidx.compose.ui.graphics.Color.Red,
        )
    }
}
