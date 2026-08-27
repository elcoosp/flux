package dev.flux.host

import dev.flux.host.shadow.ShadowTree
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
 * Reproduces the user's hot-reload interpolation regression: after a source
 * edit, the `count` in the Text is replaced by `{...}` (the raw template
 * literal) instead of the interpolated value. This applies the REAL Delta
 * frame the dev server ships for a text edit (captured from `flux-devserver`
 * via `delta_probe`) on top of the real Init frame, end-to-end through
 * [ShadowTree.applyFrame]. If the delta's thunk/string tables are not applied
 * correctly, the Text falls back to the template and shows `{...}`.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class HotReloadInterpolationTest {
    private fun initBytes(): ByteArray =
        HotReloadInterpolationTest::class.java
            .classLoader
            .getResourceAsStream("counter_init_frame.bin")!!
            .readBytes()

    private fun deltaBytes(): ByteArray =
        HotReloadInterpolationTest::class.java
            .classLoader
            .getResourceAsStream("counter_delta_interp.bin")!!
            .readBytes()

    private fun findText(n: dev.flux.host.shadow.ShadowNode?): dev.flux.host.shadow.ShadowNode {
        if (n == null) error("tree has no text node")
        if (n.displayText() != null) return n
        for (c in n.children) {
            val found = findText(c)
            if (found.displayText() != null) return found
        }
        error("tree has no text node")
    }

    @Test
    fun `source edit delta keeps interpolation`() =
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
            val initText = findText(tree.rootNode).displayText()
            check(initText != null && initText.contains("tapped") && initText.contains("times")) {
                "init interpolation expected 'tapped ... times', got '$initText'"
            }

            // Hot-reload: real text-edit delta (Remove+Insert whole tree).
            executor.receiveFrame(deltaBytes())
            dispatcher.scheduler.runCurrent()

            val afterText = findText(tree.rootNode).displayText()
            check(afterText != null && afterText.contains("tapped") && afterText.contains("times") && !afterText.contains("{")) {
                "edit must keep interpolation — got '$afterText' (raw template means thunk fell back)"
            }
        }
}
