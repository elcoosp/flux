package dev.flux.ui

import kotlin.collections.List as KList

/**
 * A flat, ordered map of component-local prop indices to decoded values.
 *
 * Mirrors `flux_ir::Props` (Appendix C.1): a `Vec<(PropIdx, Value)>` keyed by a
 * per-component `u16` field index. Adapters read props through the typed
 * accessors ([getString], [getBool], [getRecord], ...) rather than by raw
 * index, so missing/renamed fields degrade to `null`/default instead of
 * throwing. The contract a dev adapter binds against is exactly these accessors
 * plus the record layouts documented in Appendix F.
 */
public data class Props(
    val fields: KList<Props.Field>,
) {
    /**
     * A single prop field: a component-local index and its decoded value.
     * Field order within a [Props] is unspecified and must not be relied on.
     */
    public data class Field(
        val index: UShort,
        val value: FluxValue,
    )

    /** Returns the [FluxValue] stored at [index], or `null` if absent. */
    public fun get(index: UShort): FluxValue? = fields.firstOrNull { it.index == index }?.value

    /** String value at [index], or `null` when absent or not a string. */
    public fun getString(index: UShort): String? = (get(index) as? FluxValue.Str)?.value

    /** Integer value at [index], or `null` when absent or not an integer. */
    public fun getInt(index: UShort): kotlin.Long? = (get(index) as? FluxValue.Int)?.value

    /** Float value at [index], or `null` when absent or not a float. */
    public fun getFloat(index: UShort): kotlin.Double? = (get(index) as? FluxValue.Float)?.value

    /** Boolean value at [index], falling back to [default] when absent/mistyped. */
    public fun getBool(
        index: UShort,
        default: Boolean,
    ): Boolean = (get(index) as? FluxValue.Bool)?.value ?: default

    /** Record value at [index], or `null` when absent or not a record. */
    public fun getRecord(index: UShort): FluxValue.Record? = get(index) as? FluxValue.Record

    /** List value at [index], or `null` when absent or not a list. */
    public fun getList(index: UShort): KList<FluxValue>? = (get(index) as? FluxValue.List)?.items

    /**
     * Handler id at [index]. Returns `0` when the prop is absent or not a
     * handler reference — `0` is the reserved "no handler" id in the IR.
     */
    public fun getHandler(index: UShort): UInt = (get(index) as? FluxValue.HandlerRef)?.handlerId ?: 0u

    // Audit D5: the server lowers Color/Font records POSITIONALLY (fields
    // 0..3 / 0..2). The previous FNV-name lookups never matched, so colors
    // and fonts were silently dropped on Android. Field order mirrors
    // adapters/ui-swift Color.swift + TextAdapter.swift.

    /** Float value at the record's positional [slot], or `null`. */
    private fun FluxValue.Record.floatAt(slot: Int): kotlin.Double? =
        (fields.getOrNull(slot)?.value as? FluxValue.Float)?.value

    private fun FluxValue.Record.stringAt(slot: Int): String? {
        return (fields.getOrNull(slot)?.value as? FluxValue.Str)?.value
    }

    /** Int value at the record's positional [slot], or `null`. (audit D6) */
    private fun FluxValue.Record.intAt(slot: Int): kotlin.Long? =
        (fields.getOrNull(slot)?.value as? FluxValue.Int)?.value


    /** Decodes the `Color` record at [index] into a [FluxColor], or `null`. */
    public fun getColor(index: UShort): FluxColor? {
        val record = getRecord(index) ?: return null
        val red = record.floatAt(0) ?: return null
        val green = record.floatAt(1) ?: return null
        val blue = record.floatAt(2) ?: return null
        val alpha = record.floatAt(3) ?: 1.0
        return FluxColor(red, green, blue, alpha)
    }

    /** Decodes the `Font` record at [index] into a [FluxFont], or `null`. */
    public fun getFont(index: UShort): FluxFont? {
        val record = getRecord(index) ?: return null
        val size = record.floatAt(0) ?: return null
        return FluxFont(
            size = size,
            weight = record.stringAt(1),
            family = record.stringAt(2),
        )
    }

    public companion object {
        /** An empty prop bag — used when a node carries no props. */
        public val EMPTY: Props = Props(emptyList())
    }
}
