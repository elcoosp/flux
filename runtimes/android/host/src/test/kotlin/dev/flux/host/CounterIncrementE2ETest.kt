package dev.flux.host

import dev.flux.host.shadow.ShadowTree
import dev.flux.host.shadow.buttonHandlerId
import dev.flux.host.shadow.displayText
import dev.flux.host.signal.SignalGraph
import dev.flux.host.transport.MockTransport
import dev.flux.host.wire.FrameDeserializer
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Test

/**
 * End-to-end verification of the counter increment using the REAL wire frame
 * captured from the dev server. Proves the full chain — handler registration,
 * tap dispatch into the VM, signal write, dirty reconcile, and re-materialised
 * Text label — works in the actual code path (not a synthetic frame).
 */
@OptIn(ExperimentalCoroutinesApi::class)
class CounterIncrementE2ETest {
    private fun frameBytes(): ByteArray =
        CounterIncrementE2ETest::class.java
            .classLoader
            .getResourceAsStream("counter_init_frame.bin")!!
            .readBytes()

    @Test
    fun `tapping the button increments the counter label`() =
        runTest {
            val dispatcher = StandardTestDispatcher(testScheduler)
            val scope = TestScope(dispatcher)
            val signals = SignalGraph()
            val bytes = frameBytes()
            val frame = FrameDeserializer.deserialize(bytes)
            val tree = ShadowTree(AdapterRegistry.fromStringTable(emptyList()))
            val transport = MockTransport()
            val executor =
                FluxExecutor(
                    tree,
                    signals,
                    transport,
                    vmScope = scope,
                    reactiveDispatcher = ReactiveDispatcher.test(dispatcher),
                )
            // Apply the init frame (registers handlers + builds tree).
            executor.receiveFrame(bytes)
            dispatcher.scheduler.runCurrent()

            val root = tree.rootNode ?: error("no root after init frame")

            // Inspect the whole tree's kinds to diagnose.
            fun dump(
                n: dev.flux.host.shadow.ShadowNode?,
                d: Int = 0,
            ) {
                if (n == null) return
                println("E2E node kind=${n.kind} id=${n.id} text=${n.displayText()} handler=${n.buttonHandlerId()}")
                for (c in n.children) dump(c, d + 1)
            }
            dump(root)

            // Find the button node (recursively; it is a grandchild of root).
            fun findButton(n: dev.flux.host.shadow.ShadowNode?): dev.flux.host.shadow.ShadowNode? {
                if (n == null) return null
                if (n.kind == "button") return n
                for (c in n.children) findButton(c)?.let { return it }
                return null
            }
            val button = findButton(root) ?: error("no button node")
            val handlerId = button.buttonHandlerId()
            println("E2E button handlerId=$handlerId")

            fun findText(n: dev.flux.host.shadow.ShadowNode?): dev.flux.host.shadow.ShadowNode? {
                if (n == null) return null
                if (n.displayText() != null) return n
                for (c in n.children) findText(c)?.let { return it }
                return null
            }
            val textNode = findText(root) ?: error("no text node")
            val textBefore = textNode.displayText()
            println("E2E text before=$textBefore")

            // Simulate the tap.
            executor.dispatch(dev.flux.ui.HandlerEvent(handlerId))
            dispatcher.scheduler.runCurrent()
            signals.flush()

            val textAfter = textNode.displayText()
            println("E2E text after=$textAfter")
            checkNotNull(textAfter) { "text node lost its label after dispatch" }
            assert(textAfter.startsWith("tapped 1 times")) { "label must reflect count after one tap, got '$textAfter'" }
        }
}
