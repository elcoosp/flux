package dev.flux.app

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import dev.flux.host.media.ImageCache
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
    onButtonClick: (handlerId: UInt, nodeId: UInt) -> Unit,
    /** Bumped on every applied frame / dispatch so routers re-read the active
     * route (signal 97) and swap the visible screen even though a router node's
     * own props do not change on navigation. */
    routerVersion: Int = 0,
    /** Fired with a text-input's `onChangeText` handler id and the new value. */
    onTextChange: (UInt, UInt, String) -> Unit = { _, _, _ -> },
    /** Layout weight applied to this node by a parent [Row] (so a [TextInput]
     * shares the row with its sibling button instead of filling the whole
     * width). Defaults to none. */
    childModifier: Modifier = Modifier,
    /** Shared host-side image cache (FLUX-039): LRU memory + single-flight
     * fetch, keyed by resolved asset/remote URL. */
    imageCache: ImageCache,
    /** Dev-server asset base used to resolve an `Image` `source` prop. */
    assetBaseUrl: String = "http://localhost:7332/assets/",
) {
    if (node == null) return
    when (node.kind) {
        "column" -> RenderColumn(node, onButtonClick, routerVersion, onTextChange, imageCache, assetBaseUrl)
        "row" -> RenderRow(node, onButtonClick, routerVersion, onTextChange, imageCache, assetBaseUrl)
        "stack" -> RenderStack(node, onButtonClick, routerVersion, onTextChange, imageCache, assetBaseUrl)
        "grid" -> RenderGrid(node, onButtonClick, routerVersion, onTextChange, imageCache, assetBaseUrl)
        "spacer" -> RenderSpacer(node)
        "safearea" -> RenderSafeArea(node, onButtonClick, routerVersion, onTextChange, imageCache, assetBaseUrl)
        "scrollview" -> RenderScrollView(node, onButtonClick, routerVersion, onTextChange, imageCache, assetBaseUrl)
        "modal" -> RenderOverlayContainer(node, onButtonClick, routerVersion, onTextChange, imageCache, assetBaseUrl)
        "sheet" -> RenderOverlayContainer(node, onButtonClick, routerVersion, onTextChange, imageCache, assetBaseUrl)
        "dialog" -> RenderOverlayContainer(node, onButtonClick, routerVersion, onTextChange, imageCache, assetBaseUrl)
        "animate" -> RenderOverlayContainer(node, onButtonClick, routerVersion, onTextChange, imageCache, assetBaseUrl)
        "text" -> RenderText(node)
        "button" -> RenderButton(node, onButtonClick)
        "textinput" -> RenderTextInput(node, onTextChange, childModifier)
        "image" -> RenderImage(node, imageCache, assetBaseUrl)
        // A router shows exactly one screen — the one whose `route` prop matches
        // the active navigation signal (ADR-0045). Every other screen is hidden,
        // so tapping "Go to Settings" actually swaps the visible view instead of
        // stacking all screens in a column.
        "router" -> RenderRouter(node, onButtonClick, routerVersion, onTextChange, imageCache, assetBaseUrl)
        // A screen renders the content it wraps (its own children).
        "screen" -> RenderContainer(node, onButtonClick, routerVersion, onTextChange, imageCache, assetBaseUrl)
        // TextField/Image have no live adapter subtree in the MLP host; surface
        // a contained placeholder so the tree stays visible.
        else -> RenderContainer(node, onButtonClick, routerVersion, onTextChange, imageCache, assetBaseUrl)
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
    onButtonClick: (UInt, UInt) -> Unit,
    routerVersion: Int,
    onTextChange: (UInt, UInt, String) -> Unit = { _, _, _ -> },
    imageCache: ImageCache,
    assetBaseUrl: String = "http://localhost:7332/assets/",
) {
    // `routerVersion` is read by the caller's recomposition: when it changes
    // (every applied frame / dispatch) FluxRoot re-runs and re-invokes this with
    // a new value, so the active child is re-resolved. We forward it down so any
    // nested router also swaps.
    val child = activeChildrenProvider?.invoke(node)
    if (child != null) {
        FluxTreeView(
            child,
            onButtonClick,
            routerVersion,
            onTextChange,
            imageCache = imageCache,
            assetBaseUrl = assetBaseUrl,
        )
    }
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
    onButtonClick: (UInt, UInt) -> Unit,
    routerVersion: Int,
    onTextChange: (UInt, UInt, String) -> Unit = { _, _, _ -> },
    imageCache: ImageCache,
    assetBaseUrl: String = "http://localhost:7332/assets/",
) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(gapOf(node)),
    ) {
        for (child in node.children) {
            FluxTreeView(
                child,
                onButtonClick,
                routerVersion,
                onTextChange,
                imageCache = imageCache,
                assetBaseUrl = assetBaseUrl,
            )
        }
    }
}

