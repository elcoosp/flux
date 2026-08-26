package dev.flux.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.flux.host.shadow.ShadowNode
import dev.flux.ui.FluxValue
import dev.flux.ui.PropsIndex

/**
 * Renders the reconciled [ShadowNode] tree as real Compose UI (FA-RENDER Phase
 * A). The dev adapters emit an in-memory `FluxNativeView` (FLUX-009); this
 * host walks the *shadow tree* — whose `props` already carry resolved
 * [FluxValue]s — and materializes the equivalent Compose subtree so the counter
 * example shows genuine native views instead of the MLP placeholder.
 *
 * The renderer is a pure projection of the tree: the shadow tree stays the
 * source of truth, and the reconciler mutates it in place (view identity
 * preserved per node id), so this composable re-composes without rebuilding
 * state. Button clicks are forwarded through [onButtonClick] (the executor's
 * `dispatch`), which is confined to the reactive dispatcher by the caller.
 *
 * [generation] is a monotonically increasing counter bumped on every tree
 * mutation (initial frame + post-dispatch reconcile). Because the shadow tree
 * is mutated in place, the [node] reference is stable across taps; without
 * [generation] Compose would skip re-executing this subtree (the parameter
 * looks unchanged) and the UI would freeze after the first frame. Passing
 * [generation] forces a re-run whenever the tree changes.
 *
 * @param node the root node to render.
 * @param generation bumped on every tree mutation; forces re-execution.
 * @param onButtonClick invoked with a button's bound handler id on tap.
 */
@Composable
public fun FluxTreeView(
    node: ShadowNode?,
    generation: Int,
    onButtonClick: (handlerId: UInt) -> Unit,
) {
    // Read `generation` so Compose tracks it and re-runs the subtree when the
    // tree mutates in place (same `node` instance, new `props`).
    @Suppress("UNUSED_VARIABLE")
    val _gen = generation
    if (node == null) return
    when (node.kind) {
        "column" -> RenderColumn(node, generation, onButtonClick)
        "row" -> RenderRow(node, generation, onButtonClick)
        "text" -> RenderText(node, generation)
        "button" -> RenderButton(node, generation, onButtonClick)
        // Containers without bespoke layout: render their children inline.
        "screen", "router" -> RenderContainer(node, generation, onButtonClick)
        // TextField/Image have no live adapter subtree in the MLP host; surface
        // a contained placeholder so the tree stays visible.
        else -> RenderContainer(node, generation, onButtonClick)
    }
}

/** A vertical [Column] of the node's children, spaced by the `gap` prop. */
@Composable
private fun RenderColumn(
    node: ShadowNode,
    generation: Int,
    onButtonClick: (UInt) -> Unit,
) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(gapOf(node)),
    ) {
        for (child in node.children) FluxTreeView(child, generation, onButtonClick)
    }
}

/** A horizontal [Row] of the node's children, spaced by the `gap` prop. */
@Composable
private fun RenderRow(
    node: ShadowNode,
    generation: Int,
    onButtonClick: (UInt) -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(gapOf(node)),
    ) {
        for (child in node.children) FluxTreeView(child, generation, onButtonClick)
    }
}

/** A [Text] leaf from the node's `text` prop. */
@Composable
private fun RenderText(
    node: ShadowNode,
    generation: Int,
) {
    val text = node.props.getString(PropsIndex.TEXT_TEXT).orEmpty()
    Text(
        text = text,
        modifier = Modifier.padding(4.dp),
    )
}

/** A [Button] whose label comes from `text` and whose tap fires `onClick`. */
@Composable
private fun RenderButton(
    node: ShadowNode,
    generation: Int,
    onButtonClick: (UInt) -> Unit,
) {
    val label = node.props.getString(PropsIndex.BUTTON_TEXT).orEmpty()
    val handlerId = node.props.getHandler(PropsIndex.BUTTON_ON_CLICK)
    Button(onClick = { onButtonClick(handlerId) }) {
        Text(label)
    }
}

/** Generic container: lays its children out in a vertical column. */
@Composable
private fun RenderContainer(
    node: ShadowNode,
    generation: Int,
    onButtonClick: (UInt) -> Unit,
) {
    Column(modifier = Modifier.fillMaxWidth()) {
        for (child in node.children) FluxTreeView(child, generation, onButtonClick)
    }
}

/** Reads the `gap` spacing prop of a linear container, defaulting to 0dp. */
private fun gapOf(node: ShadowNode): androidx.compose.ui.unit.Dp {
    val gap = node.props.getFloat(PropsIndex.STACK_GAP) ?: 0.0
    return gap.dp
}
