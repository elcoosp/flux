package dev.flux.ui

import kotlin.collections.List as KList

/**
 * Keyed reconciliation for adapter child lists, mirroring the host reconciler
 * (Appendix G, "Keyed Reconciliation"). Given the current children of a view
 * and the desired [targetIds] in order, it removes orphans, appends missing
 * views, and reorders in place so that the final child sequence has exactly
 * the ids in [targetIds] order.
 *
 * Children are matched by [FluxNativeView.nodeId]. The host owns the actual
 * native views; this helper only mutates the [FluxNativeView] child list
 * through [FluxNativeView.setChildAt] / [FluxNativeView.addChild] /
 * [FluxNativeView.removeChildAt], so reordering never recreates a view
 * instance — preserving native state across diffs (load-bearing for the
 * Router's screen-state preservation).
 *
 * @param view The container whose children are reconciled.
 * @param targetIds The desired child node ids, in visual order.
 * @param lookup Resolves a target id to its existing view (from a sibling
 *   cache) or `null` when the view must be supplied by the caller elsewhere.
 *   The reconciler only (re)orders and removes views already known to [view];
 *   creating brand-new views is the caller's responsibility before calling.
 */
public fun reconcileChildren(
    view: FluxNativeView,
    targetIds: KList<UInt>,
    lookup: (UInt) -> FluxNativeView?,
) {
    val existing = view.children().associateBy { it.nodeId }
    val targetViews = targetIds.mapNotNull { id -> existing[id] ?: lookup(id) }

    // Rebuild the child list from reused instances. Existing views keep their
    // identity (state preserved); orphans drop out; brand-new views come from
    // [lookup]. We clear first then re-add the surviving instances in [targetIds]
    // order — no view is ever recreated, so native state survives the diff.
    if (view.childCount() != 0) view.clearChildren()
    for (child in targetViews) {
        view.addChild(child)
    }
}
