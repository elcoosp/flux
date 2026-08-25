package dev.flux.host

import dev.flux.host.shadow.ShadowTree
import dev.flux.host.signal.SignalGraph
import dev.flux.host.transport.MockTransport
import dev.flux.host.vm.CapabilityImpl
import dev.flux.host.vm.CapabilityKey
import dev.flux.host.vm.CapabilityRegistry
import dev.flux.host.vm.FluxBytecodeVM
import dev.flux.host.vm.FluxValue
import dev.flux.host.vm.InMemorySignals
import dev.flux.host.vm.TableStringResolver
import dev.flux.host.vm.VmErrorKind
import dev.flux.host.vm.VmResult
import dev.flux.host.wire.ClosureRef
import dev.flux.host.wire.FrameBuilder
import dev.flux.host.wire.FrameDeserializer
import dev.flux.host.wire.WireValue
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * Spec-gap fixes for the Android runtime (P2 / FLUX-00X): G1 handler transport,
 * G2 memory cap, G3 real string ops, G4 CALL_CAP registry, G5 lifecycle hooks,
 * G6 `@pure` reconciliation skip. One RED-then-GREEN test per gap.
 *
 * These pin the behaviors the production runtime must guarantee so a
 * regression is caught without re-running the full conformance suite.
 */
class RuntimeFixesTest {
    private val stdlibKinds = listOf("column", "text", "button", "row", "text_field", "screen", "router")
    private val stdlibEntries = (100u..106u).zip(stdlibKinds)

    // ── G1: decode + register handlers (Gap G1, critical) ────────────────────

    @Test
    fun `init frame with handler registers and runs on dispatch`() =
        runTest {
            val dispatcher = StandardTestDispatcher(testScheduler)
            val scope = TestScope(dispatcher)
            val signals = SignalGraph()
            signals.seed(listOf(1u to FluxValue.NullVal))

            // Handler 5 writes signal 1 = 1 (READ_SIGNAL r0,1; LOAD_INT_CONST r1,1;
            // ADD_I64 r0,r0,r1; WRITE_SIGNAL 1,r0; HALT).
            val handlerBytecode = counterSetClosure()
            val blob = handlerBytecode
            val handlerId = 5u
            val closureRef =
                ClosureRef(
                    hash = ByteArray(8),
                    bytecodeOffset = 0u,
                    bytecodeLen = handlerBytecode.size.toUShort(),
                    signals = emptyList(),
                )

            val bytes =
                FrameBuilder()
                    .apply {
                        magic()
                        version(1)
                        seq(0)
                        flags(fullTree = true, hasPure = false)
                        patchCount(0)
                        handlerCount(1)
                        stringCount(stdlibEntries.size)
                        handlerSection(blob, listOf(handlerId to closureRef))
                        for ((id, kind) in stdlibEntries) stringEntry(id, kind)
                        node(id = 1u, kind = 0x12u, component = 100u, props = emptyList(), childIds = listOf(2u))
                        node(id = 2u, kind = 0x10u, component = 200u, props = emptyList(), childIds = emptyList())
                    }.build()

            val frame = FrameDeserializer.deserialize(bytes)
            assertEquals(1, frame.handlers.size)
            assertEquals(handlerId, frame.handlers[0].handlerId)

            val tree = ShadowTree(AdapterRegistry.fromStringTable(stdlibEntries.map { (id, k) -> StringTableEntry(id, k) }))
            val transport = MockTransport()
            val executor = FluxExecutor(tree, signals, transport, vmScope = scope, reactiveDispatcher = dispatcher)
            executor.onError = { throw AssertionError("executor error: $it") }
            // The real integration path: receiveFrame deserializes, registers
            // handlers into the VM, and applies the tree.
            executor.receiveFrame(bytes)
            dispatcher.scheduler.runCurrent()

            // Handler must now be registered and dispatchable.
            executor.dispatch(handlerId)
            signals.flush()
            assertEquals(FluxValue.IntVal(1), signals.read(1u))
        }

    // ── G2: 16 MiB memory cap ────────────────────────────────────────────────

    @Test
    fun `small allocation succeeds`() {
        // One ALLOC_LIST with cap 10 → 80 bytes, well under the cap.
        val prog =
            byteArrayOf(
                0x80.toByte(), // ALLOC_LIST r0, cap=10
                0,
                10,
                0,
                0x00, // HALT
            )
        val out = FluxBytecodeVM.run(prog, InMemorySignals(), FluxValue.NullVal)
        assertTrue(out is VmResult.Success, "small allocation should succeed: $out")
    }

