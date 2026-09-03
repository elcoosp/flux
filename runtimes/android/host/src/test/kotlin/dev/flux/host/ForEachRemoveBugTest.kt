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
 * Reproduces the reported Android-only bug: after adding a task to a ForEach,
 * removing a seeded task removes the WRONG row (the last inserted one).
 *
 * Flow:
 * 1. Build ForEach with [A, B, C] (seeded)
 * 2. Dispatch append(D) → list becomes [A, B, C, D]
 * 3. Dispatch remove(A) → list should become [B, C, D]
 *
 * Bug: remove(A) removes D instead, leaving [A, B, C].
 */
@OptIn(ExperimentalCoroutinesApi::class)
class ForEachRemoveBugTest {
    private val stdlibKinds = listOf("column", "text", "button", "row", "textinput", "screen", "router")

    private fun stdlibEntries(): List<Pair<UInt, String>> {
        val ids = (100u..106u).toList()
        return ids.zip(stdlibKinds) +
            listOf(200u to "text", 300u to "button", 500u to "screen", 600u to "router")
    }

    /** Builds a closure that appends `elem` to list signal [listSignal]. */
    private fun appendClosure(listSignal: UInt, elem: FluxValue): ByteArray {
        val b = mutableListOf<Byte>()
        fun u32(v: UInt) {
            b.add((v.toInt() and 0xFF).toByte())
            b.add((v.toInt() ushr 8 and 0xFF).toByte())
            b.add((v.toInt() ushr 16 and 0xFF).toByte())
            b.add((v.toInt() ushr 24 and 0xFF).toByte())
        }
        // READ_SIGNAL r0, listSignal
        b.add(0x10.toByte()); b.add(0); u32(listSignal)
        // LOAD_STR_CONST r1, elem.id
        when (elem) {
            is FluxValue.StrVal -> { b.add(0xB3.toByte()); b.add(1); u32(elem.id) }
            else -> error("test only supports StrVal elements")
        }
        // LIST_PUSH r0, r1
        b.add(0x81.toByte()); b.add(0); b.add(1)
        // WRITE_SIGNAL listSignal, r0
        b.add(0x11.toByte()); u32(listSignal); b.add(0)
        // HALT
        b.add(0x00.toByte())
        return b.toByteArray()
    }

    /**
     * Builds a closure that removes the task read from `itemSlot` from the list.
     * Mirrors `|| { tasks.remove(task) }` where `task` comes from itemSlot.
     *   READ_SIGNAL r0, listSignal
     *   READ_SIGNAL r1, itemSlot
     *   LIST_REMOVE_ITEM r0, r1
     *   WRITE_SIGNAL listSignal, r0
     *   HALT
     */
    private fun removeByItemSlotClosure(listSignal: UInt, itemSlot: UInt): ByteArray {
        val b = mutableListOf<Byte>()
        fun u32(v: UInt) {
            b.add((v.toInt() and 0xFF).toByte())
            b.add((v.toInt() ushr 8 and 0xFF).toByte())
            b.add((v.toInt() ushr 16 and 0xFF).toByte())
            b.add((v.toInt() ushr 24 and 0xFF).toByte())
        }
        // READ_SIGNAL r0, listSignal
        b.add(0x10.toByte()); b.add(0); u32(listSignal)
        // READ_SIGNAL r1, itemSlot
        b.add(0x10.toByte()); b.add(1); u32(itemSlot)
        // LIST_REMOVE_ITEM r0, r1
        b.add(0x88.toByte()); b.add(0); b.add(1)
        // WRITE_SIGNAL listSignal, r0
        b.add(0x11.toByte()); u32(listSignal); b.add(0)
        // HALT
        b.add(0x00.toByte())
        return b.toByteArray()
    }

    @Test
    fun `remove seeded task after adding removes the correct row`() =
        runTest {
            val dispatcher = StandardTestDispatcher(testScheduler)
            val scope = TestScope(dispatcher)
            val signals = SignalGraph()
            val listSignal = 5u
            val itemSlot = 9u

            // Seeded tasks: A=50u, B=51u, C=52u
            val initialList = listOf(
                FluxValue.StrVal(50u),
                FluxValue.StrVal(51u),
                FluxValue.StrVal(52u),
            )
            signals.seed(listOf(listSignal to FluxValue.ListVal(initialList)))
            signals.seed(listOf(itemSlot to FluxValue.NullVal))

            val stringEntries = ArrayList<StringTableEntry>()
            stringEntries += stdlibEntries().map { (id, k) -> StringTableEntry(id, k) }
            stringEntries += listOf(
                200u to "text", 300u to "button",
                50u to "A", 51u to "B", 52u to "C", 53u to "D"
            ).map { (id, k) -> StringTableEntry(id, k) }

            val bytes = FrameBuilder().apply {
                magic()
                version(1)
                seq(0)
                flags(fullTree = true)
                patchCount(0)
                handlerCount(0)
                stringCount(stringEntries.size)
                for ((id, kind) in stringEntries) stringEntry(id, kind)
                // Root ForEach container (id=1)
                node(
                    id = 1u,
                    kind = 0x12u,
                    component = 100u,
                    props = emptyList(),
                    childIds = listOf(10u),
                )
                // Template row: a Text showing the item (id=10)
                node(
                    id = 10u,
                    kind = 0x10u,
                    component = 200u,
                    props = listOf(PropsIndex.TEXT_TEXT to WireValue.StrVal(50u)),
                    childIds = emptyList(),
                )
                signalMetaEntry(1u, listOf(listSignal), itemSlot = itemSlot)
                signalMetaEntry(10u, listOf(itemSlot))
                stateSeed(listSignal, WireValue.ListVal(listOf(
                    WireValue.StrVal(50u),
                    WireValue.StrVal(51u),
                    WireValue.StrVal(52u),
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
            assertEquals(3, root!!.children.size, "ForEach with 3-element list must render 3 rows")

            // Register append handler (handlerId=5)
            val appendHandler = appendClosure(listSignal, FluxValue.StrVal(53u))
            executor.registerClosure(5u, appendHandler)

            // Register remove-by-itemSlot handler (handlerId=6)
            val removeHandler = removeByItemSlotClosure(listSignal, itemSlot)
            executor.registerClosure(6u, removeHandler)

            // Tap "Add" button → append D
            executor.dispatch(HandlerEvent(5u, 0u))
            dispatcher.scheduler.runCurrent()
            signals.flush()

            // Verify list is [A, B, C, D]
            var list = signals.read(listSignal) as? FluxValue.ListVal
            assertEquals(4, list?.items?.size, "list must have 4 elements after append")
            assertEquals(4, root.children.size, "ForEach must re-expand to 4 rows after append")

            // Now tap "Remove" on row A (the first seeded row)
            // The button's node id is the derived id for row 0's button
            val row0ButtonId = tree.forEachRowContext.keys.first()
            println("FLUXRT TEST: row0ButtonId=$row0ButtonId forEachRowContext=${tree.forEachRowContext}")

            executor.dispatch(HandlerEvent(6u, row0ButtonId))
            dispatcher.scheduler.runCurrent()
            signals.flush()

            // Verify list is [B, C, D] — A was removed
            list = signals.read(listSignal) as? FluxValue.ListVal
            println("FLUXRT TEST: final list=${list?.items}")
            assertEquals(3, list?.items?.size, "list must have 3 elements after remove")
            assertEquals(
                listOf(51u, 52u, 53u),
                list?.items?.map { (it as? FluxValue.StrVal)?.id },
                "remove(A) must leave [B, C, D], not [A, B, C]"
            )
        }
}
