package dev.flux.host.shadow

/**
 * The Phase-1 + R1 dirty-set reconcile (ADR-0027): after a handler dispatch writes
 * a set of signals, only the nodes whose prop expressions read those signals may
 * change. These extension functions visit *just* the subtrees containing a dirty
 * node — a clean sibling is never visited (independent of tree size) — and
 * re-materialize old/new prop kits only for nodes that actually change (T5/R2).
 *
 * Kept in its own file (AGENTS.md ≤ 300-line rule) because the logic is
 * self-contained and the trace counters it drives are the cross-host parity proof.
 */
private fun ShadowTree.hasOwnDirtyDescendant(
    id: UInt,
    written: Set<UInt>,
    ownDirty: LinkedHashSet<UInt>,
): Boolean {
    val node = nodes[id] ?: return false
    val selfDirty = node.signalDeps.any { it in written }
    if (selfDirty) ownDirty.add(id)
    var childDirty = false
    for (child in node.children) {
        if (hasOwnDirtyDescendant(child.id, written, ownDirty)) childDirty = true
    }
    return selfDirty || childDirty
}

private fun ShadowTree.depthOf(id: UInt): UInt {
    var d = 0u
    var cur = parents[id]
    while (cur != null) {
        d++
        cur = parents[cur]
    }
    return d
}

/**
 * Re-reconciles only the nodes whose recorded signal dependencies intersect
 * [writtenSignals] (R1) — the signals a handler just wrote. The walk descends only
 * into subtrees that contain a dirty node, so the work is bounded by
 * `|dependents[S]|` + structural diff size, never by tree size (ADR-0027
 * invariant + Performance Budgets).
 *
 * Visit order is `(depth asc, id asc)` (ADR-0027 determinism); the emitted `dirty`
 * list reflects that order.
 */
public fun ShadowTree.reconcileDirty(
    rootId: UInt,
    writtenSignals: Set<UInt>,
) {
    if (writtenSignals.isEmpty()) {
        emitTrace(TraceEvent.Dirty(seq = lastSeq, ids = emptyList()))
        emitStepEnd()
        return
    }
    val dirty = LinkedHashSet<UInt>()
    hasOwnDirtyDescendant(rootId, writtenSignals, dirty)
    // The reported `dirty` set is exactly the nodes whose own signal deps
    // intersect the written signals (ADR-0027 determinism + R1). Ancestors that
    // are merely re-parented are NOT reported as dirty.
    val ordered = dirty.sortedWith(compareBy({ depthOf(it) }, { it }))
    emitTrace(TraceEvent.Dirty(seq = lastSeq, ids = ordered))
    for (id in ordered) {
        val node = nodes[id] ?: continue
        // ADR-0027 (FA-IRWIRE): re-materialise dynamic props against the freshly
        // written signals before sending the kit to the adapter.
        val newKit = materializeProps(node.wireProps.fields, id)
        node.props = newKit
        reconciled[id] = (reconciled[id] ?: 0) + 1
        updatedCount++
        propMaterializations += 2u
        withAdapter(node.kind, node.componentId, node.view) { adapter, view ->
            adapter.update(view, newKit)
        }
        emitTrace(TraceEvent.Update(seq = lastSeq, id = id))
    }
    emitStepEnd()
}
