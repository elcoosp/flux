package dev.flux.host.vm

/**
 * The Flux VM opcode vocabulary, normative per Appendix E §E.1.
 *
 * These byte values are a wire contract shared with the Rust reference oracle
 * (`flux-vm-ref`) and the Swift runtime (`FluxBytecodeVM`): the golden ISA
 * vectors under `/tests/isa-vectors/` are consumed by all three. Adding an
 * opcode requires an ADR.
 *
 * [operandLen] is the number of operand bytes that follow the opcode byte; the
 * total instruction width is therefore `operandLen + 1`. The mapping is taken
 * verbatim from `Opcode::operand_len` in `crates/flux-syntax/src/opcode/decode.rs`.
 */
public enum class Opcode(
    public val byte: Int,
    public val operandLen: Int,
) {
    HALT(0x00, 0),
    NOP(0x01, 0),

    READ_SIGNAL(0x10, 5),
    WRITE_SIGNAL(0x11, 5),

    ADD_I64(0x20, 3),
    SUB_I64(0x21, 3),
    MUL_I64(0x22, 3),
    DIV_I64(0x23, 3),
    MOD_I64(0x24, 3),
    NEG_I64(0x25, 2),
    EQ_I64(0x26, 3),
    LT_I64(0x27, 3),
    GT_I64(0x28, 3),
    LTE_I64(0x29, 3),
    GTE_I64(0x2A, 3),

    ADD_F64(0x30, 3),
    SUB_F64(0x31, 3),
    MUL_F64(0x32, 3),
    DIV_F64(0x33, 3),
    NEG_F64(0x34, 2),
    EQ_F64(0x35, 3),
    LT_F64(0x36, 3),
    GT_F64(0x37, 3),
    I64_TO_F64(0x38, 2),
    F64_TO_I64(0x39, 2),

    AND_BOOL(0x40, 3),
    OR_BOOL(0x41, 3),
    NOT_BOOL(0x42, 2),

    STR_CONCAT(0x50, 3),
    STR_INTERN(0x51, 5),
    STR_EQ(0x52, 3),
    STR_LEN(0x53, 2),

    JUMP(0x60, 4),
    COND_JUMP(0x61, 5),
    COND_JUMP_NOT(0x62, 5),

    ALLOC_RECORD(0x70, 3),
    GET_FIELD(0x71, 4),
    SET_FIELD(0x72, 4),
    RECORD_EQ(0x73, 3),

    ALLOC_LIST(0x80, 3),
    LIST_PUSH(0x81, 2),
    LIST_GET(0x82, 3),
    LIST_LEN(0x83, 2),
    LIST_CONCAT(0x84, 3),
    // --- FLUX-072: dynamic-list mutation opcodes (mirror flux-vm-ref). ---
    // These were added to the Rust oracle + compiler but the host VM lacked
    // them, so `tasks.clear()` / `tasks.remove(item)` / `tasks.insert(i, x)`
    // hit an unknown-opcode branch and the list signal never changed on device.
    // Operand widths match `opcode/decode.rs` verbatim.
    LIST_INSERT(0x85, 3),
    LIST_REMOVE(0x86, 2),
    LIST_CLEAR(0x87, 1),
    LIST_REMOVE_ITEM(0x88, 2),

    CALL_CAP(0x90, 8),

    MATCH_TAG(0xA0, 9),
    EXTRACT_FIELD(0xA1, 4),

    LOAD_INT_CONST(0xB0, 9),
    LOAD_FLOAT_CONST(0xB1, 9),
    LOAD_BOOL_CONST(0xB2, 2),
    LOAD_STR_CONST(0xB3, 5),
    LOAD_NULL(0xB4, 1),
    // --- FLUX-053: null-safe access (mirror flux-vm-ref IS_NULL = 0xD1). ---
    // Sets `dst` to `true` iff `src` holds `Null` — the one null-distinguishing
    // test `truthy` cannot provide (both `Null` and `Int(0)` are falsey).
    IS_NULL(0xD1, 2),
    MOV(0xB5, 2),

    GAS_CHECK(0xC0, 4),
    TO_STRING(0xD0, 2),

    /**
     * Suspends the VM, capturing the continuation (ADR-0044, MLP v2 first-class async).
     * `AWAIT resultReg(1), futureReg(1)`: the handler parks after this instruction; the
     * executor resumes it via [FluxBytecodeVM.resume], which deposits the resolved future
     * value into `r0`.
     */
    AWAIT(0xE0, 2),
    ;

    public companion object {
        /** Decodes an opcode byte, or `null` for any unassigned value. */
        public fun fromByte(byte: Int): Opcode? = entries.firstOrNull { it.byte == byte }

        /** Every opcode defined by Appendix E §E.1, in ascending byte order. */
        public val ALL: List<Opcode> = entries.sortedBy { it.byte }
    }
}
