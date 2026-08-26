package dev.flux.host

import dev.flux.host.shadow.ShadowTree
import dev.flux.host.shadow.displayText
import dev.flux.host.shadow.buttonHandlerId
import dev.flux.host.signal.SignalGraph
import dev.flux.host.transport.MockTransport
import dev.flux.host.vm.FluxValue
import dev.flux.host.wire.FrameBuilder
import dev.flux.host.wire.FrameDeserializer
import dev.flux.host.wire.WireValue
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNotNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * FA-RENDER Phase A — the reconciled shadow tree must reach real on-screen
 * views (Compose), not the `Text("Flux host ready")` placeholder.
 *
 * A full Compose UI test requires an emulator/device; the JVM unit gate (per the
 * host's device-independent contract) instead asserts the data the
 * [FluxTreeView] renderer consumes directly from the shadow tree: a real root
 * node is present and its `text` prop carries the resolved string the renderer
 * would display. That proves the placeholder path (`Flux — connecting…` / host
 * ready) is no longer the only output and that [FluxRoot] would bind the tree.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class RenderMountTest {
    private fun stdlibEntries(): List<Pair<UInt, String>> {
        val kinds = listOf("column", "text", "button", "row", "text_field", "screen", "router")
        return (100u..106u).toList().zip(kinds) +
            listOf(200u to "text", 300u to "button", 500u to "screen", 600u to "router")
    }

    /** Builds a counter-shaped Init frame: column(root) → text + button. */
    private fun counterBytes(): ByteArray =
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
                // Root column (id=1, component=100) with text (2) + button (3).
                node(id = 1u, kind = 0x12u, component = 100u, props = emptyList(), childIds = listOf(2u, 3u))
                // Text: text prop = str id 7.
                node(id = 2u, kind = 0x10u, component = 200u, props = listOf(0u.toUShort() to WireValue.StrVal(7u)), childIds = emptyList())
                // Button: text prop = str id 8, onClick handler 5.
                node(
                    id = 3u,
                    kind = 0x11u,
                    component = 300u,
                    props =
                        listOf(
                            0u.toUShort() to WireValue.StrVal(8u),
                            1u.toUShort() to WireValue.HandlerRefVal(5u),
                        ),
                    childIds = emptyList(),
                )
            }.build()

    @Test
    fun `treeReady binds shadow tree with real text and button`() =
        runTest {
            val bytes = counterBytes()
            val frame = FrameDeserializer.deserialize(bytes)
            val tree = ShadowTree(AdapterRegistry.fromStringTable(stdlibEntries().map { (id, k) -> StringTableEntry(id, k) }))
            val executor = FluxExecutor(tree, SignalGraph(), MockTransport(), vmScope = TestScope(StandardTestDispatcher(testScheduler)), reactiveDispatcher = StandardTestDispatcher(testScheduler))
            val root = tree.applyFrame(frame, executor)

            // The renderer would bind exactly this root node (the old code only
            // showed the placeholder).
            assertNotNull(root)
            assertEquals("column", root!!.kind)
            assertEquals(2, root.children.size)

            val text = root.children[0]
            val button = root.children[1]
            assertEquals("text", text.kind)
            assertEquals("button", button.kind)

            // The resolved text prop the Compose renderer projects into a `Text`.
            assertEquals("7", text.displayText(), "renderer must read the shadow tree's resolved text prop")
            // The bound handler the renderer wires to the Button's onClick.
            assertEquals(5u, button.buttonHandlerId(), "renderer must read the button's onClick handler id")
        }
}
