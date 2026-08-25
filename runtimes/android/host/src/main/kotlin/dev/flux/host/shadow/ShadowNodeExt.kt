package dev.flux.host.shadow

import dev.flux.ui.FluxValue
import dev.flux.ui.PropsIndex

/**
 * Engine-level read helpers for [ShadowNode] props, shared by the host tests
 * and any renderer that projects the shadow tree onto native UI. They are part
 * of the engine (not the Compose renderer) because they only read the resolved
 * [dev.flux.ui.FluxValue]s that the shadow tree carries — the renderer in the
 * `:app` module draws directly from `node.props` and does not depend on these.
 */

/** Reads the text string prop off a node, for test assertions. */
public fun ShadowNode.displayText(): String? = props.getString(PropsIndex.TEXT_TEXT)

/** Reads the bound handler id off a button node, for test assertions. */
public fun ShadowNode.buttonHandlerId(): UInt = props.getHandler(PropsIndex.BUTTON_ON_CLICK)

/** `true` when [value] is a resolved (non-id) string, used by tests. */
public fun FluxValue.isResolvedString(): Boolean = this is FluxValue.Str
