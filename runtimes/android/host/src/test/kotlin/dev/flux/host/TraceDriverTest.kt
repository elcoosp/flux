package dev.flux.host

import dev.flux.host.shadow.ShadowTree
import dev.flux.host.shadow.TraceEvent
import dev.flux.host.signal.SignalGraph
import dev.flux.host.transport.MockTransport
import dev.flux.host.wire.FrameBuilder
import dev.flux.host.wire.FrameDeserializer
import dev.flux.host.wire.WireValue
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * ADR-0027 dirty-set reconcile + trace parity proof (the Android half of the
 * cross-host proof; iOS mirrors this on its `ShadowTreeReconciler`). A
 * [TraceDriver] applies a hand-built frame, then drives [FluxExecutor.dispatch]
 * over an injected test dispatcher (T12), capturing every [TraceEvent] the
 * [ShadowTree] emits (INV-2: the sink is nil in production, free otherwise).
 *
 * The goldens come from reconcile-trace-format.md / reconcile-counters-and-budgets.md:
 * `counter_1000` must produce exactly one `update`, zero `build`, and ≤ 2
 * `prop_materializations` on the dispatch step, **independent of tree size**;
 * `noop_dispatch` (writes a signal nothing reads) must produce zero update/build/
 * detach events.
 */
class TraceDriverTest {
    private val stdlibKinds = listOf("column", "text", "button", "row", "text_field", "screen", "router")
    private val stdlibEntries =
        (100u..106u).zip(stdlibKinds) +
            listOf(200u to "text", 300u to "button", 500u to "screen", 600u to "router")

    /** Builds the `counter_1000` frame: 999 filler nodes + one Text bound to signal 1. */
    private fun counter1000Bytes(): ByteArray {
        val textId = 1000u
        return FrameBuilder()
            .apply {
                magic()
                version(1)
                seq(0)
                flags(fullTree = true)
                patchCount(0)
                handlerCount(0)
                stringCount(stdlibEntries.size)
                for ((id, kind) in stdlibEntries) stringEntry(id, kind)
                // Root column (id=1) with 999 children: 2..999 filler columns + the
                // bound Text (1000).
                val childIds = (2u..1000u).toList()
                node(id = 1u, kind = 0x12u, component = 100u, props = emptyList(), childIds = childIds)
                for (i in 2u until 1000u) {
                    // Filler column with a static string prop (no signal dependency).
                    node(
                        id = i,
                        kind = 0x12u,
                        component = 100u,
                        props = listOf(0u.toUShort() to WireValue.StrVal(7u)),
                        childIds = emptyList(),
                    )
                }
                // The bound Text: prop 0 is `IntVal(1)`, read as signal 1 (R1 deps).
                node(
                    id = textId,
                    kind = 0x10u,
                    component = 200u,
                    props = listOf(0u.toUShort() to WireValue.IntVal(1L)),
                    childIds = emptyList(),
                )
            }.build()
    }

    /** Wires a [ShadowTree] + [FluxExecutor] with a capturing trace sink. */
    private fun driverWithTrace(): Pair<ShadowTree, MutableList<TraceEvent>> {
        val tree = ShadowTree(AdapterRegistry.fromStringTable(stdlibEntries.map { (id, k) -> StringTableEntry(id, k) }))
        val events = mutableListOf<TraceEvent>()
        tree.trace = { events.add(it) }
        return tree to events
    }

    @Test
    fun `counter_1000 dispatch updates exactly one node with bounded materializations`() =
        runTest {
            val (tree, events) = driverWithTrace()
            val signals = SignalGraph()
            val transport = MockTransport()
            val scope = TestScope(StandardTestDispatcher(testScheduler))
            val executor =
                FluxExecutor(tree, signals, transport, vmScope = scope, reactiveDispatcher = StandardTestDispatcher(testScheduler))
            executor.onError = { throw AssertionError("executor error: $it") }

            // Apply the 1000-node frame (build pass).
            tree.applyFrame(FrameDeserializer.deserialize(counter1000Bytes()), executor)

            // Register a handler that writes signal 1 (the bound Text's dependency).
            val writeSignal1 = counterSetClosure(1u)
            executor.registerClosure(7u, writeSignal1)

            val updatesBefore = events.count { it is TraceEvent.Update }
            val buildsBefore = events.count { it is TraceEvent.Build }
            // Dispatch the handler: only the bound Text may change.
            executor.dispatch(7u)

            val dispatchUpdates = events.count { it is TraceEvent.Update } - updatesBefore
            val dispatchBuilds = events.count { it is TraceEvent.Build } - buildsBefore
            assertEquals(1, dispatchUpdates, "exactly one node should update on the counter dispatch")
            assertEquals(0, dispatchBuilds, "no node should be (re)built on dispatch")

            // The single update must be the bound Text (id 1000).
            val updateEvents = events.filterIsInstance<TraceEvent.Update>()
            assertTrue(updateEvents.any { it.id == 1000u }, "the bound Text (1000) must be the updated node")

            // dirty set is exactly the bound Text.
            val dirty = events.filterIsInstance<TraceEvent.Dirty>().last()
            assertEquals(listOf(1000u), dirty.ids, "dirty set must be exactly the bound Text, independent of tree size")

            // step_end for the dispatch: ≤ 2 prop materializations, ≤ 1 update, 0 builds.
            val step = events.filterIsInstance<TraceEvent.StepEnd>().last()
            assertEquals(0u, step.built, "dispatch step must build nothing")
            assertEquals(1u, step.updated, "dispatch step must update exactly one node")
            assertTrue(step.propMaterializations <= 2u, "dispatch step prop materializations must be ≤ 2, got ${step.propMaterializations}")
        }

