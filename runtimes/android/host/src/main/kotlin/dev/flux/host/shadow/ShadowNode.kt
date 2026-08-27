package dev.flux.host.shadow

import androidx.compose.runtime.MutableState
import dev.flux.ui.FluxNativeView
import dev.flux.ui.Props

/**
 * A node in the host render tree, created lazily from a wire [WireNode] and bound
 * to the native view produced by its adapter.
 *
 * A shadow node owns the raw [wireProps] (the decoded wire prop bag, used for
 * cheap re-apply / dirty-set compares), the materialized [props] the adapter
 * consumes (held in a Compose [MutableState] so the renderer re-composes when
 * they change in place), the [view] the adapter drives, the [signalDeps] this
 * node's props read (R1 dirty-set reconcile), and the [key] used for keyed
 * reconciliation. The [children] are kept in visual order; the reconciler
 * mutates them via [ShadowTree]. The prop/view types come from the adapter kit
 * (FLUX-009).
 *
 * The shadow tree is mutated in place (same node instances, new `props`). Each
 * node's materialized [props] therefore live in a [MutableState] — the very
 * object [dev.flux.app.ShadowTreeRenderer] reads inside a composable — so when
 * the executor re-materialises props on a tap, Compose observes the same
 * `State` instance it renders from and re-runs the leaf. (A wrapper that merely
 * delegated to an inner `State` would be invisible to Compose's snapshot
 * tracking and the UI would freeze after the first frame.)
 *
 * @property id the stable IR node id.
 * @property kind the component-local kind tag (e.g. `\"text\"`, `\"column\"`).
 * @property componentId the interned component-name id from the wire node; the
 *   [dev.flux.host.AdapterRegistry] resolves this to the dev adapter.
 * @property key the optional stable reconciliation key (`null` when absent).
 * @property isPure `true` when the component was declared `@pure` (§18.10).
 * @property wireProps the decoded wire prop bag (raw `WireValue`s + child id
 *   list). The single source of truth for dirty/unchanged comparisons.
 * @property props the materialized adapter [Props] (resolved kit values).
 *   Reassigned on patch/dirty updates via the [propsState] `MutableState`.
 * @property propsState the `MutableState` backing [props]; the renderer reads it.
 * @property signalDeps the signal ids this node's props read (R1).
 * @property view the native view this node is bound to.
 * @property children the child shadow nodes in visual order.
 */
public class ShadowNode(
    val id: UInt,
    val kind: String,
    val componentId: UInt,
    val key: UInt?,
    /** True when the component was declared `@pure` (§18.10): a pure function of props. */
    val isPure: Boolean = false,
    /** The decoded wire prop bag: raw values plus the resolved child id list. */
    var wireProps: WireProps,
    /** The `MutableState` backing [props]; the renderer reads it to observe changes. */
    public val propsState: MutableState<Props>,
    /** Per-node signal dependencies (R1): signal ids read by this node's props. */
    val signalDeps: MutableSet<UInt>,
    val view: FluxNativeView,
    val children: MutableList<ShadowNode> = mutableListOf(),
) {
    /** The materialized adapter props. Reassignment updates the [propsState]. */
    var props: Props
        get() = propsState.value
        set(value) {
            propsState.value = value
        }
}
