package dev.flux.app.vm

/**
 * Decodes a flat bytecode buffer into a list of [Instruction]s.
 *
 * Decoding is total: any unassigned opcode byte yields an [VmErrorKind.INVALID_DISPATCH]
 * error rather than an undefined variant, because the runtime must never
 * transmute a byte into an opcode. The decoder is monomorphized-agnostic — it
 * does not interpret operand *meaning*, only layout, using [Opcode.operandLen]
 * (derived from Appendix E §E.1).
 */
public object Decoder {
    /** Decodes the whole program, or throws [VmError] on a bad dispatch / truncation. */
    public fun decodeProgram(bytes: ByteArray): List<Instruction> {
        val instrs = ArrayList<Instruction>(bytes.size / 2)
        var ip = 0
        while (ip < bytes.size) {
            val offset = ip.toUInt()
            val op =
                Opcode.fromByte(bytes[ip].toInt() and 0xFF)
                    ?: throw VmError(VmErrorKind.INVALID_DISPATCH, offset)
            val n = op.operandLen
            val start = ip + 1
            val end = start + n
            if (end > bytes.size) {
                throw VmError(VmErrorKind.INDEX_OUT_OF_BOUNDS, offset)
            }
            val operands = bytes.copyOfRange(start, end)
            instrs.add(Instruction(op, offset, operands))
            ip = end
        }
        return instrs
    }
}
