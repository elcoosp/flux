package dev.flux.app.vm

import dev.flux.app.vm.FluxBytecodeVM
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
}