/** A horizontal [Row] of the node's children, spaced by the `gap` prop. */
@Composable
private fun RenderRow(
    node: ShadowNode,
    onButtonClick: (UInt, UInt) -> Unit,
    routerVersion: Int,
    onTextChange: (UInt, UInt, String) -> Unit = { _, _, _ -> },
    imageCache: ImageCache,
    assetBaseUrl: String = "http://localhost:7332/assets/",
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(gapOf(node)),
    ) {
        for (child in node.children) {
            // A TextInput inside a row takes the remaining width (weight 1f) so its
            // sibling button stays visible instead of being pushed off-screen by
            // fillMaxWidth. Every other child keeps its intrinsic size.
            val childModifier = if (child.kind == "textinput") Modifier.weight(1f) else Modifier
            FluxTreeView(
                child,
                onButtonClick,
                routerVersion,
                onTextChange,
                childModifier,
                imageCache = imageCache,
                assetBaseUrl = assetBaseUrl,
            )
        }
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
    onButtonClick: (UInt, UInt) -> Unit,
) {
    val props = node.observeProps()
    val label = props.getString(PropsIndex.BUTTON_TEXT).orEmpty()
    val handlerId = props.getHandler(PropsIndex.BUTTON_ON_PRESS)
    Button(onClick = { onButtonClick(handlerId, node.id) }) {
        Text(label)
    }
}

/** Generic container: lays its children out in a vertical column. */
@Composable
private fun RenderContainer(
    node: ShadowNode,
    onButtonClick: (UInt, UInt) -> Unit,
    routerVersion: Int,
    onTextChange: (UInt, UInt, String) -> Unit = { _, _, _ -> },
    imageCache: ImageCache,
    assetBaseUrl: String = "http://localhost:7332/assets/",
) {
    Column(modifier = Modifier.fillMaxWidth()) {
        for (child in node.children) {
            FluxTreeView(
                child,
                onButtonClick,
                routerVersion,
                onTextChange,
                imageCache = imageCache,
                assetBaseUrl = assetBaseUrl,
            )
        }
    }
}

/**
 * A native [TextField] leaf driven by the node's `text` / `placeholder` props.
 *
 * Editing the field dispatches the node's `onChangeText` handler with the new
 * value as the [HandlerEvent.payload] (the compiler binds that payload to the
 * handler's first parameter, FLUX-014), so a Flux `TextInput` is fully
 * interactive: typed characters flow into a state signal and back out as the
 * controlled `text` prop. A null/invalid handler id is a no-op.
 */
