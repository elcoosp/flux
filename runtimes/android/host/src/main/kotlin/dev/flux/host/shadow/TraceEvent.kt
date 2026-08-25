package dev.flux.host.shadow

/**
 * A single reconcile/dispatch trace event (reconcile-trace-format.md).
 *
 * These events are the cross-host proof mechanism of ADR-0027: running the same
 * frame + dispatch script against both hosts must yield byte-identical
 * (post-normalization) traces. Every event carries a `t` discriminator and the
 * fields named verbatim in the trace grammar.
 *
 * Emission is free in production (INV-2): a [ShadowTree] only materializes these
 * when a trace sink is attached via [ShadowTree.trace], so the hot path pays
 * nothing when no driver is listening. Serialization is a hand-rolled canonical
 * JSON line (sorted keys, no whitespace) so the host needs no extra dependency.
 */
public sealed interface TraceEvent {
    /** The event discriminator (`build`, `update`, `skip_unchanged`, …). */
    public val t: String

    /** The frame sequence number this event belongs to (0 for a build pass). */
    public val seq: UInt

    /** Emits the canonical JSONL line (sorted keys, no whitespace). */
    public fun toJsonLine(): String

    /**
     * A `frame` event: one decoded frame was accepted and (if delta) its
     * patches queued.
     */
    public data class Frame(
        override val seq: UInt,
        val full: Boolean,
        val root: UInt?,
        val nodes: UInt,
        val patches: UInt,
    ) : TraceEvent {
        override val t: String get() = "frame"

        override fun toJsonLine(): String =
            "{\"full\":${full.jsonBool()},\"nodes\":$nodes,\"patches\":$patches," +
                "\"root\":${root.json()},\"seq\":$seq,\"t\":\"frame\"}"
    }

    /**
     * A `apply_patch` event: a delta frame's patch list was applied to the tree
     * (covering insert/update/remove/handler sub-kinds in one line).
     */
    public data class ApplyPatch(
        override val seq: UInt,
        val patches: UInt,
    ) : TraceEvent {
        override val t: String get() = "apply_patch"

        override fun toJsonLine(): String = "{\"patches\":$patches,\"seq\":$seq,\"t\":\"apply_patch\"}"
    }

    /** A `dispatch` event: a handler closure began evaluation. */
    public data class Dispatch(
        override val seq: UInt,
        val handler: UInt,
    ) : TraceEvent {
        override val t: String get() = "dispatch"

        override fun toJsonLine(): String = "{\"handler\":$handler,\"seq\":$seq,\"t\":\"dispatch\"}"
    }

    /** A `signals` event: the VM wrote these signal ids (ascending). */
    public data class Signals(
        override val seq: UInt,
        val ids: List<UInt>,
    ) : TraceEvent {
        override val t: String get() = "signals"

        override fun toJsonLine(): String = "{\"ids\":${ids.jsonList()},\"seq\":$seq,\"t\":\"signals\"}"
    }

    /**
     * A `dirty` event: the post-prune visit set for a dispatch, in
     * `(depth asc, id asc)` order (ADR-0027 determinism rule).
     */
    public data class Dirty(
        override val seq: UInt,
        val ids: List<UInt>,
    ) : TraceEvent {
        override val t: String get() = "dirty"

        override fun toJsonLine(): String = "{\"ids\":${ids.jsonList()},\"seq\":$seq,\"t\":\"dirty\"}"
    }

    /** A `build` event: a native view was created for [id]. */
    public data class Build(
        override val seq: UInt,
        val id: UInt,
    ) : TraceEvent {
        override val t: String get() = "build"

        override fun toJsonLine(): String = "{\"id\":$id,\"seq\":$seq,\"t\":\"build\"}"
    }

    /** A `update` event: a native view's props were re-applied for [id]. */
    public data class Update(
        override val seq: UInt,
        val id: UInt,
    ) : TraceEvent {
        override val t: String get() = "update"

        override fun toJsonLine(): String = "{\"id\":$id,\"seq\":$seq,\"t\":\"update\"}"
    }

