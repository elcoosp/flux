package dev.flux.app.shadow

import dev.flux.ui.FluxNativeView
import dev.flux.ui.Props

/**
 * A node in the host render tree, created lazily from a wire [Node] and bound to
 * the native view produced by its adapter.
 *
 * A shadow node owns three things: the decoded [props] for its component, the
 * [view] the adapter drives, and the [key] used for keyed reconciliation. The
 * [children] are kept in visual order; the reconciler mutates them via
 * [ShadowTree]. The prop/view types come from the adapter kit (FLUX-009); the
 * runtime consumes that contract rather than defining its own.
 *
 * @property id the stable IR node id.
 * @property kind the component-local kind tag (e.g. `\"text\"`, `\"column\"`).
 * @property componentId the interned component-name id from the wire node; the
 *   [dev.flux.app.AdapterRegistry] resolves this to the dev adapter.
 * @property key the optional stable reconciliation key (`null` when absent).
 * @property props the decoded component props.
 * @property view the native view this node is bound to.
 * @property children the child shadow nodes in visual order.
 */
public data class ShadowNode(
    val id: UInt,
    val kind: String,
    val componentId: UInt,
    val key: UInt?,
    /** The decoded component props. Reassigned on patch updates. */
    var props: Props,
    val view: FluxNativeView,
    val children: MutableList<ShadowNode> = mutableListOf(),
)
