package dev.flux.host.vm

/**
 * Result of running a handler to completion ([FluxBytecodeVM.run]).
 *
 * Mirrors the observable outcome of the Rust `flux-vm-ref` oracle so the Kotlin
 * runtime can be checked against the same golden ISA vectors.
 *
 * @property signals final values of all signal cells that were written.
 * @property registers final values of the 16 registers (r0 = entry payload, r15 = remaining gas).
 * @property gasUsed number of non-`HALT` instructions executed (ADR-0021).
 */
public data class VmOutcome(
    val signals: List<Pair<UInt, FluxValue>>,
    val registers: Array<FluxValue>,
    val gasUsed: UInt,
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is VmOutcome) return false
        return signals == other.signals &&
            registers.contentEquals(other.registers) &&
            gasUsed == other.gasUsed
    }

    override fun hashCode(): Int {
        var h = signals.hashCode()
        h = 31 * h + registers.contentHashCode()
        h = 31 * h + gasUsed.hashCode()
        return h
    }
}

/**
 * The outcome of a VM run: either a successful [VmOutcome] or a [VmErrorKind]
 * fault located at a bytecode offset. The interpreter never lets a [VmError]
 * escape; [FluxBytecodeVM.run] converts it at its boundary.
 */
public sealed interface VmResult {
    /** The handler produced a value with no fault. */
    public data class Success(
        val outcome: VmOutcome,
    ) : VmResult

    /** The handler faulted before producing a value. */
    public data class Failure(
        val kind: VmErrorKind,
        val offset: UInt,
    ) : VmResult
}
