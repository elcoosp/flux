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
 * Recomposition of the leaves is driven entirely by the shadow node's own
 * observable props ([dev.flux.host.shadow.ShadowNode.propsState], a Compose
 * `MutableState` the app injects): when the executor re-materialises a node's
 * props in place, the leaf composable that read them re-runs. No manual
 * recomposition counter is needed here — `onTreeChanged` only flips `treeReady`
 * on the first successful frame.
 *
 * @property session the retained host session (signal graph, shadow tree,
 *   transport, executor).
 */
@Composable
@Suppress("ktlint:standard:function-naming")
public fun FluxRoot(session: FluxSession) {
    var errorMessage by remember { mutableStateOf<String?>(null) }
    // Bumped on every applied frame so the composable re-reads `rootNode` and
    // displays a freshly mounted tree. A boolean `treeReady` would stay `true`
    // after the first frame and never re-trigger recomposition, so a hot reload
    // that replaces the root (node ids are unstable across edits) would mount a
    // new tree in the shadow layer yet leave the *old* composable subtree on
    // screen (blank/stale on hot reload, FLUX-019). The counter always changes,
    // forcing the re-read.
    var frameVersion by remember { mutableStateOf(0) }

    val executor = session.executor
    DisposableEffect(executor) {
        session.start()
        executor.onTreeChanged = {
            frameVersion++
        }
        executor.onError = { errorMessage = it }
        onDispose { /* session is retained by the ViewModel; not disposed here */ }
    }

    // The renderer asks the host tree for a router's visible child; this pointer
    // keeps the renderer (app module) decoupled from the host module. Without it
    // the router would stack every screen and navigation would do nothing.
    androidx.compose.runtime.SideEffect {
        activeChildrenProvider = { routerNode -> session.shadowTree.activeChildOf(routerNode) }
    }

    // `frameVersion` is read here so Compose treats `rootNode` as state-
    // dependent: every applied frame bumps it, forcing this read to re-run and
    // the freshly mounted (possibly root-replaced) tree to be displayed.
    val rootNode = session.shadowTree.rootNode.also { _ -> frameVersion }
    Box(modifier = Modifier.fillMaxSize()) {
        when {
            // Real Compose UI: walk the shadow tree and render native widgets.
            // Leaf recomposition is driven by each node's observable props.
            rootNode != null -> FluxTreeView(
                node = rootNode,
                routerVersion = frameVersion,
                onButtonClick = { handlerId -> executor.dispatch(dev.flux.ui.HandlerEvent(handlerId)) },
            )
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
