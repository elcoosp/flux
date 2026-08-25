package dev.flux.host.shadow

import dev.flux.ui.FluxNativeView
import dev.flux.ui.Props

/**
 * A node in the host render tree, created lazily from a wire [WireNode] and bound
 * to the native view produced by its adapter.
 *
 * A shadow node owns the raw [wireProps] (the decoded wire prop bag, used for
 * cheap re-apply / dirty-set compares), the materialized [props] the adapter
 * consumes, the [view] the adapter drives, the [signalDeps] this node's props
 * read (R1 dirty-set reconcile), and the [key] used for keyed reconciliation.
 * The [children] are kept in visual order; the reconciler mutates them via
 * [ShadowTree]. The prop/view types come from the adapter kit (FLUX-009).
 *
 * @property id the stable IR node id.
 * @property kind the component-local kind tag (e.g. `"text"`, `"column"`).
 * @property componentId the interned component-name id from the wire node; the
 *   [dev.flux.host.AdapterRegistry] resolves this to the dev adapter.
 * @property key the optional stable reconciliation key (`null` when absent).
 * @property isPure `true` when the component was declared `@pure` (§18.10).
 * @property wireProps the decoded wire prop bag (raw `WireValue`s + child id
 *   list). The single source of truth for dirty/unchanged comparisons.
 * @property props the materialized adapter [Props] (resolved kit values).
 * @property signalDeps the signal ids this node's props read (R1).
 * @property view the native view this node is bound to.
 * @property children the child shadow nodes in visual order.
 */
public data class ShadowNode(
    val id: UInt,
    val kind: String,
    val componentId: UInt,
    val key: UInt?,
    /** True when the component was declared `@pure` (§18.10): a pure function of props. */
    val isPure: Boolean = false,
    /** The decoded wire prop bag: raw values plus the resolved child id list. */
    var wireProps: WireProps,
    /** The materialized adapter props. Reassigned on patch updates. */
    var props: Props,
    /** Per-node signal dependencies (R1): signal ids read by this node's props. */
    val signalDeps: MutableSet<UInt>,
    val view: FluxNativeView,
    val children: MutableList<ShadowNode> = mutableListOf(),
)
