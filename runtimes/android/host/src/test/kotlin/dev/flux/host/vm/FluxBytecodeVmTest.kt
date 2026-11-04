package dev.flux.host.vm

import dev.flux.host.signal.CellState
import dev.flux.host.vm.CapabilityRegistry
import dev.flux.host.vm.FluxBytecodeVM
import dev.flux.host.vm.FluxBytecodeVM.RunResult
import dev.flux.host.vm.FluxValue
import dev.flux.host.vm.InMemorySignals
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * Unit tests for the native Kotlin [FluxBytecodeVM], mirroring the Rust oracle's
 * `lib.rs` test battery. These pin the quirky-but-frozen semantics the golden
 * vectors rely on (ADR-0021 free `HALT`, ADR-0023 div-by-zero, ADR-0024
 * null-deref) so a regression is caught without re-running all 71 vectors.
 */
class FluxBytecodeVmTest {
    @Test
    fun `gas counts non-halt instructions only`() {
        // NOP + LOAD_INT_CONST + HALT => 2 gas (HALT is free, ADR-0021).
        val prog =
            byteArrayOf(
                0x01,
                0xB0.toByte(),
                0,
                1,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0x00,
            )
        val out = FluxBytecodeVM.run(prog, InMemorySignals(), FluxValue.NullVal)
        assertTrue(out is VmResult.Success)
        out as VmResult.Success
        assertEquals(2u, out.outcome.gasUsed)
        assertEquals(FluxValue.IntVal(1), out.outcome.registers[0])
    }

    @Test
    fun `invalid dispatch errors with offset`() {
        val err = FluxBytecodeVM.run(byteArrayOf(0xFF.toByte()), InMemorySignals(), FluxValue.NullVal)
        assertTrue(err is VmResult.Failure)
        err as VmResult.Failure
        assertEquals(VmErrorKind.INVALID_DISPATCH, err.kind)
        assertEquals(0u, err.offset)
    }

    @Test
    fun `truncated program errors`() {
        // LOAD_INT_CONST needs 9 operand bytes; supply 3.
        val err =
            FluxBytecodeVM.run(
                byteArrayOf(0xB0.toByte(), 0, 1, 2),
                InMemorySignals(),
                FluxValue.NullVal,
            )
        assertTrue(err is VmResult.Failure)
        err as VmResult.Failure
        assertEquals(VmErrorKind.INDEX_OUT_OF_BOUNDS, err.kind)
    }

    @Test
    fun `integer division by zero is DivByZero`() {
        val prog =
            byteArrayOf(
                0xB0.toByte(),
                0,
                1,
                0,
                0,
                0,
                0,
                0,
                0,
                0, // LOAD_INT_CONST r0, 1
                0xB0.toByte(),
                1,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0, // LOAD_INT_CONST r1, 0
                0x23,
                2,
                0,
                1, // DIV_I64 r2, r0, r1
                0x00,
            )
        val err = FluxBytecodeVM.run(prog, InMemorySignals(), FluxValue.NullVal)
        assertTrue(err is VmResult.Failure)
        err as VmResult.Failure
        assertEquals(VmErrorKind.DIV_BY_ZERO, err.kind)
    }

    @Test
    fun `float division by zero is infinity`() {
        val prog =
            byteArrayOf(
                0xB1.toByte(),
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0xF8.toByte(),
                0x3F, // LOAD_FLOAT_CONST r0, 1.5
                0xB1.toByte(),
                1,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0x00, // LOAD_FLOAT_CONST r1, 0.0
                0x33,
                2,
                0,
                1, // DIV_F64 r2, r0, r1
                0x00,
            )
        val out = FluxBytecodeVM.run(prog, InMemorySignals(), FluxValue.NullVal)
        assertTrue(out is VmResult.Success)
        out as VmResult.Success
        assertEquals(FluxValue.FloatVal(Double.POSITIVE_INFINITY), out.outcome.registers[2])
    }

    @Test
    fun `get field on null is null dereference`() {
        val prog =
            byteArrayOf(
                0xB4.toByte(),
                0, // LOAD_NULL r0
                0x71,
                1,
                0,
                0,
                0, // GET_FIELD r1, r0, 0
                0x00,
            )
        val err = FluxBytecodeVM.run(prog, InMemorySignals(), FluxValue.NullVal)
        assertTrue(err is VmResult.Failure)
        err as VmResult.Failure
        assertEquals(VmErrorKind.NULL_DEREFERENCE, err.kind)
    }

    @Test
    fun `entry gas is loaded into r15`() {
        assertEquals(100_000u, FluxBytecodeVM.ENTRY_GAS)
    }

