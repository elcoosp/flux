package dev.flux.host

import java.util.Locale
import dev.flux.host.ReactiveDispatcher
import dev.flux.host.shadow.ShadowTree
import dev.flux.host.shadow.reconcileDirty
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
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * FLUX-066 on-device render-perf harness — Android host side.
 *
 * Builds a fixed warm fixture tree (column root → N Text leaves), each leaf
 * subscribing to a distinct signal via the ADR-0027 signal-metadata section, then
 * drives the REAL [ShadowTree.reconcileDirty] (the in-place prop-observation
 * path from AGENTS.md §3.10) and times it. The observed latencies are emitted as
 * a `MetricRecord`-shaped JSON document (the same schema `flux-perf-harness`
 * consumes) and the §3.10 `NodeMutation` budget (p95 ≤ 3 ms) is asserted.
 *
 * This is a genuine measurement of the production reconciler on the JVM host
 * (no emulator needed for the reactive core), closing the "demonstration, not a
 * measurement" gap left by PRD-J's `ci_run` example. The numbers are what the
 * §3.10 `< 3 ms` budget is verified against.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class RenderPerfHarnessTest {
    private val stdlibEntries: List<Pair<UInt, String>> =
        (100u..106u).toList().zip(
            listOf("column", "text", "button", "row", "textinput", "screen", "router"),
        ) + listOf(200u to "text", 300u to "button", 500u to "screen", 600u to "router")

    /** Builds a counter-shaped tree: root column → [leafCount] Text leaves. */
    private fun fixtureBytes(leafCount: Int): ByteArray {
        val leafSignalBase = 1_000u
        val b = FrameBuilder()
        b.magic()
        b.version(1)
        b.seq(0)
        b.flags(fullTree = true)
        b.stringCount(stdlibEntries.size)
        for ((id, kind) in stdlibEntries) b.stringEntry(id, kind)
        val root = 1u
        val childIds = (0u until leafCount.toUInt()).map { 10u + it }
        // Root column.
        b.node(id = root, kind = 0x12u, component = 100u, props = emptyList(), childIds = childIds)
        // One Text leaf per child; each subscribes to a distinct signal so a write
        // to that signal marks exactly its leaf dirty (R1 — `reconcileDirty`
        // touches only `dependents[S]`).
        for ((i, id) in childIds.withIndex()) {
            val sig = leafSignalBase + i.toUInt()
            b.node(
                id = id,
                kind = 0x10u,
                component = 200u,
                props = listOf(PropsIndex.TEXT_TEXT to WireValue.StrVal(7u)),
                childIds = emptyList(),
            )
            b.signalMetaEntry(id, listOf(sig))
        }
        return b.build()
    }

    @Test
    fun `reconcileDirty node-mutation latency stays within the 3ms budget`() =
        runTest {
            val leafCount = 50
            val dispatcher = StandardTestDispatcher(testScheduler)
            val scope = TestScope(dispatcher)
            val tree = ShadowTree(AdapterRegistry.fromStringTable(stdlibEntries.map { (id, k) -> StringTableEntry(id, k) }))
            val executor =
                FluxExecutor(
                    tree,
                    SignalGraph(),
                    MockTransport(),
                    vmScope = scope,
                    reactiveDispatcher = ReactiveDispatcher.test(dispatcher),
                )
            executor.onError = { throw AssertionError("executor error: $it") }
            executor.receiveFrame(fixtureBytes(leafCount))
            dispatcher.scheduler.runCurrent()

            val rootId = tree.rootNode?.id ?: error("root not built")
            val leafSignalBase = 1_000u

            // Warm up the JIT / snapshot the tree before timing.
            for (i in 0 until leafCount) {
                val sig = leafSignalBase + i.toUInt()
                executor.materializationSignals.write(sig, FluxValue.IntVal(i.toLong()))
                executor.materializationSignals.flush()
                tree.reconcileDirty(rootId, setOf(sig))
            }

            val iterations = 200
            val samples = ArrayList<Double>(iterations)
            repeat(iterations) { i ->
                val sig = leafSignalBase + (i % leafCount).toUInt()
                executor.materializationSignals.write(sig, FluxValue.IntVal(i.toLong()))
                executor.materializationSignals.flush()
                val start = System.nanoTime()
                tree.reconcileDirty(rootId, setOf(sig))
                val elapsedMs = (System.nanoTime() - start).toDouble() / 1_000_000.0
                samples.add(elapsedMs)
            }

            val sorted = samples.sorted()
            val p50 = sorted[sorted.size / 2]
            val p95 = sorted[(0.95 * sorted.size).toInt().coerceAtMost(sorted.size - 1)]
            val mean = samples.average()

            // Emit a MetricRecord-shaped JSON (scenario=android-declarative-dev,
            // kind=node-mutation) so the Rust harness can gate it in CI.
            val json =
                buildString {
                    append("{")
                    append("\"scenario\":\"android-declarative-dev\",")
                    append("\"kind\":\"node-mutation\",")
                    append("\"tree_size\":$leafCount,")
                    append("\"samples\":[")
                    append(samples.joinToString(",") { "{\"latency\":${String.format(Locale.US, "%.4f", it)}}" })
                    append("]")
                    append("}")
                }
            println("RENDER_PERF android node-mutation: p50=${String.format(Locale.US, "%.3f", p50)}ms p95=${String.format(Locale.US, "%.3f", p95)}ms mean=${String.format(Locale.US, "%.3f", mean)}ms samples=$iterations json=$json")

            // §3.10 headline budget: node-mutation p95 ≤ 3 ms (measured on the
            // JVM host reconciler; the same path the Compose renderer observes).
            assertTrue(p95 <= 3.0, "node-mutation p95 ${String.format(Locale.US, "%.3f", p95)}ms exceeded §3.10 3ms ceiling")
            assertTrue(p50 > 0.0, "node-mutation latency must be a real, positive measurement")
        }
}
