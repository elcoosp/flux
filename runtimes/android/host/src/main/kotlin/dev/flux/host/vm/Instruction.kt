package dev.flux.host.vm

/**
 * A decoded instruction: its opcode and the raw operand bytes that follow it.
 *
 * Operands are kept as raw little-endian bytes so the interpreter can extract
 * exactly the widths each opcode expects, without a per-instruction heap
 * allocation in the hot path (AGENTS.md §3.3).
 *
 * @property opcode the decoded opcode.
 * @property offset byte offset of this instruction within the program (for diagnostics).
 * @property operands raw operand bytes (length == `opcode.operandLen`).
 */
public data class Instruction(
    val opcode: Opcode,
    val offset: UInt,
    val operands: ByteArray,
) {
    /** Reads a `u8` operand at [index] (0-based within the operand bytes). */
    public fun u8(index: Int): Int = operands[index].toInt() and 0xFF

    /** Reads a little-endian `u16` operand starting at [index]. */
    public fun u16(index: Int): Int =
        ((operands[index].toInt() and 0xFF)) or
            ((operands[index + 1].toInt() and 0xFF) shl 8)

    /** Reads a little-endian `u32` operand starting at [index]. */
    public fun u32(index: Int): Long =
        (u8(index).toLong()) or
            (u8(index + 1).toLong() shl 8) or
            (u8(index + 2).toLong() shl 16) or
            (u8(index + 3).toLong() shl 24)

    /** Reads a little-endian `i32` operand starting at [index]. */
    public fun i32(index: Int): Int = u32(index).toInt()

    /** Reads a little-endian `i64` operand starting at [index]. */
    public fun i64(index: Int): Long =
        (u8(index).toLong()) or
            (u8(index + 1).toLong() shl 8) or
            (u8(index + 2).toLong() shl 16) or
            (u8(index + 3).toLong() shl 24) or
            (u8(index + 4).toLong() shl 32) or
            (u8(index + 5).toLong() shl 40) or
            (u8(index + 6).toLong() shl 48) or
            (u8(index + 7).toLong() shl 56)

    /** Reads a little-endian `f64` operand starting at [index]. */
    public fun f64(index: Int): Double {
        val bits = i64(index)
        return Double.fromBits(bits)
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is Instruction) return false
        return opcode == other.opcode && offset == other.offset && operands.contentEquals(other.operands)
    }

    override fun hashCode(): Int = 31 * (31 * opcode.hashCode() + offset.hashCode()) + operands.contentHashCode()
}