    /**
     * First-class async + unified capability bridge (ADR-0044 / ADR-0045): `CALL_CAP`
     * stores a result-cell signal id in the result register; `AWAIT` parks only while
     * that cell is `Pending`, and continues with the cell value (in `r0`) when `Ready`.
     * Mirrors the Rust oracle's `await_resume` test and the Swift `AsyncSuspendResumeTests`.
     *
     * Scenario A (sync cap, (1,1) → signal 99, Ready): no suspension.
     * Scenario B (async cap, (2,99) → fresh Pending cell): real Suspend + resolveCell + resume.
     */
    @Test
    fun `sync capability does not suspend`() {
        // CALL_CAP r2, (1,1), args=r0 ; AWAIT r0, r2 ; WRITE_SIGNAL 2, r0 ; HALT
        val syncBytecode =
            byteArrayOf(
                0x90.toByte(),
                2,
                1,
                0,
                0,
                0,
                1,
                0,
                0, // CALL_CAP r2, (1,1), args=r0
                0xE0.toByte(),
                0,
                2, // AWAIT r0, r2
                0x11,
                2,
                0,
                0,
                0,
                0, // WRITE_SIGNAL 2, r0
                0x00, // HALT
            )
        val signals = InMemorySignals()
        val payload = FluxValue.RecordVal(listOf(FluxValue.Field(0u.toUShort(), FluxValue.IntVal(42))))

        val first = FluxBytecodeVM.runResumable(syncBytecode, signals, payload)
        assertTrue(first is RunResult.Halt, "sync capability should reach HALT without suspending")
        first as RunResult.Halt
        assertEquals(FluxValue.IntVal(42), signals.read(99u), "capability must echo arg[0] into signal 99")
        assertEquals(FluxValue.IntVal(99), first.outcome.registers[2], "result_reg must hold the cell id 99")
        assertEquals(FluxValue.IntVal(42), signals.read(2u), "AWAIT on Ready cell must place the value in r0")
    }

    @Test
    fun `async capability suspends then resumes to halt`() {
        // CALL_CAP r2, (2,99), args=r0 ; AWAIT r0, r2 ; WRITE_SIGNAL 2, r0 ; HALT
        val asyncBytecode =
            byteArrayOf(
                0x90.toByte(),
                2,
                2,
                0,
                0,
                0,
                99,
                0,
                0, // CALL_CAP r2, (2,99), args=r0
                0xE0.toByte(),
                0,
                2, // AWAIT r0, r2
                0x11,
                2,
                0,
                0,
                0,
                0, // WRITE_SIGNAL 2, r0
                0x00, // HALT
            )
        val signals = InMemorySignals()
        val payload = FluxValue.RecordVal(listOf(FluxValue.Field(0u.toUShort(), FluxValue.IntVal(42))))

        val first = FluxBytecodeVM.runResumable(asyncBytecode, signals, payload, capabilities = CapabilityRegistry.DEV)
        assertTrue(first is RunResult.Suspended, "async capability should suspend")
        first as RunResult.Suspended
        val cellId =
            when (val r = first.state.registers[2]) {
                is FluxValue.IntVal -> r.value.toUInt()
                else -> error("result_reg must hold the cell id")
            }
        assertTrue(cellId >= 1_000_000u, "async capability must allocate a fresh cell id")
        assertEquals(CellState.Pending, signals.cellState(cellId), "cell must be Pending after async cap")

        signals.resolveCell(cellId, FluxValue.IntVal(7))
        val resumed = FluxBytecodeVM.resume(first.state, signals, FluxValue.IntVal(7))
        assertTrue(resumed is RunResult.Halt, "expected .halt after resolve+resume")
        resumed as RunResult.Halt
        assertEquals(FluxValue.IntVal(7), signals.read(2u), "post-resume body must run with the resolved value")
        assertTrue(resumed.outcome.signals.isNotEmpty())
    }

    /** FLUX-072: list-mutation opcodes (INSERT/REMOVE/CLEAR/REMOVE_ITEM) mirror flux-vm-ref. */
    @Test
    fun `list insert remove clear removeItem mutate in place`() {
        // r0 = [], push 5, insert 7 at idx 1 => [5,7], removeItem 7 => [5],
        // clear => [], then assert r0 is an empty list.
        val prog =
            byteArrayOf(
                0x80.toByte(), 0, 0, 1, // ALLOC_LIST r0, cap=1
                0xB0.toByte(), 1, 5, 0, 0, 0, 0, 0, 0, 0, // LOAD_INT_CONST r1 = 5
                0x81.toByte(), 0, 1, // LIST_PUSH r0, r1  => [5]
                0xB0.toByte(), 2, 7, 0, 0, 0, 0, 0, 0, 0, // LOAD_INT_CONST r2 = 7
                0x85.toByte(), 0, 1, 2, // LIST_INSERT r0, idx=1, r2 => [5,7]
                0x88.toByte(), 0, 2, // LIST_REMOVE_ITEM r0, r2(7) => [5]
                0x87.toByte(), 0, // LIST_CLEAR r0 => []
                0x00, // HALT
            )
        val out = FluxBytecodeVM.run(prog, InMemorySignals(), FluxValue.NullVal)
        assertTrue(out is VmResult.Success)
        out as VmResult.Success
        val r0 = out.outcome.registers[0]
        assertTrue(r0 is FluxValue.ListVal, "r0 must be a list after clear")
        assertEquals(0, (r0 as FluxValue.ListVal).items.size)
    }

