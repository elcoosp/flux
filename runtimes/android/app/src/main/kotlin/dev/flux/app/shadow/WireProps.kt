package dev.flux.app.shadow

import dev.flux.app.wire.WireValue

/**
 * The decoded wire prop bag for a node, kept raw (no kit-value resolution) so the
 * reconciler can compare props by id-based equality (ADR-0027 INV-1) and compute
 * a content hash without materializing adapter [dev.flux.ui.Props].
 *
 * Carrying the resolved `childIds` alongside the fields lets the cheap re-apply
 * skip (T5 / Phase 1.3) check *both* prop and structural identity in one
 * comparison, so a node whose props and children are unchanged fires no adapter
 * call and emits a `skip_unchanged` trace event instead.
 *
 * @property fields the `(prop_index, raw_value)` pairs as decoded from the wire.
 * @property childIds the ordered child node ids, used for unchanged detection.
 */
public data class WireProps(
    val fields: List<Pair<UShort, WireValue>>,
    val childIds: List<UInt>,
)
