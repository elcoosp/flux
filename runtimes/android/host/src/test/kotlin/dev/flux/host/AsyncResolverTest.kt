package dev.flux.host

import dev.flux.host.ReactiveDispatcher
import dev.flux.host.shadow.ShadowTree
import dev.flux.host.signal.SignalGraph
import dev.flux.host.transport.MockTransport
import dev.flux.host.AsyncResolver
import dev.flux.host.DelayAsyncResolver
import dev.flux.host.PassthroughAsyncResolver
import dev.flux.host.CapabilityAsyncResolver
import dev.flux.host.vm.FluxValue
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * First-class async (ADR-0044) + unified capability bridge (ADR-0045): the
 * executor's [AsyncResolver] settles a `Pending` result cell and resumes the
 * parked handler. Drives [FluxExecutor.dispatchAsync] with a real async resolver to
 * prove:
 *   1. a `Pending` cell genuinely parks the handler until the future settles;
 *   2. the resolver's settled value is what the handler resumes with in `r0`;
 *   3. swapping [PassthroughAsyncResolver] for a real resolver changes the value.
 *
 * The capability under test is the oracle's reference async stub (cap 2, method
 * 99), which allocates a fresh `Pending` cell and returns its id (ADR-0045).
 *
 * `dispatchAsync` runs confined to [ReactiveDispatcher.dispatcher] (mirroring how
 * production `dispatch` launches it on `reactiveScope`), so each test `launch`es it
 * on that dispatcher and drives virtual time with `advanceTimeBy`.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class AsyncResolverTest {
    // CALL_CAP r2, cap=2, method=99, args=r0 (9 bytes) · AWAIT r0, r2 (3) ·
    // WRITE_SIGNAL 2, r0 (6) · HALT (1).
    private val asyncHandler =
        byteArrayOf(
            0x90.toByte(), 0x02, 0x02, 0x00, 0x00, 0x00, 0x63, 0x00, 0x00, // CALL_CAP r2, (2,99)
            0xE0.toByte(), 0x00, 0x02, // AWAIT r0, r2
            0x11.toByte(), 0x02, 0x00, 0x00, 0x00, 0x00, // WRITE_SIGNAL 2, r0
            0x00, // HALT
        )

    private val stdlibEntries =
        (100u..106u).zip(listOf("column", "text", "button", "row", "text_field", "screen", "router")) +
            listOf(200u to "text", 300u to "button", 500u to "screen", 600u to "router")

    private fun executor(
        resolver: AsyncResolver,
        signals: SignalGraph,
        dispatcher: ReactiveDispatcher,
    ): FluxExecutor {
        val tree = ShadowTree(AdapterRegistry.fromStringTable(stdlibEntries.map { (id, k) -> StringTableEntry(id, k) }))
        val transport = MockTransport()
        val executor =
            FluxExecutor(
                tree,
                signals,
                transport,
                vmScope = kotlinx.coroutines.test.TestScope(dispatcher.dispatcher),
                reactiveDispatcher = dispatcher,
            )
        executor.asyncResolver = resolver
        executor.registerClosure(1u, asyncHandler)
        return executor
    }

    @Test
    fun `passthrough resolver echoes the fresh async cell id`() =
        runTest {
            val dispatcher = ReactiveDispatcher.test(StandardTestDispatcher(testScheduler))
            val signals = SignalGraph()
            val executor = executor(PassthroughAsyncResolver, signals, dispatcher)
            launch(dispatcher.dispatcher) { executor.dispatchAsync(1u, FluxValue.NullVal) }
            runCurrent()
            val written = signals.read(2u)
            assertTrue(written is FluxValue.IntVal && written.value >= 1_000_000L, "Passthrough echoes the fresh async cell id into signal 2")
        }

    @Test
    fun `resolver parks the handler until the future settles`() =
        runTest {
            var settledAtMs = -1L
            val resolver =
                DelayAsyncResolver(delayMillis = 50) { ms ->
                    delay(ms)
                    settledAtMs = testScheduler.currentTime
                }
            val dispatcher = ReactiveDispatcher.test(StandardTestDispatcher(testScheduler))
            val signals = SignalGraph()
            val executor = executor(resolver, signals, dispatcher)
            launch(dispatcher.dispatcher) { executor.dispatchAsync(1u, FluxValue.NullVal) }
            runCurrent()
            // At virtual time 0 the handler must still be parked: signal 2 is unbound.
            assertEquals(null, signals.read(2u), "handler must remain parked before the future settles")
            // Let the future elapse; the handler resumes and writes the settled value.
            testScheduler.advanceTimeBy(100)
            runCurrent()
            assertEquals(FluxValue.NullVal, signals.read(2u), "after the future settles, signal 2 is the resolved value")
            assertTrue(settledAtMs >= 50, "the resolver's suspension must not have completed before the delay elapsed")
        }

    @Test
    fun `capability resolver resumes with the resolved value`() =
        runTest {
            val marker = 42
            val resolver =
                CapabilityAsyncResolver(
                    defaultResolver = { _, _ -> FluxValue.IntVal(marker.toLong()) },
                )
            val dispatcher = ReactiveDispatcher.test(StandardTestDispatcher(testScheduler))
            val signals = SignalGraph()
            val executor = executor(resolver, signals, dispatcher)
            launch(dispatcher.dispatcher) { executor.dispatchAsync(1u, FluxValue.NullVal) }
            testScheduler.advanceTimeBy(100)
            runCurrent()
            assertEquals(FluxValue.IntVal(marker.toLong()), signals.read(2u), "handler resumes with the resolver's value")
        }
}
