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
 * source of truth and is mutated in place (same node instances, new `props`),
 * so Compose must be told when a node's props change. That signal comes from
 * the node's [ShadowNode.propsState] observable, which the app supplies as a
 * Compose `MutableState`. Reading [observeProps] inside a leaf composable makes
 * Compose re-run that leaf whenever the executor re-materialises its props
 * (e.g. after a tap increments `count`) — mirroring SwiftUI, which observes the
 * tree mutation directly. No manual recomposition counter is threaded through
 * the render functions.
 *
 * Button clicks are forwarded through [onButtonClick] (the executor's
 * `dispatch`), which is confined to the reactive dispatcher by the caller.
 *
 * @param node the root node to render.
 * @param onButtonClick invoked with a button's bound handler id on tap.
 */
@Composable
public fun FluxTreeView(
    node: ShadowNode?,
    onButtonClick: (handlerId: UInt) -> Unit,
    /** Bumped on every applied frame / dispatch so routers re-read the active
     * route (signal 97) and swap the visible screen even though a router node's
     * own props do not change on navigation. */
    routerVersion: Int = 0,
) {
    if (node == null) return
    when (node.kind) {
        "column" -> RenderColumn(node, onButtonClick, routerVersion)
        "row" -> RenderRow(node, onButtonClick, routerVersion)
        "text" -> RenderText(node)
        "button" -> RenderButton(node, onButtonClick)
        // A router shows exactly one screen — the one whose `route` prop matches
        // the active navigation signal (ADR-0045). Every other screen is hidden,
        // so tapping "Go to Settings" actually swaps the visible view instead of
        // stacking all screens in a column.
        "router" -> RenderRouter(node, onButtonClick, routerVersion)
        // A screen renders the content it wraps (its own children).
        "screen" -> RenderContainer(node, onButtonClick, routerVersion)
        // TextField/Image have no live adapter subtree in the MLP host; surface
        // a contained placeholder so the tree stays visible.
        else -> RenderContainer(node, onButtonClick, routerVersion)
    }
}

/**
 * Renders the active `Screen` of a `Router` node. Reads [routerVersion] (a value
 * that changes on every applied frame / dispatch) so Compose re-runs this
 * composable when `Router.navigate` changes signal 97 and the host picks a
 * different active child — without it the router node's props stay equal and the
 * screen would never swap (the reported "navigation does nothing" bug).
 */
@Composable
private fun RenderRouter(
    node: ShadowNode,
    onButtonClick: (UInt) -> Unit,
    routerVersion: Int,
) {
    // `routerVersion` is read by the caller's recomposition: when it changes
    // (every applied frame / dispatch) FluxRoot re-runs and re-invokes this with
    // a new value, so the active child is re-resolved. We forward it down so any
    // nested router also swaps.
    val child = activeChildrenProvider?.invoke(node)
    if (child != null) FluxTreeView(child, onButtonClick, routerVersion)
}

/**
 * Supplies the visible child of a router node from the host shadow tree. Injected
 * by [dev.flux.app.FluxRoot] so the renderer can ask the tree (which owns the
 * signal graph and the active-route query) without depending on the host module.
 * When unset, the router falls back to showing all children.
 */
public var activeChildrenProvider: ((ShadowNode) -> ShadowNode?)? = null

/** Reads [ShadowNode.props] through its observable [State][androidx.compose.runtime.State]. */
@Composable
private fun ShadowNode.observeProps(): dev.flux.ui.Props = propsState.value

/** A vertical [Column] of the node's children, spaced by the `gap` prop. */
@Composable
private fun RenderColumn(
    node: ShadowNode,
    onButtonClick: (UInt) -> Unit,
    routerVersion: Int,
) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(gapOf(node)),
    ) {
        for (child in node.children) FluxTreeView(child, onButtonClick, routerVersion)
    }
}

/** A horizontal [Row] of the node's children, spaced by the `gap` prop. */
@Composable
private fun RenderRow(
    node: ShadowNode,
    onButtonClick: (UInt) -> Unit,
    routerVersion: Int,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(gapOf(node)),
    ) {
        for (child in node.children) FluxTreeView(child, onButtonClick, routerVersion)
    }
}

/** A [Text] leaf from the node's `text` prop. */
@Composable
private fun RenderText(node: ShadowNode) {
    val props = node.observeProps()
    val text = props.getString(PropsIndex.TEXT_TEXT).orEmpty()
    Text(
        text = text,
        modifier = Modifier.padding(4.dp),
    )
}

/** A [Button] whose label comes from `text` and whose tap fires `onPress`. */
@Composable
private fun RenderButton(
    node: ShadowNode,
    onButtonClick: (UInt) -> Unit,
) {
    val props = node.observeProps()
    val label = props.getString(PropsIndex.BUTTON_TEXT).orEmpty()
    val handlerId = props.getHandler(PropsIndex.BUTTON_ON_PRESS)
    Button(onClick = { onButtonClick(handlerId) }) {
        Text(label)
    }
}

/** Generic container: lays its children out in a vertical column. */
@Composable
private fun RenderContainer(
    node: ShadowNode,
    onButtonClick: (UInt) -> Unit,
    routerVersion: Int,
) {
    Column(modifier = Modifier.fillMaxWidth()) {
        for (child in node.children) FluxTreeView(child, onButtonClick, routerVersion)
    }
}

/** Reads the `gap` spacing prop of a linear container, defaulting to 0dp. */
private fun gapOf(node: ShadowNode): androidx.compose.ui.unit.Dp {
    val gap = node.props.getFloat(PropsIndex.STACK_GAP) ?: 0.0
    return gap.dp
}
