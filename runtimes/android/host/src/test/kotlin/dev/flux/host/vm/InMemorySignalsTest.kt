package dev.flux.host.vm

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class InMemorySignalsTest {
    @Test
    fun `snapshot reports only signals written during the run`() {
        val s = InMemorySignals.fromSignals(listOf(1u to FluxValue.IntVal(0), 2u to FluxValue.IntVal(0)))
        s.beginRun()
        s.write(2u, FluxValue.IntVal(5))
        assertEquals(listOf(2u to FluxValue.IntVal(5)), s.snapshot())
    }

    @Test
    fun `seeded cells are not in snapshot even if never written`() {
        val s = InMemorySignals.fromSignals(listOf(1u to FluxValue.IntVal(7)))
        s.beginRun()
        assertEquals(emptyList<Pair<UInt, FluxValue>>(), s.snapshot())
    }
}