    @Test
    fun `allocation past 16 MiB fails with MemoryExhausted`() {
        // 40 ALLOC_LIST(r0, cap=65535): each adds 65535*8 = 524280 bytes.
        // 31st crosses 16_000_000 → MEMORY_EXHAUSTED.
        val prog = ByteArray(40 * 4 + 1)
        for (i in 0 until 40) {
            val base = i * 4
            prog[base] = 0x80.toByte() // ALLOC_LIST
            prog[base + 1] = 0 // dst r0
            prog[base + 2] = 0xFF.toByte() // cap lo
            prog[base + 3] = 0xFF.toByte() // cap hi (u16 = 65535)
        }
        prog[40 * 4] = 0x00 // HALT (unreached)
        val out = FluxBytecodeVM.run(prog, InMemorySignals(), FluxValue.NullVal)
        assertTrue(out is VmResult.Failure, "over-cap allocation must fault: $out")
        out as VmResult.Failure
        assertEquals(VmErrorKind.MEMORY_EXHAUSTED, out.kind)
    }

    // ── G3: real STR_LEN / STR_CONCAT ────────────────────────────────────────

    @Test
    fun `STR_LEN resolves real string length`() {
        // LOAD_STR_CONST r0, 7 ; STR_LEN r1, r0 ; HALT  (string 7 = "hello").
        val prog =
            byteArrayOf(
                0xB3.toByte(),
                0,
                7,
                0,
                0,
                0, // LOAD_STR_CONST r0, 7
                0x53.toByte(),
                1,
                0,
                0, // STR_LEN r1, r0
                0x00,
            )
        val resolver = TableStringResolver(mapOf(7u to "hello"))
        val out = FluxBytecodeVM.run(prog, InMemorySignals(), FluxValue.NullVal, resolver)
        assertTrue(out is VmResult.Success, "str_len should succeed: $out")
        out as VmResult.Success
        assertEquals(FluxValue.IntVal(5), out.outcome.registers[1])
    }

    @Test
    fun `STR_CONCAT joins real strings then STR_LEN observes length`() {
        // r0=7("hello"), r1=8("world"), r2=concat, r3=len(r2) → 10.
        val prog =
            byteArrayOf(
                0xB3.toByte(),
                0,
                7,
                0,
                0,
                0, // LOAD_STR_CONST r0, 7
                0xB3.toByte(),
                1,
                8,
                0,
                0,
                0, // LOAD_STR_CONST r1, 8
                0x50.toByte(),
                2,
                0,
                1, // STR_CONCAT r2, r0, r1
                0x53.toByte(),
                3,
                2,
                0, // STR_LEN r3, r2
                0x00,
            )
        val resolver = TableStringResolver(mapOf(7u to "hello", 8u to "world"))
        val out = FluxBytecodeVM.run(prog, InMemorySignals(), FluxValue.NullVal, resolver)
        assertTrue(out is VmResult.Success, "str_concat should succeed: $out")
        out as VmResult.Success
        assertEquals(FluxValue.IntVal(10), out.outcome.registers[3])
    }

    // ── G4: CALL_CAP registry (data-driven) ─────────────────────────────────

    @Test
    fun `CALL_CAP routes a non default cap id through the registry`() {
        // CALL_CAP r0, cap=2, method=5, args=r1 ; HALT. Registry returns 42.
        val prog =
            byteArrayOf(
                0x90.toByte(), // CALL_CAP
                0, // result reg r0
                2,
                0,
                0,
                0, // capId = 2
                5,
                0, // methodId = 5
                1, // args reg r1
                0x00,
            )
        val registry =
            CapabilityRegistry.fromEntries(
                listOf(
                    CapabilityKey(2u, 5u.toUShort()) to
                        CapabilityImpl { _args, _signals -> FluxValue.IntVal(42) },
                ),
            )
        val out = FluxBytecodeVM.run(prog, InMemorySignals(), FluxValue.NullVal, capabilities = registry)
        assertTrue(out is VmResult.Success, "registered cap should succeed: $out")
        out as VmResult.Success
        assertEquals(FluxValue.IntVal(42), out.outcome.registers[0])
    }

    @Test
    fun `CALL_CAP with no registry entry faults`() {
        val prog =
            byteArrayOf(
                0x90.toByte(),
                0,
                2,
                0,
                0,
                0, // capId = 2 (unregistered)
                5,
                0,
                1,
                0x00,
            )
        // Empty registry → unknown cap faults as TYPE_MISMATCH.
        val out = FluxBytecodeVM.run(prog, InMemorySignals(), FluxValue.NullVal, capabilities = CapabilityRegistry.EMPTY)
        assertTrue(out is VmResult.Failure, "unregistered cap must fault: $out")
        out as VmResult.Failure
        assertEquals(VmErrorKind.TYPE_MISMATCH, out.kind)
    }

    // ── G5: lifecycle hooks ──────────────────────────────────────────────────

