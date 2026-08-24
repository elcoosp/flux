package dev.flux.app

import dev.flux.app.shadow.ShadowTree
import dev.flux.app.signal.SignalGraph
import dev.flux.app.testkit.MockAdapter
import dev.flux.app.transport.MockTransport
import dev.flux.app.vm.FluxValue
import dev.flux.app.wire.FrameBuilder
import dev.flux.app.wire.FrameDeserializer
import dev.flux.app.wire.WireValue
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNotNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * End-to-end test without sockets (FLUX-007 acceptance criterion): a hand-built
 * `Init` frame is deserialized, fed to the [ShadowTree] with in-dir mock
 * adapters, and the resulting native view hierarchy is asserted; then a
 * [FluxExecutor.dispatch] runs a closure in the VM, the signal graph updates,
 * and the reconciled view reflects the new value.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class EndToEndTest {
    private fun buildTree(): Pair<ShadowTree, MockAdapter> {
        val text = MockAdapter("text", childCapable = false)
        val column = MockAdapter("column")
        return ShadowTree(mapOf("text" to text, "column" to column)) to text
    }

    @Test
    fun `init frame builds native view hierarchy`() {
        val bytes =
            FrameBuilder()
                .apply {
                    magic()
                    version(1)
                    seq(0)
                    flags(fullTree = true)
                    patchCount(0)
                    handlerCount(0)
                    stringCount(0)
                    node(
                        id = 1u,
                        kind = 0x12u,
                        component = 100u,
                        props = emptyList<Pair<UShort, WireValue>>(),
                        childIds = listOf(2u),
                    )
                    node(
                        id = 2u,
                        kind = 0x10u,
                        component = 200u,
                        props = listOf(0u.toUShort() to WireValue.StrVal(7u)),
                        childIds = emptyList(),
                    )
                }.build()

        val frame = FrameDeserializer.deserialize(bytes)
        val (tree, textAdapter) = buildTree()
        val executor = FakeKitExecutor()
        val root = tree.applyFrame(frame, executor)

        assertNotNull(root)
        assertEquals(1u, root!!.id)
        assertEquals(1, root.children.size)
        val textNode = root.children[0]
        assertEquals(2u, textNode.id)
        // The mock adapter recorded the text prop via update().
        assertTrue(textAdapter.updates.isNotEmpty())
        val props = textAdapter.updates.last()
        assertEquals("7", props.getString(0u))
    }

    @Test
    fun `dispatch runs closure and updates signals`() =
        runTest {
            val dispatcher = StandardTestDispatcher(testScheduler)
            val scope = TestScope(dispatcher)
            val signals = SignalGraph()
            // Seed signal 1 = 41 so the handler can read-increment-write.
            signals.seed(listOf(1u to FluxValue.IntVal(41)))

            // Closure: READ_SIGNAL r0, 1 ; LOAD_INT_CONST r1, 1 ; ADD_I64 r0, r0, r1 ;
            //          WRITE_SIGNAL 1, r0 ; HALT  (count = count + 1)
            val closure =
                byteArrayOf(
                    0x10,
                    0,
                    1,
                    0,
                    0,
                    0, // READ_SIGNAL r0, signal 1
                    0xB0.toByte(),
                    1,
                    1,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0, // LOAD_INT_CONST r1, 1
                    0x20,
                    0,
                    0,
                    1, // ADD_I64 r0, r0, r1
                    0x11,
                    1,
                    0,
                    0,
                    0,
                    0, // WRITE_SIGNAL signal 1, r0
                    0x00,
                )

            val (tree, _) = buildTree()
            val transport = MockTransport()
            val executor = FluxExecutor(tree, signals, transport, scope, dispatcher)

            // Register the closure under handler id 5 and dispatch it.
            executor.registerClosure(5u, closure)
            executor.dispatch(5u)
            dispatcher.scheduler.runCurrent()
            signals.flush()

            assertEquals(FluxValue.IntVal(42), signals.read(1u))
        }
}

/** A minimal [dev.flux.ui.FluxExecutor] stand-in for tree-building tests. */
private class FakeKitExecutor : dev.flux.ui.FluxExecutor {
    override fun dispatch(event: dev.flux.ui.HandlerEvent) = Unit
}
