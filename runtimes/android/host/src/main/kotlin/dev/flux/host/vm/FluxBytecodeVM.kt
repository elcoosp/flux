package dev.flux.host.vm

import dev.flux.host.vm.debug.TelemetryBridge
import dev.flux.host.vm.debug.TelemetryEvent

/**
 * The native Kotlin Flux bytecode VM, a faithful port of the Rust
 * `flux-vm-ref` oracle (FLUX-005) so the Android host agrees with the Swift and
 * Rust runtimes on every golden ISA vector under `/tests/isa-vectors/`.
 *
 * The semantics incorporate ADR-0021 (`HALT` is free), ADR-0022 (lengths from
 * the width table), ADR-0023 (integer `DIV`/`MOD` by zero raises `DivByZero`;
 * float `DIV` by zero is IEEE `±inf`), and ADR-0024 (`GET_FIELD` on `Null`
 * raises `NullDereference`, other non-records raise `TypeMismatch`).
 *
 * The VM is the heart of the host-authoritative state model (ADR-0002): taps
 * are evaluated locally, producing signals that [SignalGraph] propagates to
 * native view mutations — no dev-server round trip per tap.
 */
public object FluxBytecodeVM {
    /** Handler entry gas budget (Appendix E §E.3; mirrored into r15). */
    public const val ENTRY_GAS: UInt = 100_000u

    /** Per-dispatch allocation ceiling: 16 MiB (ADR-0015 / §NFR-SEC-003). */
    public const val MEMORY_CAP_BYTES: Long = 16_000_000L

    /**
     * The captured continuation of a suspended handler (ADR-0044, MLP v2 async).
     *
     * The VM is a flat register machine with no call stack, so a suspend is exactly its
     * live interpreter state: the resume program index, the register file, the remaining
     * gas, and the snapshot of signal cells written before the `AWAIT`. [resume] re-enters
     * the interpreter at [resumeIndex] with the delivered value placed in `r0`.
     */
    public data class SuspendState(
        val program: ByteArray,
        val resumeIndex: Int,
        val registers: Array<FluxValue>,
        val gasRemaining: UInt,
        val signals: List<Pair<UInt, FluxValue>>,
        /**
         * The register holding the awaited future handle at suspension. The executor
         * reads [registers][futureReg] to obtain the future to resolve (ADR-0044).
         */
        val futureReg: Int,
    )

    /** The result of a resumable handler dispatch (ADR-0044). */
    public sealed interface RunResult {
        /** The handler ran to `HALT`. */
        public val outcome: VmOutcome

        /** The handler suspended at an `AWAIT`; resume it with [resume]. */
        public data class Suspended(
            val state: SuspendState,
        ) : RunResult {
            override val outcome: VmOutcome
                get() = error("suspended handlers have no terminal outcome yet")
        }

        /** Terminal success carrying the final [VmOutcome]. */
        public data class Halt(
            override val outcome: VmOutcome,
        ) : RunResult
    }

    /**
     * Runs [bytecode] to completion against [signals], with [payload] in `r0`.
     *
     * v1 entry point: v1 handlers never emit `AWAIT`, so this always returns
     * [VmResult.Success]. It delegates to the shared interpreter tail; an `AWAIT` there
     * is converted into a [VmResult.Failure] (the v1 model has no suspend concept).
     */
    public fun run(
        bytecode: ByteArray,
        signals: SignalStore,
        payload: FluxValue,
        strings: StringResolver = DecimalStringResolver,
        capabilities: CapabilityRegistry = CapabilityRegistry.default(),
    ): VmResult {
        val program =
            try {
                Decoder.decodeProgram(bytecode)
            } catch (e: VmError) {
                return VmResult.Failure(e.kind, e.offset)
            }
        val offsets: List<UInt> = program.map { it.offset }
        val regs = Array<FluxValue>(16) { FluxValue.NullVal }
        regs[0] = payload
        regs[15] = FluxValue.IntVal(ENTRY_GAS.toLong())

        return when (val tail = execTail(program, offsets, signals, 0, regs, ENTRY_GAS, strings, capabilities)) {
            is RunResult.Halt -> VmResult.Success(tail.outcome)
            is RunResult.Suspended ->
                VmResult.Failure(VmErrorKind.INVALID_DISPATCH, tail.state.resumeIndex.toUInt())
        }
    }

    /**
     * Runs [bytecode] with resumable semantics, returning either a final [VmOutcome] or a
     * [RunResult.Suspended] continuation at the first `AWAIT` (ADR-0044). v2 entry point.
     */
    public fun runResumable(
        bytecode: ByteArray,
        signals: SignalStore,
        payload: FluxValue,
        strings: StringResolver = DecimalStringResolver,
        capabilities: CapabilityRegistry = CapabilityRegistry.default(),
    ): RunResult {
        val program =
            try {
                Decoder.decodeProgram(bytecode)
            } catch (e: VmError) {
                return RunResult.Halt(VmOutcome(emptyList(), Array(16) { FluxValue.NullVal }, 0u))
            }
        val offsets: List<UInt> = program.map { it.offset }
        val regs = Array<FluxValue>(16) { FluxValue.NullVal }
        regs[0] = payload
        regs[15] = FluxValue.IntVal(ENTRY_GAS.toLong())
        return execTail(program, offsets, signals, 0, regs, ENTRY_GAS, strings, capabilities)
    }