@Composable
private fun RenderTextInput(
    node: ShadowNode,
    onTextChange: (UInt, UInt, String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val props = node.observeProps()
    val propsText = props.getString(PropsIndex.TEXT_INPUT_TEXT).orEmpty()
    val placeholder = props.getString(PropsIndex.TEXT_INPUT_PLACEHOLDER).orEmpty()
    val handlerId = props.getHandler(PropsIndex.TEXT_INPUT_ON_CHANGE_TEXT)
    // The field is a controlled TextField whose displayed value is held in a local
    // MutableState so keystrokes appear instantly. Routing every keystroke through
    // the executor → VM → signal → reconcile round-trip and reading `value` straight
    // from the shadow prop would snap the field back to the stale snapshot on the
    // next recomposition and drop characters (FLUX-014: the compiler binds the
    // payload to the handler's first parameter). We seed the local state from the
    // prop once, then re-sync it only when the *external* prop actually changes
    // (e.g. the Add-task handler clears `newTask`), so in-flight typed edits are
    // never overwritten by the round-trip. `modifier` carries the row weight so the
    // field shares the row with its sibling button instead of filling the width.
    var textState by remember { mutableStateOf(propsText) }
    LaunchedEffect(propsText) {
        if (propsText != textState) textState = propsText
    }
    val focusRequester = remember { FocusRequester() }
    LaunchedEffect(Unit) { focusRequester.requestFocus() }
    androidx.compose.material3.TextField(
        value = textState,
        onValueChange = {
            textState = it
            onTextChange(handlerId, node.id, it)
        },
        placeholder = { Text(placeholder) },
        modifier = modifier.fillMaxWidth().padding(4.dp).focusRequester(focusRequester),
    )
}

/**
 * Renders an `Image` leaf (FLUX-039): fetches the `source` prop over HTTP from
 * the dev server's asset route, caches the decoded bitmap in the shared
 * [imageCache] (LRU memory + single-flight fetch), and shows a grey placeholder
 * until the bitmap arrives or if loading fails (BR-003 — never crash the host).
 *
 * Reads `width` / `height` / `resizeMode` props; absent dimensions let the image
 * fill its parent. The load runs off the main thread (the cache's async fetcher)
 * and is keyed by the resolved URL, so a repeated `Image` with the same `source`
 * is served from cache without a second network round-trip.
 */
@Composable
private fun RenderImage(
    node: ShadowNode,
    imageCache: ImageCache,
    assetBaseUrl: String,
) {
    val props = node.observeProps()
    val src = props.getString(PropsIndex.IMAGE_SOURCE).orEmpty()
    val width = props.getFloat(PropsIndex.IMAGE_WIDTH)?.let { Dp(it.toFloat()) }
    val height = props.getFloat(PropsIndex.IMAGE_HEIGHT)?.let { Dp(it.toFloat()) }
    val resizeMode =
        when (props.getString(PropsIndex.IMAGE_RESIZE_MODE)) {
            "fit" -> ContentScale.Fit
            "stretch" -> ContentScale.FillBounds
            else -> ContentScale.Crop
        }

    // Placeholder shown until the bitmap arrives or on load failure.
    var bitmap by remember(node) { mutableStateOf<androidx.compose.ui.graphics.ImageBitmap?>(null) }
    var failed by remember(node) { mutableStateOf(false) }

    LaunchedEffect(src, assetBaseUrl) {
        if (src.isEmpty()) {
            bitmap = null
            failed = false
            return@LaunchedEffect
        }
        failed = false
        bitmap = null
        val url = imageCache.resolveUrl(src, assetBaseUrl)
        when (val result = imageCache.get(url)) {
            is ImageCache.Result.Success -> {
                val decoded =
                    android.graphics.BitmapFactory.decodeByteArray(
                        result.bytes,
                        0,
                        result.bytes.size,
                    )
                bitmap = decoded?.asImageBitmap()
                failed = decoded == null
            }
            is ImageCache.Result.Failure -> failed = true
        }
    }

    val modifier =
        Modifier
            .then(width?.let { Modifier.width(it) } ?: Modifier)
            .then(height?.let { Modifier.height(it) } ?: Modifier.fillMaxWidth())
            .then(if (width == null && height == null) Modifier.fillMaxWidth() else Modifier)

    if (bitmap != null) {
        androidx.compose.foundation.Image(
            bitmap = bitmap!!,
            contentDescription = null,
            contentScale = resizeMode,
            modifier = modifier,
        )
    } else {
        // Placeholder box — grey while loading, red tint on failure (BR-003).
        androidx.compose.foundation.Canvas(modifier = modifier) {
            drawRect(
                color =
                    if (failed) {
                        Color(0xFFFF6B6B)
                    } else {
                        Color(0xFF2A2A2E)
                    },
            )
        }
    }
}

/** Reads the `gap` spacing prop of a linear container, defaulting to 0dp. */
private fun gapOf(node: ShadowNode): androidx.compose.ui.unit.Dp {
    val gap = node.props.getFloat(PropsIndex.STACK_GAP) ?: 0.0
    return gap.dp
}

/**
 * `Stack` — z-order overlay of children (FLUX-037). Children are painted in
 * source order, the last on top, spaced by the `gap` prop. Mapped from the
 * Compose `Box` the codegen emits.
 */
@Composable
private fun RenderStack(
    node: ShadowNode,
    onButtonClick: (UInt, UInt) -> Unit,
    routerVersion: Int,
    onTextChange: (UInt, UInt, String) -> Unit = { _, _, _ -> },
    imageCache: ImageCache,
    assetBaseUrl: String = "http://localhost:7332/assets/",
) {
    val gap = gapOf(node)
    Box(modifier = Modifier.fillMaxWidth()) {
        // Back-to-front stacking: later children paint above earlier ones.
        val kids = node.children
        for (i in kids.indices) {
            Box(modifier = Modifier.fillMaxWidth().padding(bottom = if (i < kids.lastIndex) gap else 0.dp)) {
                FluxTreeView(
                    kids[i],
                    onButtonClick,
                    routerVersion,
                    onTextChange,
                    imageCache = imageCache,
                    assetBaseUrl = assetBaseUrl,
                )
            }
        }
    }
}

/**
 * `Grid` — responsive grid of children (FLUX-037), laid out in row-major order
 * with [PropsIndex.GRID_COLUMNS] columns, spaced by the `gap` prop. Mapped from
 * the Compose `LazyVerticalGrid` the codegen emits.
 */
@Composable
private fun RenderGrid(
    node: ShadowNode,
    onButtonClick: (UInt, UInt) -> Unit,
    routerVersion: Int,
    onTextChange: (UInt, UInt, String) -> Unit = { _, _, _ -> },
    imageCache: ImageCache,
    assetBaseUrl: String = "http://localhost:7332/assets/",
) {
    val columns = (node.props.getInt(PropsIndex.GRID_COLUMNS) ?: 2L).toInt().coerceAtLeast(1)
    val gap = gapOf(node)
    LazyVerticalGrid(
        columns = GridCells.Fixed(columns),
        verticalArrangement = Arrangement.spacedBy(gap),
        horizontalArrangement = Arrangement.spacedBy(gap),
        modifier = Modifier.fillMaxWidth(),
    ) {
        itemsIndexed(node.children) { _, child ->
            FluxTreeView(
                child,
                onButtonClick,
                routerVersion,
                onTextChange,
                imageCache = imageCache,
                assetBaseUrl = assetBaseUrl,
            )
        }
    }
}

/**
 * `Spacer` — elastic gap that grows along the parent's main axis (FLUX-037).
 * The [PropsIndex.SPACER_FLEX] prop is the relative weight; defaults to 1.
 * Mapped from the Compose `Spacer` the codegen emits.
 */
@Composable
private fun RenderSpacer(node: ShadowNode) {
    // NOTE: a true elastic spacer (FLUX-037) applies `weight` to grow along the
    // parent's main axis, but `Modifier.weight()` is only valid inside a
    // RowScope/ColumnScope. The generic renderer invokes this composable outside
    // that scope, so the elastic behavior is deferred to a scope-aware FLUX-037
    // follow-up; here we fill the available cross-axis width so the spacer still
    // occupies space in the running app.
    Spacer(modifier = Modifier.fillMaxWidth())
}

/**
 * `SafeArea` — insets its children within the platform safe area (FLUX-037).
 * The [PropsIndex.SAFEAREA_EDGES] prop selects which edges to inset; absent
 * means all edges. Mapped from the Compose `Scaffold` the codegen emits.
 */
@Composable
private fun RenderSafeArea(
    node: ShadowNode,
    onButtonClick: (UInt, UInt) -> Unit,
    routerVersion: Int,
    onTextChange: (UInt, UInt, String) -> Unit = { _, _, _ -> },
    imageCache: ImageCache,
    assetBaseUrl: String = "http://localhost:7332/assets/",
) {
    val edges = node.props.getString(PropsIndex.SAFEAREA_EDGES)
    val inset = if (edges == "top") 24.dp else 8.dp
    Column(modifier = Modifier.fillMaxWidth().padding(inset)) {
        for (child in node.children) {
            FluxTreeView(
                child,
                onButtonClick,
                routerVersion,
                onTextChange,
                imageCache = imageCache,
                assetBaseUrl = assetBaseUrl,
            )
        }
    }
}

/**
 * `ScrollView` — a scrollable viewport for its children (FLUX-056, PRD-N).
 * The [PropsIndex.SCROLL_ORIENTATION] prop selects the scroll axis (`"vertical"`
 * default, `"horizontal"` otherwise); the matching Compose scroll modifier is
 * applied so the children can scroll past the viewport. Mapped from the Android
 * `ScrollView` / `horizontalScroll` the codegen emits.
 */
@Composable
private fun RenderScrollView(
    node: ShadowNode,
    onButtonClick: (UInt, UInt) -> Unit,
    routerVersion: Int,
    onTextChange: (UInt, UInt, String) -> Unit = { _, _, _ -> },
    imageCache: ImageCache,
    assetBaseUrl: String = "http://localhost:7332/assets/",
) {
    val orientation = node.props.getString(PropsIndex.SCROLL_ORIENTATION) ?: "vertical"
    val scrollModifier = if (orientation == "horizontal") {
        Modifier.horizontalScroll(rememberScrollState())
    } else {
        Modifier.verticalScroll(rememberScrollState())
    }
    Column(modifier = Modifier.fillMaxWidth().then(scrollModifier)) {
        for (child in node.children) {
            FluxTreeView(
                child,
                onButtonClick,
                routerVersion,
                onTextChange,
                imageCache = imageCache,
                assetBaseUrl = assetBaseUrl,
            )
        }
    }
}

/**
 * FLUX-038 overlay containers (`Modal` / `Sheet` / `Dialog`) and the FLUX-042
 * `Animate` wrapper, rendered in their degraded (pre-ADR-0048) form: the
 * content / animated subtree is hosted inside a plain container so the app
 * renders rather than blanking. The native presentation / animation is gated
 * on ADR-0048; the dev/release parity mapping is already pinned by
 * `flux-parity`.
 */
@Composable
private fun RenderOverlayContainer(
    node: ShadowNode,
    onButtonClick: (UInt, UInt) -> Unit,
    routerVersion: Int,
    onTextChange: (UInt, UInt, String) -> Unit = { _, _, _ -> },
    imageCache: ImageCache,
    assetBaseUrl: String = "http://localhost:7332/assets/",
) {
    Column(modifier = Modifier.fillMaxWidth()) {
        for (child in node.children) {
            FluxTreeView(
                child,
                onButtonClick,
                routerVersion,
                onTextChange,
                imageCache = imageCache,
                assetBaseUrl = assetBaseUrl,
            )
        }
    }
}
