package dev.flux.app.vm

/**
 * A decoded Flux value, mirroring `flux_syntax::Value` (Appendix C.1).
 *
 * Over the wire protocol (Appendix D §D.5) values arrive already decoded; the
 * VM and the conformance suite speak in these variants so the runtime and the
 * Rust `flux-vm-ref` oracle agree on representation without sharing code.
 *
 * The `IntVal`/`FloatVal`/`BoolVal`/`StrVal` names avoid shadowing the Kotlin
 * built-in types while keeping the `Value` discriminant obvious at call sites.
 */
public sealed interface FluxValue {
    /** A 64-bit signed integer (`Value::Int`). */
    public data class IntVal(
        public val value: Long,
    ) : FluxValue

    /** A 64-bit IEEE-754 float (`Value::Float`). */
    public data class FloatVal(
        public val value: Double,
    ) : FluxValue

    /** A boolean (`Value::Bool`). */
    public data class BoolVal(
        public val value: Boolean,
    ) : FluxValue

    /** An interned string, carried by its table id (`Value::Str`). */
    public data class StrVal(
        public val id: UInt,
    ) : FluxValue

    /** An ordered list of values (`Value::List`). */
    public data class ListVal(
        public val items: List<FluxValue>,
    ) : FluxValue

    /** A structured record of indexed fields (`Value::Record`). */
    public data class RecordVal(
        public val fields: List<Field>,
    ) : FluxValue

    /** A reference into the host handler (closure) table (`Value::HandlerRef`). */
    public data class HandlerRefVal(
        public val handlerId: UInt,
    ) : FluxValue

    /** The unit/null value (`Value::Null`). */
    public data object NullVal : FluxValue

    /** A single field of a [RecordVal], keyed by its component-local index. */
    public data class Field(
        public val index: UShort,
        public val value: FluxValue,
    )
}
