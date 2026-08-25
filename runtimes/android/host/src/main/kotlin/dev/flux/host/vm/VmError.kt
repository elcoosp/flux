package dev.flux.host.vm

/**
 * Why a handler invocation terminated without producing a value.
 *
 * The error kinds are a superset of Appendix E §E.6: `DivByZero` is added by
 * ADR-0023 (integer division by zero must fail rather than panic) and
 * `NullDereference` vs `TypeMismatch` for `GET_FIELD` is resolved by ADR-0024.
 */
public enum class VmErrorKind {
    /** The 100,000-instruction gas budget was exhausted (Appendix E §E.3). */
    GAS_EXHAUSTED,

    /** The 16 MiB frame memory pool was exhausted. */
    MEMORY_EXHAUSTED,

    /** An index (list/record/string) fell outside its bounds. */
    INDEX_OUT_OF_BOUNDS,

    /** A field access was performed on `Null` (ADR-0024). */
    NULL_DEREFERENCE,

    /** The dispatch byte was not a valid opcode. */
    INVALID_DISPATCH,

    /** Operand types were not what the (monomorphized) opcode expected. */
    TYPE_MISMATCH,

    /** Integer division or remainder by zero (ADR-0023). */
    DIV_BY_ZERO,
}

/**
 * A VM fault with its location in the bytecode.
 *
 * Thrown internally by the interpreter and caught at the [FluxBytecodeVM.run]
 * boundary, which converts it into a [VmResult.Failure]; the interpreter never
 * lets a [VmError] escape the VM.
 *
 * @property kind the category of fault.
 * @property offset byte offset of the offending instruction in the program.
 */
public class VmError(
    public val kind: VmErrorKind,
    public val offset: UInt,
) : Exception("${kind.name} at offset $offset")
