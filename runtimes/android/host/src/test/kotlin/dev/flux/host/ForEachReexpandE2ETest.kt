package dev.flux.host

import dev.flux.host.ReactiveDispatcher
import dev.flux.host.shadow.ShadowTree
import dev.flux.host.signal.SignalGraph
import dev.flux.host.transport.MockTransport
import dev.flux.host.vm.FluxValue
import dev.flux.host.vm.Instruction
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
 * FLUX-072 / ADR-0050 regression: a `ForEach` must re-expand its rows when the
 * backing list signal changes via a dispatch, not only at initial `build()`.
 *
 * This reproduces the reported To-Do bug: the list starts empty (so no rows
 * render), the user types a task and taps "Add task", which `tasks.append(…)`.
 * The handler writes the list signal; `reconcileDirty` then runs. If the host
 * only expanded the ForEach in `build()`, the new element never produces a row
 * and the todo list stays blank — exactly what the user sees.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class ForEachReexpandE2ETest {
    private val stdlibKinds = listOf("column", "text", "button", "row", "textinput", "screen", "router")

    private fun stdlibEntries(): List<Pair<UInt, String>> {
        val ids = (100u..106u).toList()
        return ids.zip(stdlibKinds) +
            listOf(200u to "text", 300u to "button", 500u to "screen", 600u to "router")
    }

    /** Builds a closure that appends `elem` to list signal [listSignal]. */
    private fun appendClosure(listSignal: UInt, elem: FluxValue): ByteArray {
        // READ_SIGNAL r0, listSignal ; LIST_PUSH r0, elem ; WRITE_SIGNAL listSignal, r0 ; HALT
        val b = mutableListOf<Byte>()
        fun u32(v: UInt) {
            b.add((v.toInt() and 0xFF).toByte())
            b.add((v.toInt() ushr 8 and 0xFF).toByte())
            b.add((v.toInt() ushr 16 and 0xFF).toByte())
            b.add((v.toInt() ushr 24 and 0xFF).toByte())
        }
        // READ_SIGNAL dst=0, id=listSignal
        b.add(0x10.toByte()); b.add(0); u32(listSignal)
        // LIST_PUSH dst=0, elemId=1 (r1 holds the element; we load it next)
        // Load element into r1 first.
        // LOAD_STR_CONST r1, elem.id  (if elem is StrVal)
        // Simpler: embed element as LIST_PUSH's second operand is a register id,
        // so we need the value in a register. Use LOAD_STR_CONST.
        when (elem) {
            is FluxValue.StrVal -> {
                b.add(0xB3.toByte()); b.add(1); u32(elem.id)
            }
            else -> error("test only supports StrVal elements")
        }
        // LIST_PUSH dst=0, src=1
        b.add(0x81.toByte()); b.add(0); b.add(1)
        // WRITE_SIGNAL id=listSignal, src=0
        b.add(0x11.toByte()); u32(listSignal); b.add(0)
        // HALT
        b.add(0x00.toByte())
        return b.toByteArray()
    }

    @Test
    fun `adding a task re-expands the ForEach rows`() =
        runTest {
            val dispatcher = StandardTestDispatcher(testScheduler)
            val scope = TestScope(dispatcher)
            val signals = SignalGraph()
            val listSignal = 5u
            val itemSlot = 9u
            // Empty list to start: nothing should render, mirroring a fresh todo.
            signals.seed(listOf(listSignal to FluxValue.ListVal(emptyList())))
            signals.seed(listOf(itemSlot to FluxValue.NullVal))

            val bytes =
                FrameBuilder()
                    .apply {
                        magic()
                        version(1)
                        seq(0)
                        flags(fullTree = true)
                        patchCount(0)
                        handlerCount(0)
                        stringCount(stdlibEntries().size)
                        for ((id, kind) in stdlibEntries()) stringEntry(id, kind)
                        // Root ForEach container (column-like kind 0x12) carrying the splice child.
                        node(
                            id = 1u,
                            kind = 0x12u,
                            component = 100u,
                            props = emptyList(),
                            childIds = listOf(10u),
                        )
                        // Template row: a Text showing the item (id=10).
                        node(
                            id = 10u,
                            kind = 0x10u,
                            component = 200u,
                            props = listOf(PropsIndex.TEXT_TEXT to WireValue.StrVal(7u)),
                            childIds = emptyList(),
                        )
                        // Add button (id=2) with onPress handler 5.
                        node(
                            id = 2u,
                            kind = 0x11u,
                            component = 300u,
                            props =
                                listOf(
                                    PropsIndex.BUTTON_TEXT to WireValue.StrVal(8u),
                                    PropsIndex.BUTTON_ON_PRESS to WireValue.HandlerRefVal(5u),
                                ),
                            childIds = emptyList(),
                        )
                        signalMetaEntry(1u, listOf(listSignal), itemSlot = itemSlot)
                        signalMetaEntry(2u, listOf(5u))
                        signalMetaEntry(10u, listOf(itemSlot))
                    }.build()

            val frame = FrameDeserializer.deserialize(bytes)
            val tree = ShadowTree(AdapterRegistry.fromStringTable(stdlibEntries().map { (id, k) -> StringTableEntry(id, k) }))
            val transport = MockTransport()
            val executor = FluxExecutor(tree, signals, transport, scope, ReactiveDispatcher.test(dispatcher))
            executor.materializationSignals.write(listSignal, signals.read(listSignal)!!)
            executor.materializationSignals.flush()

            val root = tree.applyFrame(frame, executor)
            assertNotNull(root)
            // Starts empty: no rows rendered.
            assertEquals(0, root!!.children.size, "empty list must render zero rows initially")

            // Tap "Add task": append a new element to the list signal.
            val closure = appendClosure(listSignal, FluxValue.StrVal(50u))
            executor.registerClosure(5u, closure)
            executor.dispatch(dev.flux.ui.HandlerEvent(5u, 0u))
            dispatcher.scheduler.runCurrent()
            signals.flush()

            // The list signal now has one element...
            val list = signals.read(listSignal) as? FluxValue.ListVal
            assertEquals(1, list?.items?.size, "list signal must have grown to 1 element")
            // ...and the ForEach must have re-expanded to one row.
            assertEquals(1, root.children.size, "ForEach must re-expand to 1 row after append")
        }
}