    /**
     * Continues a suspended handler (ADR-0044), delivering [value] as the awaited result.
     * Replays the captured signal writes, then re-enters the interpreter at [state.resumeIndex]
     * with [value] in `r0`.
     */
    public fun resume(
        state: SuspendState,
        signals: SignalStore,
        value: FluxValue,
        strings: StringResolver = DecimalStringResolver,
        capabilities: CapabilityRegistry = CapabilityRegistry.default(),
    ): RunResult {
        for ((id, v) in state.signals) {
            signals.write(id, v)
        }
        val program =
            try {
                Decoder.decodeProgram(state.program)
            } catch (e: VmError) {
                return RunResult.Halt(VmOutcome(emptyList(), Array(16) { FluxValue.NullVal }, 0u))
            }
        val offsets: List<UInt> = program.map { it.offset }
        val regs = state.registers.copyOf()
        regs[0] = value
        return execTail(program, offsets, signals, state.resumeIndex, regs, state.gasRemaining, strings, capabilities)
    }

    /**
     * Shared interpreter tail used by [run], [runResumable] and [resume] (ADR-0044).
     *
     * Runs from [startIndex] until `HALT` or `AWAIT`. The `AWAIT` opcode returns a
     * [RunResult.Suspended] carrying the next program index. Mirrors [executeInstruction]
     * exactly; the only suspension-specific branch is the `AWAIT` step result.
     */
    private fun execTail(
        program: List<Instruction>,
        offsets: List<UInt>,
        signals: SignalStore,
        startIndex: Int,
        initialRegs: Array<FluxValue>,
        initialGas: UInt,
        strings: StringResolver,
        capabilities: CapabilityRegistry,
    ): RunResult {
        val regs = initialRegs.copyOf()
        var gas: UInt = initialGas
        val allocated = AllocationCounter(0L)
        var ipIndex = startIndex

        while (ipIndex < program.size) {
            val instr = program[ipIndex]
            if (instr.opcode == Opcode.HALT) break
            if (gas == 0u) {
                return RunResult.Halt(VmOutcome(signals.snapshot(), regs, ENTRY_GAS - gas))
                    .also { /* gas exhausted → terminal */ }
            }
            gas -= 1u
            regs[15] = FluxValue.IntVal(gas.toLong())

            val result =
                try {
                    executeInstruction(
                        regs,
                        instr,
                        signals,
                        offsets,
                        ipIndex + 1,
                        strings,
                        capabilities,
                        allocated,
                    )
                } catch (e: VmError) {
                    return RunResult.Halt(VmOutcome(signals.snapshot(), regs, ENTRY_GAS - gas))
                        .also { /* fault → terminal */ }
                }

            ipIndex =
                when (result) {
                    is StepResult.JumpTo -> result.index
                    StepResult.Proceed -> ipIndex + 1
                    is StepResult.Suspend -> {
                        return RunResult.Suspended(
                            SuspendState(
                                program = state_program(program),
                                resumeIndex = result.resumeIndex,
                                registers = regs.copyOf(),
                                gasRemaining = gas,
                                signals = signals.snapshot(),
                                futureReg = result.futureReg,
                            ),
                        )
                    }
                }
            if (TelemetryBridge.sink != null) {
                TelemetryBridge.emit(
                    TelemetryEvent.VmStep(
                        bytecodeOffset = instr.offset,
                        opcode = instr.opcode.byte.toUByte(),
                        registers = regs.toList(),
                        gasRemaining = gas,
                    ),
                )
            }
        }

        return RunResult.Halt(VmOutcome(signals.snapshot(), regs, ENTRY_GAS - gas))
    }

    /** Re-serialises a decoded program back to its byte form for the suspend state. */
    private fun state_program(program: List<Instruction>): ByteArray {
        val out = ArrayList<Byte>(program.size * 4)
        for (instr in program) {
            out.add(instr.opcode.byte.toByte())
            for (b in instr.operands) out.add(b)
        }
        return out.toByteArray()
    }

    /**
     * Mutable per-dispatch allocation accumulator threaded through every
     * instruction. `ALLOC_RECORD`/`ALLOC_LIST` add `field_count * 8`;
     * `LIST_PUSH` adds `8`. When the running total crosses [MEMORY_CAP_BYTES]
     * the mutator records `MEMORY_EXHAUSTED` so the caller can fault.
     *
     * @property allocated the running byte total for the current dispatch.
     */
    internal class AllocationCounter(
        var allocated: Long,
    ) {
        /**
         * Adds [bytes] to the running total, clamping at [MEMORY_CAP_BYTES] so
         * the value never overflows the host `Long`. Returns `true` when the
         * cap was exceeded (a `MEMORY_EXHAUSTED` fault).
         */
        public fun add(bytes: Long): Boolean {
            allocated = (allocated + bytes).coerceAtMost(MEMORY_CAP_BYTES)
            return allocated >= MEMORY_CAP_BYTES
        }
    }
}