    @Test
    fun `noop_dispatch writes a signal nothing reads produces zero reconcile events`() =
        runTest {
            val (tree, events) = driverWithTrace()
            val signals = SignalGraph()
            val transport = MockTransport()
            val scope = TestScope(StandardTestDispatcher(testScheduler))
            val executor =
                FluxExecutor(tree, signals, transport, vmScope = scope, reactiveDispatcher = StandardTestDispatcher(testScheduler))
            executor.onError = { throw AssertionError("executor error: $it") }

            tree.applyFrame(FrameDeserializer.deserialize(counter1000Bytes()), executor)

            // Handler writes signal 99 — no node reads it.
            val writeSignal99 = counterSetClosure(99u)
            executor.registerClosure(8u, writeSignal99)

            val updatesBefore = events.count { it is TraceEvent.Update }
            val buildsBefore = events.count { it is TraceEvent.Build }
            val detachesBefore = events.count { it is TraceEvent.Detach }

            executor.dispatch(8u)

            assertEquals(0, events.count { it is TraceEvent.Update } - updatesBefore, "noop dispatch must not update")
            assertEquals(0, events.count { it is TraceEvent.Build } - buildsBefore, "noop dispatch must not build")
            assertEquals(0, events.count { it is TraceEvent.Detach } - detachesBefore, "noop dispatch must not detach")
            // The dirty set is empty.
            val dirty = events.filterIsInstance<TraceEvent.Dirty>().last()
            assertEquals(emptyList<UInt>(), dirty.ids)
        }

    @Test
    fun `reapplying an identical update skips the adapter call (T5 R2)`() =
        runTest {
            val (tree, events) = driverWithTrace()
            val signals = SignalGraph()
            val transport = MockTransport()
            val scope = TestScope(StandardTestDispatcher(testScheduler))
            val executor =
                FluxExecutor(tree, signals, transport, vmScope = scope, reactiveDispatcher = StandardTestDispatcher(testScheduler))
            executor.onError = { throw AssertionError("executor error: $it") }

            val bytes =
                FrameBuilder()
                    .apply {
                        magic()
                        version(1)
                        seq(0)
                        flags(fullTree = true)
                        patchCount(0)
                        handlerCount(0)
                        stringCount(stdlibEntries.size)
                        for ((id, kind) in stdlibEntries) stringEntry(id, kind)
                        node(
                            id = 1u,
                            kind = 0x12u,
                            component = 100u,
                            props = listOf(0u.toUShort() to WireValue.StrVal(7u)),
                            childIds = listOf(2u),
                        )
                        node(
                            id = 2u,
                            kind = 0x10u,
                            component = 200u,
                            props = listOf(0u.toUShort() to WireValue.StrVal(8u)),
                            childIds = emptyList(),
                        )
                    }.build()
            tree.applyFrame(FrameDeserializer.deserialize(bytes), executor)
            assertEquals(1, tree.reconcileCount(2u))

            // Re-send the SAME props for node 2 → skip_unchanged, no extra reconcile.
            val update =
                FrameBuilder()
                    .apply {
                        magic()
                        version(1)
                        seq(1)
                        flags(fullTree = false)
                        patchCount(1)
                        handlerCount(0)
                        stringCount(0)
                        patchUpdate(id = 2u, changes = listOf(0u.toUShort() to WireValue.StrVal(8u)))
                    }.build()
            tree.applyFrame(FrameDeserializer.deserialize(update), executor)

            assertEquals(1, tree.reconcileCount(2u), "identical-update node must not re-reconcile (T5 R2)")
            assertTrue(
                events.any { it is TraceEvent.SkipUnchanged && it.id == 2u },
                "identical update must emit skip_unchanged",
            )
        }

    /** Writes [value] into signal [id], then halts. */
    private fun counterSetClosure(id: UInt): ByteArray =
        byteArrayOf(
            0xB0.toByte(),
            0,
            (id.toInt() and 0xFF).toByte(),
            0,
            0,
            0,
            0,
            0,
            0,
            0, // LOAD_INT_CONST r0, id
            0x11.toByte(),
            (id.toInt() and 0xFF).toByte(),
            0,
            0,
            0,
            0, // WRITE_SIGNAL id, r0
            0x00, // HALT
        )
}