    /**
     * A `skip_unchanged` event: a node was visited during reconcile but its raw
     * props and child-id list were identical, so no adapter call fired (Phase 1
     * cheap re-apply, ADR-0027 §Phase 1.3).
     */
    public data class SkipUnchanged(
        override val seq: UInt,
        val id: UInt,
    ) : TraceEvent {
        override val t: String get() = "skip_unchanged"

        override fun toJsonLine(): String = "{\"id\":$id,\"seq\":$seq,\"t\":\"skip_unchanged\"}"
    }

    /**
     * A `skip_pruned` event: a dirty descendant pruned because it read no signal
     * in the written set (Phase 2 subtree early-out).
     */
    public data class SkipPruned(
        override val seq: UInt,
        val id: UInt,
    ) : TraceEvent {
        override val t: String get() = "skip_pruned"

        override fun toJsonLine(): String = "{\"id\":$id,\"seq\":$seq,\"t\":\"skip_pruned\"}"
    }

    /** A `detach` event: a native view was torn down for [id]. */
    public data class Detach(
        override val seq: UInt,
        val id: UInt,
    ) : TraceEvent {
        override val t: String get() = "detach"

        override fun toJsonLine(): String = "{\"id\":$id,\"seq\":$seq,\"t\":\"detach\"}"
    }

    /**
     * A `setchildren` event: a parent re-attached its child views (count in
     * [n]).
     */
    public data class SetChildren(
        override val seq: UInt,
        val id: UInt,
        val n: UInt,
    ) : TraceEvent {
        override val t: String get() = "setchildren"

        override fun toJsonLine(): String = "{\"id\":$id,\"n\":$n,\"seq\":$seq,\"t\":\"setchildren\"}"
    }

    /** A `mount` event: a node's `onMount` lifecycle fired for [id]. */
    public data class Mount(
        override val seq: UInt,
        val id: UInt,
    ) : TraceEvent {
        override val t: String get() = "mount"

        override fun toJsonLine(): String = "{\"id\":$id,\"seq\":$seq,\"t\":\"mount\"}"
    }

    /** A `cleanup` event: a node's `onCleanup` lifecycle fired for [id]. */
    public data class Cleanup(
        override val seq: UInt,
        val id: UInt,
    ) : TraceEvent {
        override val t: String get() = "cleanup"

        override fun toJsonLine(): String = "{\"id\":$id,\"seq\":$seq,\"t\":\"cleanup\"}"
    }

    /**
     * An `error` event: a VM fault or reconcile failure surfaced (kind [kind],
     * offset [offset]).
     */
    public data class Error(
        override val seq: UInt,
        val kind: String,
        val offset: UInt,
    ) : TraceEvent {
        override val t: String get() = "error"

        override fun toJsonLine(): String = "{\"kind\":\"$kind\",\"offset\":$offset,\"seq\":$seq,\"t\":\"error\"}"
    }

    /**
     * A `step_end` event: a script step completed, carrying cumulative counters.
     * `propMaterializations` is the Phase-1 smoking gun (must go
     * `3N → ≤ 2·changed + built`).
     */
    public data class StepEnd(
        override val seq: UInt,
        val i: UInt,
        val built: UInt,
        val updated: UInt,
        val skippedUnchanged: UInt,
        val skippedPure: UInt,
        val detached: UInt,
        val propMaterializations: UInt,
    ) : TraceEvent {
        override val t: String get() = "step_end"

        override fun toJsonLine(): String =
            "{\"built\":$built,\"detached\":$detached,\"i\":$i," +
                "\"prop_materializations\":$propMaterializations,\"seq\":$seq," +
                "\"skipped_pure\":$skippedPure,\"skipped_unchanged\":$skippedUnchanged," +
                "\"t\":\"step_end\",\"updated\":$updated}"
    }
}

/** Renders a nullable [UInt] as canonical JSON (`null` or decimal). */
private fun UInt?.json(): String = if (this == null) "null" else this.toString()

/** Renders a [Boolean] as canonical JSON (`true`/`false`). */
private fun Boolean.jsonBool(): String = if (this) "true" else "false"

/** Renders a [List] of [UInt] as a canonical JSON array. */
private fun List<UInt>.jsonList(): String = "[" + joinToString(",") { it.toString() } + "]"