    @Test
    fun `onMount runs on node creation and mutates state`() =
        runTest {
            val dispatcher = StandardTestDispatcher(testScheduler)
            val scope = TestScope(dispatcher)
            val signals = SignalGraph()

            // onMount for node 2 writes signal 99 = 7.
            val onMount = closureWriteSignal(99u, 7)
            val bytes =
                FrameBuilder()
                    .apply {
                        magic()
                        version(1)
                        seq(0)
                        flags(fullTree = true)
                        patchCount(0)
                        handlerCount(0)
                        stringCount(stdlibEntries.size)
                        for ((id, kind) in stdlibEntries) stringEntry(id, kind)
                        node(id = 1u, kind = 0x12u, component = 100u, props = emptyList(), childIds = listOf(2u))
                        node(id = 2u, kind = 0x10u, component = 200u, props = emptyList(), childIds = emptyList())
                    }.build()

            val frame = FrameDeserializer.deserialize(bytes)
            val tree = ShadowTree(AdapterRegistry.fromStringTable(stdlibEntries.map { (id, k) -> StringTableEntry(id, k) }))
            val transport = MockTransport()
            val executor = FluxExecutor(tree, signals, transport, vmScope = scope, reactiveDispatcher = dispatcher)
            executor.onError = { throw AssertionError("g5 onMount error: $it") }
            executor.registerLifecycle(2u, FluxExecutor.LifecycleHooks(onMount = onMount))
            tree.applyFrame(frame, executor)
            dispatcher.scheduler.runCurrent()
            signals.flush()
            assertEquals(FluxValue.IntVal(7), signals.read(99u))
        }

    @Test
    fun `onCleanup runs on remove patch`() =
        runTest {
            val dispatcher = StandardTestDispatcher(testScheduler)
            val scope = TestScope(dispatcher)
            val signals = SignalGraph()

            val onCleanup = closureWriteSignal(98u, 1)
            val initBytes =
                FrameBuilder()
                    .apply {
                        magic()
                        version(1)
                        seq(0)
                        flags(fullTree = true)
                        patchCount(0)
                        handlerCount(0)
                        stringCount(stdlibEntries.size)
                        for ((id, kind) in stdlibEntries) stringEntry(id, kind)
                        node(id = 1u, kind = 0x12u, component = 100u, props = emptyList(), childIds = listOf(2u))
                        node(id = 2u, kind = 0x10u, component = 200u, props = emptyList(), childIds = emptyList())
                    }.build()
            val frame = FrameDeserializer.deserialize(initBytes)
            val tree = ShadowTree(AdapterRegistry.fromStringTable(stdlibEntries.map { (id, k) -> StringTableEntry(id, k) }))
            val transport = MockTransport()
            val executor = FluxExecutor(tree, signals, transport, vmScope = scope, reactiveDispatcher = dispatcher)
            executor.onError = { throw AssertionError("g5 onCleanup error: $it") }
            executor.registerLifecycle(2u, FluxExecutor.LifecycleHooks(onCleanup = onCleanup))
            tree.applyFrame(frame, executor)
            dispatcher.scheduler.runCurrent()

            // Remove node 2 → onCleanup fires.
            val remove =
                FrameBuilder()
                    .apply {
                        magic()
                        version(1)
                        seq(1)
                        flags(fullTree = false)
                        patchCount(1)
                        handlerCount(0)
                        stringCount(0)
                        patchRemove(2u)
                    }.build()
            tree.applyFrame(FrameDeserializer.deserialize(remove), executor)
            dispatcher.scheduler.runCurrent()
            signals.flush()
            assertEquals(FluxValue.IntVal(1), signals.read(98u))
        }

    // ── G6: @pure subtree skip ──────────────────────────────────────────────

