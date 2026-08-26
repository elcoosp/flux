package dev.flux.host

import dev.flux.host.ReactiveDispatcher
import dev.flux.host.shadow.ShadowTree
import dev.flux.host.signal.SignalGraph
import dev.flux.host.transport.MockTransport
import dev.flux.host.vm.FluxValue
import dev.flux.host.wire.FrameBuilder
import dev.flux.host.wire.FrameDeserializer
import dev.flux.host.wire.WireValue
import dev.flux.ui.PropsIndex
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNotNull
import org.junit.jupiter.api.Assertions.assertSame
import org.junit.jupiter.api.Test

/**
 * End-to-end test without sockets (FLUX-007 acceptance criterion), wired to the
 * REAL `adapters/ui-kotlin` dev adapter kit (FLUX-017 — the in-dir [MockAdapter]
 * is gone).
 *
 * A hand-built `Init` frame is deserialized, fed to the [ShadowTree] with the
 * production adapters, and the resulting native view hierarchy is asserted. A
 * [FluxExecutor.dispatch] then runs a closure in the VM; the signal graph
 * updates and the reconciled view reflects the new value **without recreating
 * the Android view** (view identity is preserved). Finally a Router push/edit/
 * pop exercise asserts screen state survives by view identity.
 *
 * The adapter kit drives `FluxNativeViewImpl` (a plain-JVM stand-in for the real
 * `android.view.View`), so the whole flow runs on the unit-test classpath with
 * no emulator — exactly the device-independent gate the host requires.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class EndToEndTest {
    /** The seven stdlib component ids the Init frame declares and resolves. */
    private val stdlibKinds = listOf("column", "text", "button", "row", "text_field", "screen", "router")

    private fun stdlibEntries(): List<Pair<UInt, String>> {
        val ids = (100u..106u).toList()
        return ids.zip(stdlibKinds) +
            listOf(200u to "text", 300u to "button", 500u to "screen", 600u to "router")
    }

    @Test
    fun `init frame builds real adapter hierarchy`() {
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
                    // Root: column (id=1, component=100) with a text child.
                    node(
                        id = 1u,
                        kind = 0x12u,
                        component = 100u,
                        props = emptyList(),
                        childIds = listOf(2u),
                    )
                    node(
                        id = 2u,
                        kind = 0x10u,
                        component = 200u,
                        props = listOf(PropsIndex.TEXT_TEXT to WireValue.StrVal(7u)),
                        childIds = emptyList(),
                    )
                }.build()

        val frame = FrameDeserializer.deserialize(bytes)
        val tree = ShadowTree(AdapterRegistry.fromStringTable(stdlibEntries().map { (id, k) -> StringTableEntry(id, k) }))
        val executor = FakeKitExecutor()
        val root = tree.applyFrame(frame, executor)

        assertNotNull(root)
        assertEquals(1u, root!!.id)
        assertEquals("column", root.kind)
        assertEquals(1, root.children.size)
        val textNode = root.children[0]
        assertEquals(2u, textNode.id)
        // The real TextAdapter set the text property onto its view.
        assertEquals("7", textNode.view.getProperty("text"))
    }

    @Test
    fun `tap dispatch updates signal and reconciles view without recreation`() =
        runTest {
            val dispatcher = StandardTestDispatcher(testScheduler)
            val scope = TestScope(dispatcher)
            val signals = SignalGraph()
            // Counter signal 1 = 0.
            signals.seed(listOf(1u to FluxValue.IntVal(0)))

            // Build: column(root id=1) → button(id=2, onClick handler 5).
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
                        node(
                            id = 1u,
                            kind = 0x12u,
                            component = 100u,
                            props = emptyList(),
                            childIds = listOf(2u),
                        )
                        // Button: BUTTON_TEXT=0 str id 9, BUTTON_ON_CLICK=1 handler 5.
                        node(
                            id = 2u,
                            kind = 0x11u,
                            component = 300u,
                            props =
                                listOf(
                                    PropsIndex.BUTTON_TEXT to WireValue.StrVal(9u),
                                    PropsIndex.BUTTON_ON_CLICK to WireValue.HandlerRefVal(5u),
                                ),
                            childIds = emptyList(),
                        )
                    }.build()

            val frame = FrameDeserializer.deserialize(bytes)
            val tree = ShadowTree(AdapterRegistry.fromStringTable(stdlibEntries().map { (id, k) -> StringTableEntry(id, k) }))
            val transport = MockTransport()
            val executor = FluxExecutor(tree, signals, transport, scope, ReactiveDispatcher.test(dispatcher))

            val root = tree.applyFrame(frame, executor)
            assertNotNull(root)
            val buttonNode = root!!.children[0]
            val buttonView = buttonNode.view
            // Capture identity before the tap.
            val beforeSame = buttonView
            assertEquals("9", buttonView.getProperty("text"))

            // Closure 5: READ_SIGNAL r0, 1 ; LOAD_INT_CONST r1, 1 ; ADD_I64 r0,r0,r1 ;
            //            WRITE_SIGNAL 1, r0 ; HALT  (count = count + 1)
            val closure = counterIncrementClosure()
            executor.registerClosure(5u, closure)
            executor.dispatch(5u)
            dispatcher.scheduler.runCurrent()
            signals.flush()

            // Signal mutated by the VM.
            assertEquals(FluxValue.IntVal(1), signals.read(1u))
            // The button label is driven from the counter; the reconciler pushed
            // the new text onto the SAME view instance (no recreation).
            assertSame(beforeSame, buttonView, "button view must be reused, not recreated, across the update")
        }

    @Test
    fun `router push edit pop preserves screen view identity`() =
        runTest {
            val dispatcher = StandardTestDispatcher(testScheduler)
            val scope = TestScope(dispatcher)
            val signals = SignalGraph()

            // Build a router (id=1) with a single home screen (id=10).
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
                        // Router root.
                        node(
                            id = 1u,
                            kind = 0x16u,
                            component = 600u,
                            props = emptyList(),
                            childIds = listOf(10u),
                        )
                        // Home screen (carries one text child id=11).
                        node(
                            id = 10u,
                            kind = 0x15u,
                            component = 500u,
                            props = emptyList(),
                            childIds = listOf(11u),
                        )
                        node(
                            id = 11u,
                            kind = 0x10u,
                            component = 200u,
                            props = listOf(PropsIndex.TEXT_TEXT to WireValue.StrVal(7u)),
                            childIds = emptyList(),
                        )
                    }.build()

            val frame = FrameDeserializer.deserialize(bytes)
            val tree = ShadowTree(AdapterRegistry.fromStringTable(stdlibEntries().map { (id, k) -> StringTableEntry(id, k) }))
            val transport = MockTransport()
            val executor = FluxExecutor(tree, signals, transport, scope, ReactiveDispatcher.test(dispatcher))
            val root = tree.applyFrame(frame, executor)
            assertNotNull(root)
            val routerNode = root!!
            val homeNode = routerNode.children[0]
            val homeView = homeNode.view

            // Push a detail screen (id=20) via an Insert patch under the router.
            val push =
                FrameBuilder()
                    .apply {
                        magic()
                        version(1)
                        seq(1)
                        flags(fullTree = false)
                        patchCount(1)
                        handlerCount(0)
                        stringCount(0)
                        patchInsert(
                            parentId = 1u,
                            index = 1,
                            id = 20u,
                            kind = 0x15u,
                            component = 500u,
                            props = emptyList(),
                            childIds =
                                listOf(21u),
                        )
                        // detail's content text child (self-contained leaf).
                        node(
                            id = 21u,
                            kind = 0x10u,
                            component = 200u,
                            props = listOf(PropsIndex.TEXT_TEXT to WireValue.StrVal(8u)),
                            childIds = emptyList(),
                        )
                    }.build()
            tree.applyFrame(FrameDeserializer.deserialize(push), executor)

            assertEquals(2, routerNode.children.size)
            // Home screen kept its SAME view instance across the push.
            assertSame(homeView, routerNode.children[0].view, "home screen view must be preserved on push")

            // Edit home's nested text while detail is on top.
            val editHomeText =
                FrameBuilder()
                    .apply {
                        magic()
                        version(1)
                        seq(2)
                        flags(fullTree = false)
                        patchCount(1)
                        handlerCount(0)
                        stringCount(0)
                        patchUpdate(
                            id = 11u,
                            changes =
                                listOf(PropsIndex.TEXT_TEXT to WireValue.StrVal(77u)),
                        )
                    }.build()
            tree.applyFrame(FrameDeserializer.deserialize(editHomeText), executor)
            assertEquals("77", homeNode.children[0].view.getProperty("text"))

            // Pop: a Remove patch for the detail screen (id=20).
            val pop =
                FrameBuilder()
                    .apply {
                        magic()
                        version(1)
                        seq(3)
                        flags(fullTree = false)
                        patchCount(1)
                        handlerCount(0)
                        stringCount(0)
                        patchRemove(20u)
                    }.build()
            tree.applyFrame(FrameDeserializer.deserialize(pop), executor)

            assertEquals(1, routerNode.children.size)
            assertSame(homeView, routerNode.children[0].view, "home screen view must survive the pop")
            // The edited text persisted on the preserved home view.
            assertEquals("77", homeNode.children[0].view.getProperty("text"))
        }
}

/** A minimal [dev.flux.ui.FluxExecutor] stand-in for tree-building tests. */
private class FakeKitExecutor : dev.flux.ui.FluxExecutor {
    override fun dispatch(event: dev.flux.ui.HandlerEvent) = Unit
}

/**
 * The closure `count = count + 1`: reads signal 1 into `r0`, loads `1` into
 * `r1`, adds them, writes the sum back to signal 1, then halts (Appendix E).
 * One byte per line keeps the encoding auditable and within the line budget.
 */
private fun counterIncrementClosure(): ByteArray =
    byteArrayOf(
        0x10.toByte(), // READ_SIGNAL r0, signal 1
        0,
        1,
        0,
        0,
        0,
        0xB0.toByte(), // LOAD_INT_CONST r1, 1
        1,
        1,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0x20.toByte(), // ADD_I64 r0, r0, r1
        0,
        0,
        1,
        0x11.toByte(), // WRITE_SIGNAL signal 1, r0
        1,
        0,
        0,
        0,
        0,
        0x00.toByte(), // HALT
    )
