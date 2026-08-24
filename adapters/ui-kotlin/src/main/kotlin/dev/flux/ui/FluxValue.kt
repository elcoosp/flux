package dev.flux.ui

import kotlin.collections.List as KList

/**
 * A decoded Flux value, mirroring `flux_syntax::Value` (Appendix C.1).
 *
 * On the Rust dev server values are content-addressed `Value` enums; over the
 * wire protocol (Appendix D) they arrive at the host already decoded. This
 * sealed interface is what adapters read when applying [Props] to a native
 * view. Each variant maps 1:1 to a Rust `Value` variant, so the host and the
 * dev server agree on representation without sharing code.
 */
public sealed interface FluxValue {
    /** A 64-bit signed integer (`Value::Int`). */
    public data class Int(
        val value: kotlin.Long,
    ) : FluxValue

    /** A 64-bit floating point number (`Value::Float`). */
    public data class Float(
        val value: kotlin.Double,
    ) : FluxValue

    /** A boolean (`Value::Bool`). */
    public data class Bool(
        val value: kotlin.Boolean,
    ) : FluxValue

    /** A resolved interned string (`Value::Str` after string-table lookup). */
    public data class Str(
        val value: String,
    ) : FluxValue

    /** An ordered list of values (`Value::List`). */
    public data class List(
        val items: KList<FluxValue>,
    ) : FluxValue

    /** A structured record of named fields (`Value::Record`). */
    public data class Record(
        val fields: KList<Field>,
    ) : FluxValue {
        /** Returns the field stored at [index], or `null` when absent. */
        public fun get(index: UShort): FluxValue? = fields.firstOrNull { it.index == index }?.value

        /** String field at [index], or `null`. */
        public fun getString(index: UShort): String? = (get(index) as? Str)?.value

        /** Float field at [index], or `null`. */
        public fun getFloat(index: UShort): kotlin.Double? = (get(index) as? Float)?.value

        /** Integer field at [index], or `null`. */
        public fun getInt(index: UShort): kotlin.Long? = (get(index) as? Int)?.value

        /** Boolean field at [index], or [default] when absent/mistyped. */
        public fun getBool(
            index: UShort,
            default: Boolean,
        ): Boolean = (get(index) as? Bool)?.value ?: default
    }

    /** A reference into the host handler (closure) table (`Value::HandlerRef`). */
    public data class HandlerRef(
        val handlerId: UInt,
    ) : FluxValue

    /** The unit/null value (`Value::Null`). */
    public data object Null : FluxValue

    /** A single named field of a [Record], keyed by its component-local index. */
    public data class Field(
        val index: UShort,
        val value: FluxValue,
    )
}
