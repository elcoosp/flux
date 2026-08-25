package dev.flux.app.vm

/**
 * Control-flow result of executing one instruction.
 *
 * @property index the program index to jump to (only meaningful for [JumpTo]).
 */
internal sealed interface StepResult {
    /** Execution should advance to the next instruction. */
    data object Proceed : StepResult

    /** Execution should jump to [index] in the program. */
    data class JumpTo(
        val index: Int,
    ) : StepResult
}

/**
 * Executes a single [instr], mutating [regs]/[signals] in place.
 *
 * Split out of [FluxBytecodeVM.run] so the interpreter dispatch stays a focused
 * table (AGENTS.md §1.2: functions ≤ 40 lines, files ≤ 300 lines). Any fault is
 * thrown as a [VmError] and converted at the run boundary; jumps are reported via
 * [StepResult.JumpTo] rather than captured loop state.
 *
 * @param regs the 16 VM registers (mutated in place).
 * @param instr the instruction to execute.
 * @param signals the signal graph the closure reads/writes.
 * @param offsets the byte offset of every decoded instruction (for jumps).
 * @param nextIndex the program index after [instr] (jump anchor).
 * @param strings resolves interned `StringId`s for `STR_LEN`/`STR_CONCAT`.
 * @param capabilities the `(capId, methodId) → impl` table for `CALL_CAP`.
 * @param allocated the running per-dispatch allocation counter; `ALLOC_*` and
 *   `LIST_PUSH` increment it and fault `MEMORY_EXHAUSTED` past the cap.
 * @return [StepResult.Proceed] to advance, or [StepResult.JumpTo] to branch.
 */
