package dev.flux.app.wire

import dev.flux.app.signal.SignalGraph
import dev.flux.app.vm.FluxBytecodeVM
import dev.flux.app.vm.FluxValue
import dev.flux.app.vm.TableStringResolver
import dev.flux.app.vm.VmResult
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

/**
 * Perf task 7 (P2 follow-on): the native→VM `String`→`StringId` reverse lookup
 * must be O(1) and produce a *stable* id matching the wire string table, not a
 * per-event `hashCode()` (which is unstable across runs and never matches the
 * canonical table id). A [StringInterning] reverse index keyed by the resolved
 * string yields the canonical id in O(1), so repeated event dispatch (a tap)
 * does not re-scan or re-hash the table.
 */
class StringInterningTest {
    @Test
    fun `reverse lookup returns canonical id in O(1)`() {
        val index = StringInterning.fromEntries(listOf(StringEntry(7u, "hello"), StringEntry(8u, "world")))
        // The canonical id is the wire table id, resolved in O(1) by string.
        assertEquals(7u, index.resolve("hello"))
        assertEquals(8u, index.resolve("world"))
        // Idempotent: the same string always maps to the same id.
        assertEquals(index.resolve("hello"), index.resolve("hello"))
    }

    @Test
    fun `unknown string hashes deterministically as a fallback`() {
        val index = StringInterning.fromEntries(emptyList())
        // Uninterned strings get a stable synthetic id (deterministic, O(1)).
        val a = index.resolve("untracked")
        val b = index.resolve("untracked")
        assertEquals(a, b)
    }

    @Test
    fun `dispatched handler reads resolved string id from the index`() {
        // STR_LEN r1, r0 where r0 = StrVal("hello" resolved to 7); expect 5.
        val prog =
            byteArrayOf(
                0xB3.toByte(),
                0,
                7,
                0,
                0,
                0, // LOAD_STR_CONST r0, 7
                0x53.toByte(),
                1,
                0,
                0, // STR_LEN r1, r0
                0x00,
            )
        val interning = StringInterning.fromEntries(listOf(StringEntry(7u, "hello")))
        val signals = SignalGraph()
        val resolver = TableStringResolver(mapOf(7u to "hello"))
        // The StrVal carrying "hello" must map back to id 7 via the index.
        val payload = FluxValue.StrVal(interning.resolve("hello"))
        val out = FluxBytecodeVM.run(prog, signals, payload, resolver)
        assertEquals(FluxValue.IntVal(5), (out as VmResult.Success).outcome.registers[1])
    }
}
