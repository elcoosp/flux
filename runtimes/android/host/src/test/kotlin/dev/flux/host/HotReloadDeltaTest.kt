package dev.flux.host

import dev.flux.host.shadow.ShadowTree
import dev.flux.host.shadow.displayText
import dev.flux.host.signal.SignalGraph
import dev.flux.host.transport.MockTransport
import dev.flux.host.wire.FrameBuilder
import dev.flux.host.wire.FrameDeserializer
import dev.flux.host.wire.WireValue
import dev.flux.ui.PropsIndex
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

/**
 * Regression test for the source-edit hot-reload path (FLUX-019).
 *
 * A text edit shifts every node id (ids derive from byte-accurate source
 * spans), so the dev server ships a Delta with `Remove` patches for the whole
 * old tree followed by `Insert` patches for the new one. Two bugs broke this
 * on Android:
 *
 *  1. Applying the patches sequentially tore the old root down first, so every
 *     `Insert` found its parent already removed and silently no-oped — the UI
 *     never reflected the edit. Fixed by rebuilding from the merged patch index
 *     when the old root is removed and a new root is inserted.
 *
 *  2. A Delta frame carries only its changed strings and an empty
 *     `componentNames`, so feeding `frame.strings` into the adapter registry
 *     overwrote a component-name binding at a colliding id (a literal at id 2
 *     clobbering the "Column" component), surfacing as
 *     `no adapter registered for component 2`. The registry must only gain
 *     entries from `componentNames`, and the string resolver must merge rather
 *     than replace its table.
 *
 * Uses hand-built frames (no live dev server) so the path is exercised
 * deterministically through the real `ShadowTree.applyFrame` code.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class HotReloadDeltaTest {
    private val componentEntries =
        listOf(
            100u to "column",
            200u to "text",
            300u to "button",
        )

    /** Init: column(root 1) -> text(2) + button(3). */
    private fun initBytes(): ByteArray =
        FrameBuilder()
            .flags(fullTree = true)
            .apply {
                stringEntry(7u, "tapped 0 times")
                stringEntry(8u, "Increment")
                for ((cid, name) in componentEntries) componentEntry(cid, name)
                // Root column.
                node(1u, 0x12u, 100u, emptyList(), listOf(2u, 3u))
                // Text: text prop = str id 7.
                node(2u, 0x10u, 200u, listOf(PropsIndex.TEXT_TEXT to WireValue.StrVal(7u)), emptyList())
                // Button: text prop = str id 8.
                node(
                    3u,
                    0x11u,
                    300u,
                    listOf(PropsIndex.BUTTON_TEXT to WireValue.StrVal(8u)),
                    emptyList(),
                )
            }.build()

    /**
     * Hot-reload Delta: remove old root (1) and insert a fresh tree rooted at a
     * new id (10), with the Text changed. Crucially the Delta carries NO
     * component names and a literal string at id 2 — the same id the "Column"
     * component used in the Init — to prove the registry/string fixes hold.
     */
    private fun deltaBytes(): ByteArray =
        FrameBuilder()
            .flags(fullTree = false)
            .apply {
                // Literal string colliding with component id 2 ("Column").
                stringEntry(2u, " times")
                stringEntry(9u, "tapped 99 times")
                patchRemove(1u)
                // New root column at id 10.
                patchInsert(
                    parentId = 0u,
                    index = 0,
                    id = 10u,
                    kind = 0x12u,
                    component = 100u,
                    props = emptyList(),
                    childIds = listOf(20u, 30u),
                )
                // New text at id 20, changed literal.
                patchInsert(
                    parentId = 10u,
                    index = 0,
                    id = 20u,
                    kind = 0x10u,
                    component = 200u,
                    props = listOf(PropsIndex.TEXT_TEXT to WireValue.StrVal(9u)),
                    childIds = emptyList(),
                )
                // New button at id 30.
                patchInsert(
                    parentId = 10u,
                    index = 1,
                    id = 30u,
                    kind = 0x11u,
                    component = 300u,
                    props = listOf(PropsIndex.BUTTON_TEXT to WireValue.StrVal(8u)),
                    childIds = emptyList(),
                )
            }.build()

    private fun findText(n: dev.flux.host.shadow.ShadowNode?): dev.flux.host.shadow.ShadowNode? {
        if (n == null) return null
        if (n.displayText() != null) return n
        for (c in n.children) findText(c)?.let { return it }
        return null
    }

    @Test
    fun `source edit delta rebuilds the tree and keeps component adapters`() =
        runTest {
            val dispatcher = StandardTestDispatcher(testScheduler)
            val scope = TestScope(dispatcher)
            val signals = SignalGraph()
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

            executor.receiveFrame(initBytes())
            dispatcher.scheduler.runCurrent()
            val rootBefore = tree.rootNode ?: error("no root after init frame")
            assertEquals("column", rootBefore.kind)
            assertEquals(
                "tapped 0 times",
                findText(rootBefore)?.displayText(),
                "init text",
            )

            // Hot-reload: remove old root + insert new tree (changed text).
            executor.receiveFrame(deltaBytes())
            dispatcher.scheduler.runCurrent()

            val rootAfter =
                tree.rootNode
                    ?: error("root missing after hot-reload delta — tree was not rebuilt")
            assertEquals("column", rootAfter.kind, "root must still resolve the Column adapter")
            assertEquals(
                "tapped 99 times",
                findText(rootAfter)?.displayText(),
                "hot-reload text must reflect the edited source",
            )
        }
}