    @Test
    fun `pure node with unchanged props is not re reconciled on unrelated update`() =
        runTest {
            val dispatcher = StandardTestDispatcher(testScheduler)
            val scope = TestScope(dispatcher)
            val signals = SignalGraph()
            val bytes =
                FrameBuilder()
                    .apply {
                        magic()
                        version(1)
                        seq(0)
                        flags(fullTree = true, hasPure = true)
                        patchCount(0)
                        handlerCount(0)
                        stringCount(stdlibEntries.size)
                        for ((id, kind) in stdlibEntries) stringEntry(id, kind)
                        // root column (id=1) with a pure text child (id=2) and a sibling (id=3).
                        node(id = 1u, kind = 0x12u, component = 100u, props = emptyList(), childIds = listOf(2u, 3u))
                        node(
                            id = 2u,
                            kind = 0x10u,
                            component = 200u,
                            props = listOf(0u.toUShort() to WireValue.StrVal(7u)),
                            childIds = emptyList(),
                            pure = true,
                        )
                        node(
                            id = 3u,
                            kind = 0x10u,
                            component = 200u,
                            props = listOf(0u.toUShort() to WireValue.StrVal(8u)),
                            childIds = emptyList(),
                        )
                    }.build()
            val frame = FrameDeserializer.deserialize(bytes)
            assertEquals(true, frame.extraNodes.first { it.id == 2u }.isPure)

            val tree = ShadowTree(AdapterRegistry.fromStringTable(stdlibEntries.map { (id, k) -> StringTableEntry(id, k) }))
            val transport = MockTransport()
            val executor = FluxExecutor(tree, signals, transport, vmScope = scope, reactiveDispatcher = dispatcher)
            tree.applyFrame(frame, executor)
            assertEquals(1, tree.reconcileCount(2u))
            assertEquals(1, tree.reconcileCount(3u))

            // Update the sibling (id=3) only — the pure node 2 must NOT reconcile again.
            val update =
                FrameBuilder()
                    .apply {
                        magic()
                        version(1)
                        seq(1)
                        flags(fullTree = false)
                        patchCount(1)
                        handlerCount(0)
                        stringCount(0)
                        patchUpdate(id = 3u, changes = listOf(0u.toUShort() to WireValue.StrVal(77u)))
                    }.build()
            tree.applyFrame(FrameDeserializer.deserialize(update), executor)
            assertEquals(1, tree.reconcileCount(2u), "@pure node must not re-reconcile on unrelated update")
            assertEquals(2, tree.reconcileCount(3u), "sibling should reconcile")
        }

    @Test
    fun `pure node with identical props update is skipped`() =
        runTest {
            val dispatcher = StandardTestDispatcher(testScheduler)
            val scope = TestScope(dispatcher)
            val signals = SignalGraph()
            val bytes =
                FrameBuilder()
                    .apply {
                        magic()
                        version(1)
                        seq(0)
                        flags(fullTree = true, hasPure = true)
                        patchCount(0)
                        handlerCount(0)
                        stringCount(stdlibEntries.size)
                        for ((id, kind) in stdlibEntries) stringEntry(id, kind)
                        node(id = 1u, kind = 0x12u, component = 100u, props = emptyList(), childIds = listOf(2u))
                        node(
                            id = 2u,
                            kind = 0x10u,
                            component = 200u,
                            props = listOf(0u.toUShort() to WireValue.StrVal(7u)),
                            childIds = emptyList(),
                            pure = true,
                        )
                    }.build()
            val frame = FrameDeserializer.deserialize(bytes)
            val tree = ShadowTree(AdapterRegistry.fromStringTable(stdlibEntries.map { (id, k) -> StringTableEntry(id, k) }))
            val transport = MockTransport()
            val executor = FluxExecutor(tree, signals, transport, vmScope = scope, reactiveDispatcher = dispatcher)
            tree.applyFrame(frame, executor)
            assertEquals(1, tree.reconcileCount(2u))

            // Re-send the SAME props for the pure node → skip.
            val update =
                FrameBuilder()
                    .apply {
                        magic()
                        version(1)
                        seq(1)
                        flags(fullTree = false)
                        patchCount(1)
                        handlerCount(0)
                        stringCount(0)
                        patchUpdate(id = 2u, changes = listOf(0u.toUShort() to WireValue.StrVal(7u)))
                    }.build()
            tree.applyFrame(FrameDeserializer.deserialize(update), executor)
            assertEquals(1, tree.reconcileCount(2u), "@pure node with identical props must skip reconcile")
        }

    // ── bytecode helpers ─────────────────────────────────────────────────────

    /** Writes [value] into signal [id], then halts (LOAD_INT_CONST r0,value ; WRITE_SIGNAL id,r0 ; HALT). */
    private fun closureWriteSignal(
        id: UInt,
        value: Long,
    ): ByteArray =
        byteArrayOf(
            0xB0.toByte(),
            0,
            value.toInt().toByte(),
            0,
            0,
            0,
            0,
            0,
            0,
            0, // LOAD_INT_CONST r0, value
            0x11.toByte(),
            id.toInt().toByte(),
            0,
            0,
            0,
            0, // WRITE_SIGNAL id, r0
            0x00, // HALT
        )

    /** Writes signal 1 = 1 directly (LOAD_INT_CONST r0,1 ; WRITE_SIGNAL 1,r0 ; HALT). */
    private fun counterSetClosure(): ByteArray =
        byteArrayOf(
            0xB0.toByte(),
            0,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0, // LOAD_INT_CONST r0, 1
            0x11.toByte(),
            1,
            0,
            0,
            0,
            0, // WRITE_SIGNAL 1, r0
            0x00, // HALT
        )
}
