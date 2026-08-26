package dev.flux.host.vm

/**
 * Helper predicates and field accessors for [FluxBytecodeVM].
 *
 * Kept in their own file so the interpreter body stays a focused dispatch
 * table (AGENTS.md §1.2: files ≤ 300 lines). Each helper throws a [VmError]
 * with the offending instruction's [offset] on a type or bounds violation,
 * which the interpreter converts into a [VmResult.Failure].
 *
 * [truthy] returns `true` for truthy values: `Bool(true)`, non-zero `Int`, else `false`.
 */
internal fun truthy(v: FluxValue): Boolean =
    when (v) {
        is FluxValue.BoolVal -> v.value
        is FluxValue.IntVal -> v.value != 0L
        else -> false
    }

/** IEEE-754 division: `x/0.0` is `±inf` (ADR-0023), never an error. */
internal fun fdiv(
    x: Double,
    y: Double,
): Double {
    if (y == 0.0) {
        if (x.isNaN()) return Double.NaN
        return if (x >= 0.0) Double.POSITIVE_INFINITY else Double.NEGATIVE_INFINITY
    }
    return x / y
}

internal fun requireInt(
    v: FluxValue,
    off: UInt,
): Long = (v as? FluxValue.IntVal)?.value ?: throw VmError(VmErrorKind.TYPE_MISMATCH, off)

internal fun requireFloat(
    v: FluxValue,
    off: UInt,
): Double = (v as? FluxValue.FloatVal)?.value ?: throw VmError(VmErrorKind.TYPE_MISMATCH, off)

internal fun requireBool(
    v: FluxValue,
    off: UInt,
): Boolean = (v as? FluxValue.BoolVal)?.value ?: throw VmError(VmErrorKind.TYPE_MISMATCH, off)

internal fun requireStr(
    v: FluxValue,
    off: UInt,
): UInt = (v as? FluxValue.StrVal)?.id ?: throw VmError(VmErrorKind.TYPE_MISMATCH, off)

internal fun requireInts(
    a: FluxValue,
    b: FluxValue,
    off: UInt,
): Pair<Long, Long> = requireInt(a, off) to requireInt(b, off)

internal fun requireFloats(
    a: FluxValue,
    b: FluxValue,
    off: UInt,
): Pair<Double, Double> = requireFloat(a, off) to requireFloat(b, off)

internal fun requireBools(
    a: FluxValue,
    b: FluxValue,
    off: UInt,
): Pair<Boolean, Boolean> = requireBool(a, off) to requireBool(b, off)

internal fun requireList(
    v: FluxValue,
    off: UInt,
): List<FluxValue> = (v as? FluxValue.ListVal)?.items ?: throw VmError(VmErrorKind.TYPE_MISMATCH, off)

internal fun requireRecord(
    v: FluxValue,
    off: UInt,
): List<FluxValue.Field> = (v as? FluxValue.RecordVal)?.fields ?: throw VmError(VmErrorKind.TYPE_MISMATCH, off)

internal fun requireListIndex(
    list: FluxValue,
    idx: Int,
    off: UInt,
): Pair<List<FluxValue>, Int> {
    val items = requireList(list, off)
    val i = idx
    if (i < 0 || i >= items.size) {
        throw VmError(VmErrorKind.INDEX_OUT_OF_BOUNDS, off)
    }
    return items to i
}

/**
 * Resolves a relative jump offset (relative to the *next* instruction) to a
 * program index, or `INDEX_OUT_OF_BOUNDS` if it lands outside the program.
 */
internal fun jumpTarget(
    instr: Instruction,
    nextIndex: Int,
    offsets: List<UInt>,
    offset: Int,
): Int {
    // `offsets[nextIndex]` is the byte offset of the instruction immediately
    // after the jumping instruction, which is the anchor the offset is measured
    // from (Appendix E §E.4).
    val base =
        offsets.getOrNull(nextIndex)
            ?: throw VmError(VmErrorKind.INDEX_OUT_OF_BOUNDS, instr.offset)
    val targetOffset = (base.toLong() + offset.toLong())
    if (targetOffset < 0) {
        throw VmError(VmErrorKind.INDEX_OUT_OF_BOUNDS, instr.offset)
    }
    val target = targetOffset.toUInt()
    return offsets.indexOf(target).takeIf { it >= 0 }
        ?: throw VmError(VmErrorKind.INDEX_OUT_OF_BOUNDS, instr.offset)
}

/** Reads record field [idx]; `Null` → `NullDereference` (ADR-0024). */
internal fun getField(
    obj: FluxValue,
    idx: UShort,
    off: UInt,
): FluxValue {
    if (obj is FluxValue.NullVal) {
        throw VmError(VmErrorKind.NULL_DEREFERENCE, off)
    }
    if (obj is FluxValue.RecordVal) {
        val i = idx.toInt()
        val field =
            obj.fields.getOrNull(i)
                ?: throw VmError(VmErrorKind.INDEX_OUT_OF_BOUNDS, off)
        return field.value
    }
    throw VmError(VmErrorKind.TYPE_MISMATCH, off)
}

/** Writes record field [idx]; `Null` → `NullDereference` (ADR-0024). */
internal fun setField(
    regs: Array<FluxValue>,
    obj: Int,
    idx: UShort,
    value: FluxValue,
    off: UInt,
) {
    val target = regs[obj]
    if (target is FluxValue.NullVal) {
        throw VmError(VmErrorKind.NULL_DEREFERENCE, off)
    }
    if (target is FluxValue.RecordVal) {
        val i = idx.toInt()
        if (i < 0 || i >= target.fields.size) {
            throw VmError(VmErrorKind.INDEX_OUT_OF_BOUNDS, off)
        }
        val newFields = target.fields.toMutableList()
        newFields[i] = FluxValue.Field(idx, value)
        regs[obj] = FluxValue.RecordVal(newFields)
        return
    }
    throw VmError(VmErrorKind.TYPE_MISMATCH, off)
}

/**
 * Renders [value] as the text `TO_STRING` (0xD0, ADR-0043) produces.
 *
 * This is a cross-runtime contract: the Rust oracle, the Swift runtime and this
 * Kotlin VM must produce byte-identical text for the same value, because a
 * node's materialised props are compared against the release codegen output in
 * the parity suite. An integral `Double` keeps one fractional digit (`1.0`),
 * and a `StrVal` resolves through [strings] (falling back to its id when the
 * table has no entry, which only happens outside a live frame).
 */
internal fun renderToString(
    value: FluxValue,
    strings: StringResolver,
): String =
    when (value) {
        is FluxValue.IntVal -> value.value.toString()
        is FluxValue.FloatVal -> {
            val f = value.value
            if (f.isFinite() && f == f.toLong().toDouble()) "%.1f".format(f) else f.toString()
        }
        is FluxValue.BoolVal -> value.value.toString()
        is FluxValue.StrVal -> strings.resolve(value.id)
        is FluxValue.HandlerRefVal -> "handler(${value.handlerId})"
        is FluxValue.ListVal ->
            "[${value.items.joinToString(", ") { renderToString(it, strings) }}]"
        is FluxValue.RecordVal ->
            "{${value.fields.joinToString(", ") { "${it.index}: ${renderToString(it.value, strings)}" }}}"
        is FluxValue.NullVal -> "null"
    }
