package dev.flux.app.vm

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

    /**
     * Runs [bytecode] to completion against [signals], with [payload] in `r0`.
     *
     * Never throws: a [VmError] raised while decoding or executing is converted
     * into a [VmResult.Failure] carrying its [VmErrorKind] and bytecode offset.
     */
    public fun run(
        bytecode: ByteArray,
        signals: SignalStore,
        payload: FluxValue,
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

        var gas: UInt = ENTRY_GAS
        var ipIndex = 0

        while (ipIndex < program.size) {
            val instr = program[ipIndex]
            if (instr.opcode == Opcode.HALT) break
            if (gas == 0u) {
                return VmResult.Failure(VmErrorKind.GAS_EXHAUSTED, instr.offset)
            }
            gas -= 1u
            // Mirror the live gas budget into r15 (Appendix E §E.3; ADR-0021 says
            // the budget register decrements as instructions run).
            regs[15] = FluxValue.IntVal(gas.toLong())

            val result =
                try {
                    executeInstruction(regs, instr, signals, offsets, ipIndex + 1)
                } catch (e: VmError) {
                    return VmResult.Failure(e.kind, e.offset)
                }

            ipIndex =
                when (result) {
                    is StepResult.JumpTo -> result.index
                    StepResult.Proceed -> ipIndex + 1
                }
        }

        return VmResult.Success(VmOutcome(signals.snapshot(), regs, ENTRY_GAS - gas))
    }
}