internal fun executeInstruction(
    regs: Array<FluxValue>,
    instr: Instruction,
    signals: SignalStore,
    offsets: List<UInt>,
    nextIndex: Int,
    strings: StringResolver,
    capabilities: CapabilityRegistry,
    allocated: FluxBytecodeVM.AllocationCounter,
): StepResult {
    val op = instr.opcode
    return when (op) {
        Opcode.NOP -> StepResult.Proceed
        Opcode.READ_SIGNAL -> {
            val dst = instr.u8(0)
            val id = instr.u32(1).toUInt()
            regs[dst] = signals.read(id) ?: FluxValue.NullVal
            StepResult.Proceed
        }
        Opcode.WRITE_SIGNAL -> {
            val id = instr.u32(0).toUInt()
            val src = instr.u8(4)
            signals.write(id, regs[src])
            StepResult.Proceed
        }
        Opcode.EQ_I64, Opcode.LT_I64, Opcode.GT_I64, Opcode.LTE_I64, Opcode.GTE_I64 -> {
            val dst = instr.u8(0)
            val (x, y) = requireInts(regs[instr.u8(1)], regs[instr.u8(2)], instr.offset)
            val r =
                when (op) {
                    Opcode.EQ_I64 -> x == y
                    Opcode.LT_I64 -> x < y
                    Opcode.GT_I64 -> x > y
                    Opcode.LTE_I64 -> x <= y
                    Opcode.GTE_I64 -> x >= y
                }
            regs[dst] = FluxValue.BoolVal(r)
            StepResult.Proceed
        }
        Opcode.ADD_I64, Opcode.SUB_I64, Opcode.MUL_I64, Opcode.DIV_I64, Opcode.MOD_I64 -> {
            val dst = instr.u8(0)
            val (x, y) = requireInts(regs[instr.u8(1)], regs[instr.u8(2)], instr.offset)
            val r =
                when (op) {
                    Opcode.ADD_I64 -> x + y
                    Opcode.SUB_I64 -> x - y
                    Opcode.MUL_I64 -> x * y
                    Opcode.DIV_I64 -> {
                        if (y == 0L) throw VmError(VmErrorKind.DIV_BY_ZERO, instr.offset)
                        x / y
                    }
                    Opcode.MOD_I64 -> {
                        if (y == 0L) throw VmError(VmErrorKind.DIV_BY_ZERO, instr.offset)
                        x % y
                    }
                }
            regs[dst] = FluxValue.IntVal(r)
            StepResult.Proceed
        }
        Opcode.NEG_I64 -> {
            regs[instr.u8(0)] = FluxValue.IntVal(-requireInt(regs[instr.u8(1)], instr.offset))
            StepResult.Proceed
        }
        Opcode.EQ_F64, Opcode.LT_F64, Opcode.GT_F64 -> {
            val dst = instr.u8(0)
            val (x, y) = requireFloats(regs[instr.u8(1)], regs[instr.u8(2)], instr.offset)
            val r =
                when (op) {
                    Opcode.EQ_F64 -> (x == y) || (x.isNaN() && y.isNaN())
                    Opcode.LT_F64 -> x < y
                    Opcode.GT_F64 -> x > y
                }
            regs[dst] = FluxValue.BoolVal(r)
            StepResult.Proceed
        }
        Opcode.ADD_F64, Opcode.SUB_F64, Opcode.MUL_F64, Opcode.DIV_F64 -> {
            val dst = instr.u8(0)
            val (x, y) = requireFloats(regs[instr.u8(1)], regs[instr.u8(2)], instr.offset)
            val r =
                when (op) {
                    Opcode.ADD_F64 -> x + y
                    Opcode.SUB_F64 -> x - y
                    Opcode.MUL_F64 -> x * y
                    Opcode.DIV_F64 -> fdiv(x, y)
                }
            regs[dst] = FluxValue.FloatVal(r)
            StepResult.Proceed
        }
        Opcode.NEG_F64 -> {
            regs[instr.u8(0)] = FluxValue.FloatVal(-requireFloat(regs[instr.u8(1)], instr.offset))
            StepResult.Proceed
        }
        Opcode.I64_TO_F64 -> {
            regs[instr.u8(0)] = FluxValue.FloatVal(requireInt(regs[instr.u8(1)], instr.offset).toDouble())
            StepResult.Proceed
        }
        Opcode.F64_TO_I64 -> {
            regs[instr.u8(0)] = FluxValue.IntVal(requireFloat(regs[instr.u8(1)], instr.offset).toLong())
            StepResult.Proceed
        }
        Opcode.AND_BOOL -> {
            val (x, y) = requireBools(regs[instr.u8(1)], regs[instr.u8(2)], instr.offset)
            regs[instr.u8(0)] = FluxValue.BoolVal(x && y)
            StepResult.Proceed
        }
        Opcode.OR_BOOL -> {
            val (x, y) = requireBools(regs[instr.u8(1)], regs[instr.u8(2)], instr.offset)
            regs[instr.u8(0)] = FluxValue.BoolVal(x || y)
            StepResult.Proceed
        }
        Opcode.NOT_BOOL -> {
            regs[instr.u8(0)] = FluxValue.BoolVal(!requireBool(regs[instr.u8(1)], instr.offset))
            StepResult.Proceed
        }
        Opcode.STR_INTERN -> {
            regs[instr.u8(0)] = FluxValue.StrVal(instr.u32(1).toUInt())
            StepResult.Proceed
        }
        Opcode.STR_EQ -> {
            val x = requireStr(regs[instr.u8(1)], instr.offset)
            val y = requireStr(regs[instr.u8(2)], instr.offset)
            regs[instr.u8(0)] = FluxValue.BoolVal(x == y)
            StepResult.Proceed
        }
        Opcode.STR_LEN -> {
            // `STR_LEN` resolves the interned id to text and reports its UTF-8
            // byte length (Appendix E §E.1). The golden ISA vectors assume the
            // oracle proxy "no live table → length is the id's digit count", so
            // the default [DecimalStringResolver] reproduces that exactly; a real
            // frame table (via [TableStringResolver]) yields genuine length.
            val id = requireStr(regs[instr.u8(1)], instr.offset)
            val text = strings.resolve(id)
            regs[instr.u8(0)] = FluxValue.IntVal(text.length.toLong())
            StepResult.Proceed
        }
        Opcode.STR_CONCAT -> {
            // `STR_CONCAT` resolves both ids to text, joins them, and interns the
            // result (Appendix E §E.1). Dynamic interning at runtime is out of
            // MLP scope (ADR-flux-0028); the default [DecimalStringResolver]
            // reproduces the oracle's `x*10_000_000 + y` proxy so the golden
            // vectors stay green, while a real frame table widens the proxy to
            // the joined text's hashed id so downstream ops observe the result.
            val x = requireStr(regs[instr.u8(1)], instr.offset)
            val y = requireStr(regs[instr.u8(2)], instr.offset)
            val resultId = strings.concat(x, y)
            regs[instr.u8(0)] = FluxValue.StrVal(resultId)
            StepResult.Proceed
        }
        Opcode.JUMP -> StepResult.JumpTo(jumpTarget(instr, nextIndex, offsets, instr.i32(0)))
        Opcode.COND_JUMP, Opcode.COND_JUMP_NOT -> {
            val taken = truthy(regs[instr.u8(0)])
            if (taken == (op == Opcode.COND_JUMP)) {
                StepResult.JumpTo(jumpTarget(instr, nextIndex, offsets, instr.i32(1)))
            } else {
                StepResult.Proceed
            }
        }
        Opcode.ALLOC_RECORD -> {
            val count = instr.u16(1)
            // Each field slot reserves 8 bytes (Appendix E §E.1); past the cap we
            // fault rather than allocate (ADR-0015 / §NFR-SEC-003).
            if (allocated.add(count.toLong() * 8L)) {
                throw VmError(VmErrorKind.MEMORY_EXHAUSTED, instr.offset)
            }
            val fields = ArrayList<FluxValue.Field>(count)
            for (i in 0 until count) {
                fields.add(FluxValue.Field(i.toUShort(), FluxValue.NullVal))
            }
            regs[instr.u8(0)] = FluxValue.RecordVal(fields)
            StepResult.Proceed
        }
        Opcode.GET_FIELD -> {
            regs[instr.u8(0)] = getField(regs[instr.u8(3)], instr.u16(1).toUShort(), instr.offset)
            StepResult.Proceed
        }
        Opcode.SET_FIELD -> {
            setField(regs, instr.u8(0), instr.u16(1).toUShort(), regs[instr.u8(3)], instr.offset)
            StepResult.Proceed
        }
        Opcode.RECORD_EQ -> {
            val x = requireRecord(regs[instr.u8(1)], instr.offset)
            val y = requireRecord(regs[instr.u8(2)], instr.offset)
            regs[instr.u8(0)] = FluxValue.BoolVal(x == y)
            StepResult.Proceed
        }
        Opcode.ALLOC_LIST -> {
            val cap = instr.u16(1)
            if (allocated.add(cap.toLong() * 8L)) {
                throw VmError(VmErrorKind.MEMORY_EXHAUSTED, instr.offset)
            }
            regs[instr.u8(0)] = FluxValue.ListVal(ArrayList(cap))
            StepResult.Proceed
        }
        Opcode.LIST_PUSH -> {
            if (allocated.add(8L)) {
                throw VmError(VmErrorKind.MEMORY_EXHAUSTED, instr.offset)
            }
            val items = requireList(regs[instr.u8(0)], instr.offset).toMutableList()
            items.add(regs[instr.u8(1)])
            regs[instr.u8(0)] = FluxValue.ListVal(items)
            StepResult.Proceed
        }
        Opcode.LIST_GET -> {
            val (items, i) = requireListIndex(regs[instr.u8(1)], instr.u8(2), instr.offset)
            regs[instr.u8(0)] = items[i]
            StepResult.Proceed
        }
        Opcode.LIST_LEN -> {
            regs[instr.u8(0)] = FluxValue.IntVal(requireList(regs[instr.u8(1)], instr.offset).size.toLong())
            StepResult.Proceed
        }
        Opcode.LIST_CONCAT -> {
            val a = requireList(regs[instr.u8(1)], instr.offset)
            val b = requireList(regs[instr.u8(2)], instr.offset)
            val items =
                ArrayList<FluxValue>(a.size + b.size).apply {
                    addAll(a)
                    addAll(b)
                }
            regs[instr.u8(0)] = FluxValue.ListVal(items)
            StepResult.Proceed
        }
        Opcode.CALL_CAP -> {
            val resultReg = instr.u8(0)
            val capId = instr.u32(1).toUInt()
            val methodId = instr.u16(5).toUShort()
            val argsReg = instr.u8(7)
            // Data-driven capability dispatch (G4): route through the injected
            // registry instead of a hardcoded `(1,1)` test. An unregistered
            // `(capId, methodId)` is a `TYPE_MISMATCH` fault, matching the
            // oracle's "capability must exist" contract.
            val impl = capabilities.lookup(capId, methodId)
            if (impl == null) {
                throw VmError(VmErrorKind.TYPE_MISMATCH, instr.offset)
            }
            val result = impl.call(regs[argsReg], signals)
            if (result == null) {
                throw VmError(VmErrorKind.TYPE_MISMATCH, instr.offset)
            }
            regs[resultReg] = result
            StepResult.Proceed
        }
        Opcode.MATCH_TAG -> {
            val value = regs[instr.u8(0)]
            val tag = instr.u32(1).toUInt()
            val matched =
                value is FluxValue.RecordVal &&
                    value.fields.firstOrNull().let { it != null && it.value is FluxValue.IntVal && it.value.value == tag.toLong() }
            if (matched) {
                StepResult.JumpTo(jumpTarget(instr, nextIndex, offsets, instr.i32(5)))
            } else {
                StepResult.Proceed
            }
        }
        Opcode.EXTRACT_FIELD -> {
            regs[instr.u8(0)] = getField(regs[instr.u8(3)], instr.u16(1).toUShort(), instr.offset)
            StepResult.Proceed
        }
        Opcode.LOAD_INT_CONST -> {
            regs[instr.u8(0)] = FluxValue.IntVal(instr.i64(1))
            StepResult.Proceed
        }
        Opcode.LOAD_FLOAT_CONST -> {
            regs[instr.u8(0)] = FluxValue.FloatVal(instr.f64(1))
            StepResult.Proceed
        }
        Opcode.LOAD_BOOL_CONST -> {
            regs[instr.u8(0)] = FluxValue.BoolVal(instr.u8(1) != 0)
            StepResult.Proceed
        }
        Opcode.LOAD_STR_CONST -> {
            regs[instr.u8(0)] = FluxValue.StrVal(instr.u32(1).toUInt())
            StepResult.Proceed
        }
        Opcode.LOAD_NULL -> {
            regs[instr.u8(0)] = FluxValue.NullVal
            StepResult.Proceed
        }
        Opcode.MOV -> {
            regs[instr.u8(0)] = regs[instr.u8(1)]
            StepResult.Proceed
        }
        Opcode.GAS_CHECK -> {
            val budget = instr.u32(0).toUInt()
            if (regs[15] is FluxValue.IntVal) {
                val gas = (regs[15] as FluxValue.IntVal).value
                if (gas < budget.toLong()) {
                    throw VmError(VmErrorKind.GAS_EXHAUSTED, instr.offset)
                }
            } else {
                throw VmError(VmErrorKind.GAS_EXHAUSTED, instr.offset)
            }
            StepResult.Proceed
        }
        Opcode.HALT -> StepResult.Proceed
    }
}
