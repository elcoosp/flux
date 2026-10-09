package dev.flux.host.signal

import dev.flux.host.vm.FluxValue
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class SignalGraphTest {
    /**
     * F2: `snapshot()` must report only signals *written since `beginRun`*, not
     * every seeded cell. The Rust oracle filters on a `written` set for exactly
     * this reason (see `flux-vm-ref/src/vm.rs`); Kotlin drifted and reported the
     * whole graph, which defeated dirty-subset reconciliation and made `resume`
     * replay every signal back into the store (clobbering unrelated writes made
     * during the suspend window).
     */
    @Test
    fun `snapshot reports only signals written during the run`() {
        val g = SignalGraph()
        g.seed(listOf(1u to FluxValue.IntVal(0), 2u to FluxValue.IntVal(0)))
        g.beginRun()
        g.write(2u, FluxValue.IntVal(5))
        assertEquals(listOf(2u to FluxValue.IntVal(5)), g.snapshot())
    }

    @Test
    fun `seeded cells are not in snapshot even if never written`() {
        val g = SignalGraph()
        g.seed(listOf(1u to FluxValue.IntVal(0), 2u to FluxValue.IntVal(0)))
        g.beginRun()
        assertEquals(emptyList<Pair<UInt, FluxValue>>(), g.snapshot())
    }

    @Test
    fun `resolveCell does not appear in snapshot`() {
        // Result cells are not user-visible writes; only `write()` is.
        val g = SignalGraph()
        g.beginRun()
        g.resolveCell(7u, FluxValue.IntVal(1))
        assertEquals(emptyList<Pair<UInt, FluxValue>>(), g.snapshot())
    }

    @Test
    fun `beginRun resets the written set`() {
        val g = SignalGraph()
        g.beginRun()
        g.write(1u, FluxValue.IntVal(0))
        g.beginRun()
        assertEquals(emptyList<Pair<UInt, FluxValue>>(), g.snapshot())
    }
}
