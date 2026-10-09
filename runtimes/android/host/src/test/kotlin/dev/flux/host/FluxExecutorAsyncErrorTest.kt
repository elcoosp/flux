package dev.flux.host

import dev.flux.host.shadow.ShadowTree
import dev.flux.host.signal.CellState
import dev.flux.host.signal.SignalGraph
import dev.flux.host.transport.MockTransport
import dev.flux.host.vm.FluxValue
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNotNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import java.io.IOException

/**
 * F4: async capability failures must surface as a fault (and mark the result
 * cell [CellState.Error]), never silently substitute `Null`. The synchronous
 * AWAIT path in [dev.flux.host.vm.FluxBytecodeVM] throws a `TYPE_MISMATCH`
 * fault for an Error cell; the async path used to `catch (e: Exception) {
 * NullVal }`, so a network failure produced a blank result with no banner
 * (AGENTS.md §3.11).
 */
@OptIn(ExperimentalCoroutinesApi::class)
class FluxExecutorAsyncErrorTest {
    // Same handler as AsyncResolverTest: CALL_CAP (2,99), AWAIT, WRITE_SIGNAL 2, HALT.
    private val asyncHandler =
        byteArrayOf(
            0x90.toByte(), 0x02, 0x02, 0x00, 0x00, 0x00, 0x63, 0x00, 0x00,
            0xE0.toByte(), 0x00, 0x02,
            0x11.toByte(), 0x02, 0x00, 0x00, 0x00, 0x00,
            0x00,
        )

    private val stdlibEntries =
        (100u..106u).zip(listOf("column", "text", "button", "row", "textinput", "screen", "router")) +
            listOf(200u to "text", 300u to "button", 500u to "screen", 600u to "router")

    private fun executor(
        resolver: AsyncResolver,
        signals: SignalGraph,
        dispatcher: ReactiveDispatcher,
    ): FluxExecutor {
        val tree =
            ShadowTree(
                AdapterRegistry.fromStringTable(
                    stdlibEntries.map { (id, k) -> StringTableEntry(id, k) },
                ),
            )
        val transport = MockTransport()
        val executor =
            FluxExecutor(
                tree,
                signals,
                transport,
                vmScope = TestScope(dispatcher.dispatcher),
                reactiveDispatcher = dispatcher,
            )
        executor.asyncResolver = resolver
        executor.registerClosure(1u, asyncHandler)
        return executor
    }

    /** An [AsyncResolver] that always throws. */
    private class ThrowingResolver(private val ex: Throwable) : AsyncResolver {
        override suspend fun resolve(future: FluxValue): FluxValue = throw ex
    }

    @Test
    fun `async capability failure faults the handler and marks the cell Error`() =
        runTest {
            val dispatcher = ReactiveDispatcher.test(StandardTestDispatcher(testScheduler))
            val signals = SignalGraph()
            val executor = executor(ThrowingResolver(IOException("offline")), signals, dispatcher)
            var reported: FluxError? = null
            executor.onError = { err -> reported = err }

            launch(dispatcher.dispatcher) { executor.dispatchAsync(1u, FluxValue.NullVal) }
            testScheduler.advanceTimeBy(100)
            runCurrent()

            val err = reported
            assertNotNull(err, "async failure must invoke onError, not silently Null")
            assertEquals(
                FluxErrorKind.CAPABILITY,
                err!!.kind,
                "async capability failure must surface as a capability error",
            )
            assertTrue(
                err.message.contains("offline"),
                "error must carry the resolver's message, got ${err.message}",
            )

            // The handler must NOT have resumed to WRITE_SIGNAL 2.
            assertEquals(
                null,
                signals.read(2u),
                "handler must not resume after the future faulted",
            )

            // The result cell that was Pending must now be Error.
            // The cell id is drawn from the store's high allocator; scan for it.
            val errorCells =
                (1_000_001u..1_000_100u).filter { signals.cellState(it) == CellState.Error }
            assertEquals(
                1,
                errorCells.size,
                "exactly one cell should be marked Error, got ${errorCells.size}",
            )
        }

    @Test
    fun `passthrough resolver failure also faults (does not swallow)`() =
        runTest {
            val dispatcher = ReactiveDispatcher.test(StandardTestDispatcher(testScheduler))
            val signals = SignalGraph()
            val executor = executor(ThrowingResolver(RuntimeException("boom")), signals, dispatcher)
            var reported: FluxError? = null
            executor.onError = { err -> reported = err }

            launch(dispatcher.dispatcher) { executor.dispatchAsync(1u, FluxValue.NullVal) }
            testScheduler.advanceTimeBy(100)
            runCurrent()

            assertNotNull(reported, "any exception type must be caught and reported")
        }
}
