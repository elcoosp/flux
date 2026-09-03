package dev.flux.host

import dev.flux.host.ReactiveDispatcher
import dev.flux.host.shadow.ShadowTree
import dev.flux.host.signal.SignalGraph
import dev.flux.host.transport.MockTransport
import dev.flux.host.vm.FluxValue
import dev.flux.host.wire.FrameBuilder
import dev.flux.host.wire.FrameDeserializer
import dev.flux.host.wire.WireValue
import dev.flux.ui.HandlerEvent
import dev.flux.ui.PropsIndex
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNotNull
import org.junit.jupiter.api.Test

/**
 * FLUX-092 regression: tapping "Remove" on a ForEach row must seed the shared
 * `itemSlot` with THAT row's element, not the last row's.
 *
 * The list contains `Buy milk / Walk dog / Do taxes / Call mom`. Calling
 * `seedRowContext(row2DerivedId)` must leave `itemSlot` holding
 * "Walk dog" (StrVal 8u), not "Call mom" (StrVal 10u).
 */
@OptIn(ExperimentalCoroutinesApi::class)
class ForEachRowContextTest {
    private val stdlibKinds = listOf("column", "text", "button", "row", "textinput", "screen", "router")

    private fun stdlibEntries(): List<Pair<UInt, String>> {
        val ids = (100u..106u).toList()
        return ids.zip(stdlibKinds)
    }

    @Test
    fun `seedRowContext picks the correct per-row element`() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        val scope = TestScope(dispatcher)
        val signals = SignalGraph()
        val listSignal = 5u
        val itemSlot = 9u

        val initialList = listOf(
            FluxValue.StrVal(7u),
            FluxValue.StrVal(8u),
            FluxValue.StrVal(9u),
            FluxValue.StrVal(10u),
        )
        signals.seed(listOf(
            listSignal to FluxValue.ListVal(initialList),
            itemSlot to FluxValue.NullVal,
        ))

        val stringEntries = ArrayList<StringTableEntry>()
        stringEntries += stdlibEntries().map { (id, k) -> StringTableEntry(id, k) }
        stringEntries += listOf(200u to "text", 300u to "button", 6u to "Remove", 7u to "Buy milk", 8u to "Walk dog", 9u to "Do taxes", 10u to "Call mom")
            .map { (id, k) -> StringTableEntry(id, k) }

        val bytes = FrameBuilder().apply {
            magic()
            version(1)
            seq(0)
            flags(fullTree = true)
            patchCount(0)
            handlerCount(0)
            stringCount(stringEntries.size)
            for ((id, kind) in stringEntries) stringEntry(id, kind)

            // Root container with ForEach child + Add button.
            node(
                id = 1u,
                kind = 0x10u,
                component = 100u,
                props = emptyList(),
                childIds = listOf(20u, 2u),
            )
            // ForEach node (id=20) with itemSlot=9.
            node(
                id = 20u,
                kind = 0x12u,
                component = 100u,
                props = emptyList(),
                childIds = listOf(10u),
            )
            signalMetaEntry(20u, listOf(listSignal), itemSlot = itemSlot)
            // Template row: Text showing item (id=10).
            node(
                id = 10u,
                kind = 0x10u,
                component = 200u,
                props = listOf(PropsIndex.TEXT_TEXT to WireValue.StrVal(7u)),
                childIds = emptyList(),
            )
            signalMetaEntry(10u, listOf(itemSlot))
            // Add button (id=2).
            node(
                id = 2u,
                kind = 0x11u,
                component = 300u,
                props = listOf(
                    PropsIndex.BUTTON_TEXT to WireValue.StrVal(6u),
                ),
                childIds = emptyList(),
            )
            stateSeed(listSignal, WireValue.ListVal(listOf(
                WireValue.StrVal(7u),
                WireValue.StrVal(8u),
                WireValue.StrVal(9u),
                WireValue.StrVal(10u),
            )))
        }.build()

        val frame = FrameDeserializer.deserialize(bytes)
        val tree = ShadowTree(AdapterRegistry.fromStringTable(stringEntries))
        val transport = MockTransport()
        val executor = FluxExecutor(tree, signals, transport, scope, ReactiveDispatcher.test(dispatcher))
        executor.materializationSignals.write(listSignal, signals.read(listSignal)!!)
        executor.materializationSignals.flush()

        val root = tree.applyFrame(frame, executor)
        assertNotNull(root)

        // Row 2 (index 1 = "Walk dog"). The derived ID is the cloned row's
        // own id: deriveForEachChildId(deriveForEachRowId(20u,1u), templateRowId=10u)
        val row2DerivedId = ((20u * 2654435761u + 1u * 40503u + 0x9E3779B9u) * 2654435761u xor (10u * 40503u)) xor 0x55555555u

        // Seed row 2's context → itemSlot should hold StrVal(8u) = "Walk dog".
        tree.seedRowContext(row2DerivedId)
        executor.materializationSignals.flush()
        assertEquals(
            FluxValue.StrVal(8u),
            executor.materializationSignals.read(itemSlot),
            "seedRowContext(row2) must set itemSlot to Walk dog"
        )

        // Row 4 (index 3 = "Call mom"). deriveForEachChildId(deriveForEachRowId(20u,3u), 10u)
        val row4DerivedId = ((20u * 2654435761u + 3u * 40503u + 0x9E3779B9u) * 2654435761u xor (10u * 40503u)) xor 0x55555555u

        // Seed row 4's context → itemSlot should hold StrVal(10u) = "Call mom".
        tree.seedRowContext(row4DerivedId)
        executor.materializationSignals.flush()
        assertEquals(
            FluxValue.StrVal(10u),
            executor.materializationSignals.read(itemSlot),
            "seedRowContext(row4) must set itemSlot to Call mom"
        )

        // Back to row 2 — must NOT hold the row 4 value.
        tree.seedRowContext(row2DerivedId)
        executor.materializationSignals.flush()
        assertEquals(
            FluxValue.StrVal(8u),
            executor.materializationSignals.read(itemSlot),
            "seedRowContext(row2) must still hold Walk dog, not stale Call mom"
        )
    }
}