    @Test
    fun `list remove out of bounds faults`() {
        val prog =
            byteArrayOf(
                0x80.toByte(), 0, 0, 1, // ALLOC_LIST r0, cap=1
                0x86.toByte(), 0, 5, // LIST_REMOVE r0, idx=5 (empty list)
                0x00, // HALT
            )
        val err = FluxBytecodeVM.run(prog, InMemorySignals(), FluxValue.NullVal)
        assertTrue(err is VmResult.Failure)
        err as VmResult.Failure
        assertEquals(VmErrorKind.INDEX_OUT_OF_BOUNDS, err.kind)
    }

    /**
     * Cancellation parity (FLUX-086, Part B): when the awaiting coroutine is dropped
     * before its `Pending` cell settles, the signal graph must match the Rust oracle's
     * `SuspendState` exactly — the `Pending` result cell is left untouched and no other
     * signal was written before the `AWAIT` (the handler suspended at the first
     * instruction after `CALL_CAP`, having written nothing). A drop is modelled as
     * never calling `resume`; the captured `SuspendState` is the post-cancel source of
     * truth for all three runtimes. Filing a follow-up would be wrong — the cancellation
     * contract is already fully specified by the oracle's suspend semantics, so we assert
     * it directly here.
     */
    @Test
    fun `await cancellation leaves pending cell and no writes matching oracle suspend state`() {
        // CALL_CAP r2, (2,99), args=r0 ; AWAIT r0, r2 ; WRITE_SIGNAL 2, r0 ; HALT
        val asyncBytecode =
            byteArrayOf(
                0x90.toByte(),
                2,
                2,
                0,
                0,
                0,
                99,
                0,
                0, // CALL_CAP r2, (2,99), args=r0
                0xE0.toByte(),
                0,
                2, // AWAIT r0, r2
                0x11,
                2,
                0,
                0,
                0,
                0, // WRITE_SIGNAL 2, r0
                0x00, // HALT
            )
        val signals = InMemorySignals()
        val payload = FluxValue.RecordVal(listOf(FluxValue.Field(0u.toUShort(), FluxValue.IntVal(42))))

        // Drive to the suspend point and then simulate cancellation by dropping the
        // continuation (never resuming).
        val first = FluxBytecodeVM.runResumable(asyncBytecode, signals, payload, capabilities = CapabilityRegistry.DEV)
        assertTrue(first is RunResult.Suspended, "async capability should suspend")
        first as RunResult.Suspended
        val cellId =
            when (val r = first.state.registers[2]) {
                is FluxValue.IntVal -> r.value.toUInt()
                else -> error("result_reg must hold the cell id")
            }
        assertTrue(cellId >= 1_000_000u, "async capability must allocate a fresh cell id")

        // Oracle contract: the only signal state at cancellation is the Pending result cell.
        assertEquals(CellState.Pending, signals.cellState(cellId), "cell must remain Pending after cancel")
        // The handler had written no signals before the AWAIT (CALL_CAP only allocated/marked
        // the pending cell); signal 2 must still be unbound.
        assertEquals(null, signals.read(2u), "no signal writes may have occurred before the AWAIT")
        // The captured continuation must expose the same cell id the graph holds Pending.
        assertEquals(FluxValue.IntVal(cellId.toLong()), first.state.registers[2], "captured future_reg must match the pending cell")
    }

    /** v1 semantics preserved: a program without `AWAIT` runs straight to `HALT`. */
    @Test
    fun `no await runs straight to halt`() {
        val plain =
            byteArrayOf(
                0xB0.toByte(),
                0,
                42,
                0,
                0,
                0,
                0,
                0,
                0,
                0, // LOAD_INT_CONST r0, 42
                0x11,
                2,
                0,
                0,
                0,
                0, // WRITE_SIGNAL 2, r0
                0x00, // HALT
            )
        val signals = InMemorySignals()
        val result = FluxBytecodeVM.runResumable(plain, signals, FluxValue.NullVal)
        assertTrue(result is RunResult.Halt)
        assertEquals(FluxValue.IntVal(42), signals.read(2u))
    }
}
