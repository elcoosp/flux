package dev.flux.host.shadow

import dev.flux.host.AdapterRegistry
import dev.flux.host.StringTableEntry
import dev.flux.host.FluxExecutor
import dev.flux.host.ReactiveDispatcher
import dev.flux.host.signal.SignalGraph
import dev.flux.host.transport.MockTransport
import dev.flux.host.wire.FluxFrame
import dev.flux.host.wire.Patch
import dev.flux.host.wire.StringEntry
import dev.flux.host.wire.WireChild
import dev.flux.host.wire.WireNode
import dev.flux.host.wire.WireValue
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * Regression guard for the `wireIndex` staleness bug (round 6).
 *
 * `ShadowTree.wireIndex` is the wire-node mirror `reconcileForEach` reads for
 * template and descendant lookups. Before this fix it was assigned only on
 * `frame.fullTree == true`, so any Delta that replaced the tree left it
 * pointing at the *previous* frame's node ids. A subsequent dispatch that
 * re-expanded a `ForEach` would build rows from stale wire nodes — a
 * hot-reload blank-list on Android. This test drives a tree-replacing delta
 * storm and asserts `wireIndex` tracks the current node set.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class WireIndexStalenessTest {

    private fun leaf(
        id: UInt,
        componentId: UInt,
        props: List<Pair<UShort, WireValue>> = emptyList(),
    ) = WireNode(
        id = id,
        kind = "1",
        componentId = componentId,
        props = props,
        children = emptyList(),
        handlerIds = emptyList(),
        spanFile = 0u,
        spanStart = 0u,
        spanEnd = 0u,
    )

    private fun root(id: UInt, childIds: List<UInt>) = WireNode(
        id = id,
        kind = "1",
        componentId = 2u, // Column
        props = emptyList(),
        children = childIds.map { WireChild.Node(it) },
        handlerIds = emptyList(),
        spanFile = 0u,
        spanStart = 0u,
        spanEnd = 0u,
    )

    private fun initFrame(rootId: UInt): FluxFrame {
        val textId = rootId + 1u
        val buttonId = rootId + 2u
        val text = leaf(textId, 0u, listOf(0u.toUShort() to WireValue.StrVal(7u)))
        val button = leaf(buttonId, 1u, listOf(0u.toUShort() to WireValue.StrVal(8u)))
        val column = root(rootId, listOf(textId, buttonId))
        return FluxFrame(
            version = 1u,
            seq = 0u,
            fullTree = true,
            patches = emptyList(),
            root = column,
            strings = listOf(
                StringEntry(7u, "tapped 0 times"),
                StringEntry(8u, "Increment"),
            ),
            componentNames = listOf(
                StringEntry(0u, "Text"),
                StringEntry(1u, "Button"),
                StringEntry(2u, "Column"),
            ),
            stateDelta = emptyList(),
            handlers = emptyList(),
            bytecodeBlob = null,
            extraNodes = listOf(text, button),
        )
    }

    /// The shape the *differ* emits for a source-span-shifted edit: every old
    /// node id is `Remove`d and every new node id is `Insert`ed. The host
    /// detects this whole-tree pattern (`oldRootRemoved && newRootInserted`)
    /// and rebuilds from the merged index in one pass. Emitting `.replace` with
    /// a *different* id — as a hand-rolled test might — is NOT the differ's
    /// output and exercises a shape the host does not see in production.
    private fun removeInsertDeltaFrame(
        oldRootId: UInt,
        newRootId: UInt,
        seq: UInt,
    ): FluxFrame {
        val textId = newRootId + 1u
        val buttonId = newRootId + 2u
        val text = leaf(textId, 0u, listOf(0u.toUShort() to WireValue.StrVal(7u)))
        val button = leaf(buttonId, 1u, listOf(0u.toUShort() to WireValue.StrVal(8u)))
        val column = root(newRootId, listOf(textId, buttonId))
        return FluxFrame(
            version = 1u,
            seq = seq,
            fullTree = false,
            patches = listOf(
                // Old tree torn down first (differ emits Remove for every id
                // that no longer exists in the new tree).
                Patch(
                    tag = 0x04u, id = oldRootId, parentId = 0u, index = 0u,
                    node = null, diff = null, keyCount = 0u, keys = emptyList(),
                    closure = null,
                ),
                Patch(
                    tag = 0x04u, id = oldRootId + 1u, parentId = 0u, index = 0u,
                    node = null, diff = null, keyCount = 0u, keys = emptyList(),
                    closure = null,
                ),
                Patch(
                    tag = 0x04u, id = oldRootId + 2u, parentId = 0u, index = 0u,
                    node = null, diff = null, keyCount = 0u, keys = emptyList(),
                    closure = null,
                ),
                // New tree inserted top-down. The root's parent is the synthetic
                // wrapper (id 0 in the frame convention).
                Patch(
                    tag = 0x03u, id = 0u, parentId = 0u, index = 0u,
                    node = column, diff = null, keyCount = 0u, keys = emptyList(),
                    closure = null,
                ),
                Patch(
                    tag = 0x03u, id = newRootId, parentId = newRootId, index = 0u,
                    node = text, diff = null, keyCount = 0u, keys = emptyList(),
                    closure = null,
                ),
                Patch(
                    tag = 0x03u, id = newRootId, parentId = newRootId, index = 1u,
                    node = button, diff = null, keyCount = 0u, keys = emptyList(),
                    closure = null,
                ),
            ),
            root = null,
            strings = emptyList(),
            componentNames = emptyList(),
            stateDelta = emptyList(),
            handlers = emptyList(),
            bytecodeBlob = null,
            extraNodes = emptyList(),
        )
    }

    @Test
    fun `wireIndex tracks current node set across a replace storm`() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        val scope = TestScope(dispatcher)
        val signals = SignalGraph()

        val registry = AdapterRegistry.fromStringTable(
            listOf(
                StringTableEntry(0u, "Text"),
                StringTableEntry(1u, "Button"),
                StringTableEntry(2u, "Column"),
            ),
        )
        val tree = ShadowTree(registry)
        val executor = FluxExecutor(
            shadowTree = tree,
            signals = signals,
            transport = MockTransport(),
            vmScope = scope,
            reactiveDispatcher = ReactiveDispatcher.test(dispatcher),
        )

        var rootId = 100u
        tree.applyFrame(initFrame(rootId), executor)

        for (i in 1..30) {
            val nextId = 100u + (i.toUInt() * 10u)
            tree.applyFrame(removeInsertDeltaFrame(rootId, nextId, i.toUInt()), executor)
            rootId = nextId
        }

        val index = tree.debugWireIndex()
        // `wireIndex` must be bounded by the current tree (3 nodes) and must
        // contain the current root id — not the initial frame's ids.
        assertTrue(
            index.containsKey(rootId),
            "wireIndex must contain the current root id $rootId; got keys ${index.keys.sorted()}",
        )
        assertTrue(
            index.size <= 10,
            "wireIndex must be bounded by the current tree, got ${index.size} entries",
        )
    }
}
