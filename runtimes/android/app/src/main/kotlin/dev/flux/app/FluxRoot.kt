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
import dev.flux.host.shadow.ShadowTree

/**
 * The Compose entry point for the Flux host (FLUX-007).
 *
 * Binds the reconciled [ShadowTree] to real Compose UI
 * (FA-RENDER Phase A) via [FluxTreeView] once a root node exists, manages
 * [FluxExecutor] lifecycle, and renders a red error overlay when the VM or wire
 * layer faults (Appendix E §E.6: errors show a red banner rather than crashing).
 * On `onPause` the executor stops dispatching; on `onResume` it resumes. When no
 * tree has been built yet it shows a launch-screen placeholder.
 *
 * @property session the retained host session (signal graph, shadow tree,
 *   transport, executor).
 */
@Composable
@Suppress("ktlint:standard:function-naming")
public fun FluxRoot(session: FluxSession) {
    var errorMessage by remember { mutableStateOf<String?>(null) }
    var treeReady by remember { mutableStateOf(false) }
    // Bumped on every tree mutation (initial frame + post-dispatch reconcile) so
    // Compose re-composes and re-reads the shadow node props. Holding only a
    // `treeReady` boolean would freeze the UI after the first frame, because the
    // shadow tree is mutated in place (same root node instance) and Compose would
    // never observe the prop changes a tap produces.
    var generation by remember { mutableStateOf(0) }

    val executor = session.executor
    DisposableEffect(executor) {
        session.start()
        executor.onTreeChanged = {
            val rn = session.shadowTree.rootNode
            treeReady = rn != null
            generation++
        }
        executor.onError = { errorMessage = it }
        onDispose { /* session is retained by the ViewModel; not disposed here */ }
    }

    // Reading `generation` inside the composition makes this composable re-run
    // whenever the tree changes, so `FluxTreeView` below re-reads the latest
    // node props (the new `count` value after a tap).
    generation
    val rootNode = session.shadowTree.rootNode
    Box(modifier = Modifier.fillMaxSize()) {
        when {
            // Real Compose UI: walk the shadow tree and render native widgets.
            treeReady && rootNode != null -> FluxTreeView(node = rootNode, generation = generation) { handlerId ->
                executor.dispatch(dev.flux.ui.HandlerEvent(handlerId))
            }
            else -> Text("Flux — connecting…", modifier = Modifier.align(Alignment.Center))
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
