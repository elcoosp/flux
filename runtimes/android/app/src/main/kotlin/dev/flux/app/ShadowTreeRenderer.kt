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
import dev.flux.app.shadow.ShadowNode
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
 * @param node the root node to render.
 * @param onButtonClick invoked with a button's bound handler id on tap.
 */
@Composable
public fun FluxTreeView(
    node: ShadowNode?,
    onButtonClick: (handlerId: UInt) -> Unit,
) {
    if (node == null) return
    when (node.kind) {
        "column" -> RenderColumn(node, onButtonClick)
        "row" -> RenderRow(node, onButtonClick)
        "text" -> RenderText(node)
        "button" -> RenderButton(node, onButtonClick)
        // Containers without bespoke layout: render their children inline.
        "screen", "router" -> RenderContainer(node, onButtonClick)
        // TextField/Image have no live adapter subtree in the MLP host; surface
        // a contained placeholder so the tree stays visible.
        else -> RenderContainer(node, onButtonClick)
    }
}

/** A vertical [Column] of the node's children, spaced by the `gap` prop. */
@Composable
private fun RenderColumn(
    node: ShadowNode,
    onButtonClick: (UInt) -> Unit,
) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(gapOf(node)),
    ) {
        for (child in node.children) FluxTreeView(child, onButtonClick)
    }
}

/** A horizontal [Row] of the node's children, spaced by the `gap` prop. */
@Composable
private fun RenderRow(
    node: ShadowNode,
    onButtonClick: (UInt) -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(gapOf(node)),
    ) {
        for (child in node.children) FluxTreeView(child, onButtonClick)
    }
}

/** A [Text] leaf from the node's `text` prop. */
@Composable
private fun RenderText(node: ShadowNode) {
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
    onButtonClick: (UInt) -> Unit,
) {
    Column(modifier = Modifier.fillMaxWidth()) {
        for (child in node.children) FluxTreeView(child, onButtonClick)
    }
}

/** Reads the `gap` spacing prop of a linear container, defaulting to 0dp. */
private fun gapOf(node: ShadowNode): androidx.compose.ui.unit.Dp {
    val gap = node.props.getFloat(PropsIndex.STACK_GAP) ?: 0.0
    return gap.dp
}

/** Reads the text string prop off a node, for test assertions. */
internal fun ShadowNode.displayText(): String? = props.getString(PropsIndex.TEXT_TEXT)

/** Reads the bound handler id off a button node, for test assertions. */
internal fun ShadowNode.buttonHandlerId(): UInt = props.getHandler(PropsIndex.BUTTON_ON_CLICK)

/** `true` when [value] is a resolved (non-id) string, used by tests. */
internal fun FluxValue.isResolvedString(): Boolean = this is FluxValue.Str
