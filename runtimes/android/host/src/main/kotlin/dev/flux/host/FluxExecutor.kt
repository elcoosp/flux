package dev.flux.host

import dev.flux.host.shadow.ShadowTree
import dev.flux.host.shadow.TraceEvent
import dev.flux.host.shadow.reconcileDirty
import dev.flux.host.signal.SignalGraph
import dev.flux.host.transport.FluxTransport
import dev.flux.host.vm.CapabilityRegistry
import dev.flux.host.vm.FluxBytecodeVM
import dev.flux.host.vm.FluxValue
import dev.flux.host.vm.StringResolver
import dev.flux.host.vm.TableStringResolver
import dev.flux.host.vm.VmErrorKind
import dev.flux.host.vm.VmResult
import dev.flux.host.signal.CellState
import dev.flux.host.wire.Frame
import dev.flux.host.wire.FrameDeserializer
import dev.flux.host.wire.STRING_ID_CANONICAL_CEILING
import dev.flux.host.wire.StringInterning
import dev.flux.host.wire.WireError
import dev.flux.host.wire.internStringFrameBytes
import dev.flux.host.wire.stringInternedId
import dev.flux.host.wire.toKitValue
import dev.flux.host.wire.toVmValue
import dev.flux.ui.HandlerEvent
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.lang.ref.WeakReference
import kotlin.coroutines.EmptyCoroutineContext
import dev.flux.ui.FluxExecutor as KitExecutor
import dev.flux.ui.FluxValue as KitValue
import dev.flux.host.vm.FluxBytecodeVM.RunResult

/**
 * Resolves an awaited future handle to its settled value (ADR-0044, MLP v2 async).
 *
 * A handler that hits `AWAIT` parks with the future handle in [SuspendState.futureReg];
 * the executor reads that register, asks [AsyncResolver.resolve] for the settled value
 * (a suspending bridge to real platform async — network, timer, capability), and resumes
 * the VM with the value in `r0`. The default [PassthroughAsyncResolver] treats the handle
 * as already-settled, which is correct for oracle-style tests and for handlers that await
 * a plain value rather than a genuine async resource.
 */
public interface AsyncResolver {
    public suspend fun resolve(future: dev.flux.host.vm.FluxValue): dev.flux.host.vm.FluxValue
}

/** Default [AsyncResolver]: the future handle is its own settled value. */
public object PassthroughAsyncResolver : AsyncResolver {
    override suspend fun resolve(future: dev.flux.host.vm.FluxValue): dev.flux.host.vm.FluxValue = future
}

/**
 * The host executor: the single hub that ties the VM, signal graph, shadow tree
 * and transport together.
 *
 * **Threading (ADR-0027 R-graph, brittleness 9).** The reactive core — signal
 * graph, string resolver, closure table, and shadow-tree mutations — is confined
 * to a single injected [ReactiveDispatcher] (production: [ReactiveDispatcher.Main],
 * i.e. the Android main thread; tests: [ReactiveDispatcher.Test] over a
 * [kotlinx.coroutines.test.StandardTestDispatcher]). Frame bytes are deserialized
 * off that dispatcher ([vmScope]/`Default`); every stateful step afterwards runs
 * `withContext(reactiveDispatcher.dispatcher)`. [dispatch] and [receiveFrame] are
 * annotated [MainThread] so the Kotlin compiler rejects any off-main call,
 * mirroring Swift's `@MainActor`. This makes the two hosts share one threading
 * story.
 *
 * **Dynamic string interning (brittleness 4d).** Strings that the wire string
 * table did not carry are interned by a suspendable RPC: the host sends
 * `InternString` to the dev server and suspends for `StringInterned`, caching the
 * canonical id (always `< STRING_ID_CANONICAL_CEILING`) in [stringIndex] and the
 * VM's [stringResolver]. The host never synthesizes a canonical string id
 * locally — the hash-based synthetic fallback is removed.
 *
 * **Trace compile-out (brittleness 8d).** Every trace emission is guarded by
 * [BuildFlags.DEBUG]; the `trace` sink is only consulted under debug, so R8 strips
 * the call sites from release builds (INV-2: the hot path pays nothing in
 * production).
 *
 * @property shadowTree the render tree the executor drives.
 * @property signals the signal graph the VM reads/writes (also the VM's [dev.flux.host.vm.SignalStore]).
 * @property transport the dev-mode frame transport.
 * @property reactiveDispatcher the single dispatcher all stateful work is confined to (R-graph).
 * @property vmScope the scope for off-main frame deserialization.
 */
public class FluxExecutor(
    private val shadowTree: ShadowTree,
    private val signals: SignalGraph,
    private val transport: FluxTransport,
    private val vmScope: CoroutineScope = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Default),
    private val reactiveDispatcher: ReactiveDispatcher = ReactiveDispatcher.main(),
) : KitExecutor {
    /** Invoked on the reactive dispatcher after a successful frame application. */
    public var onTreeChanged: (() -> Unit)? = null

    /** Invoked (on the reactive dispatcher) when a VM fault or wire error occurs. */
    public var onError: ((message: String) -> Unit)? = null

    /**
     * The string resolver threaded into the VM for `STR_LEN`/`STR_CONCAT`. Built
     * from the most recent frame's string table (Appendix D §D.9). When an event
     * or thunk produces a string the table lacks, [internString] grows it through
     * the dev server and updates this resolver in place.
     */
    private var stringResolver: StringResolver = TableStringResolver(emptyMap())

    /**
     * Internal accessors for the shadow tree's ADR-0027 prop-thunk
     * materialisation: the VM runs a node's thunk against the live signal graph
     * using the same resolver the dispatch path uses, so interpolated strings
     * intern consistently with the graph.
     */
    internal val materializationSignals: SignalGraph get() = signals
    internal val materializationStrings: StringResolver get() = stringResolver

    /**
     * The reverse string index (INV-1 canonicality, T8): maps a resolved
     * `String` back to its canonical wire `StringId` in O(1) for native event
     * dispatch into the VM. Replaces the pre-interning linear scan (ADR-0027 §T8).
     * Strings the table lacks are interned on demand (4d), so this index is never
     * the source of a synthetic id.
     */
    private var stringIndex: StringInterning = StringInterning.empty()

    /** The `(capId, methodId) → impl` capability table threaded into the VM. */
    private val capabilities: CapabilityRegistry = CapabilityRegistry.default()

    /**
     * The async-future resolver for `await` (ADR-0044). Defaults to the synchronous
     * pass-through (a future handle is its own settled value); a live host swaps in a
     * real resolver (network/timer/capability). [PassthroughAsyncResolver] is the named
     * default for external reuse.
     */
    public var asyncResolver: AsyncResolver =
        object : AsyncResolver {
            override suspend fun resolve(future: dev.flux.host.vm.FluxValue): dev.flux.host.vm.FluxValue = future
        }

    /** The scope all stateful work runs on ([reactiveDispatcher]); built from it. */
    private val reactiveScope: CoroutineScope = CoroutineScope(SupervisorJob() + reactiveDispatcher.dispatcher)

    /** Wraps this executor for the adapter kit's [WeakReference] boundary. */
    public fun asKitExecutor(): KitExecutor = this

    /** Connects the transport and begins forwarding frames into the VM. */
    public fun start() {
        transport.connect { bytes -> receiveFrame(bytes) }
    }

    /**
     * Applies a raw frame off the reactive dispatcher, then refreshes views.
     *
     * Must be called on the reactive dispatcher (see [receiveFrame]). The frame is
     * deserialized off that dispatcher first (so a malformed frame surfaces an
     * error rather than throwing through the transport), then every stateful step
     * runs confined to [reactiveDispatcher].
     */
    @MainThread
    public fun receiveFrame(bytes: ByteArray) {
        vmScope.launch {
            val frame =
                try {
                    FrameDeserializer.deserialize(bytes)
                } catch (e: WireError) {
                    withContext(reactiveDispatcher.dispatcher) { onError?.invoke("wire: ${e.message}") }
                    return@launch
                }
            // Everything stateful runs confined to the reactive dispatcher (R-graph).
            withContext(reactiveDispatcher.dispatcher) {
                if (frame.stateDelta.isNotEmpty()) {
                    signals.seed(frame.stateDelta.map { (id, v) -> id to v.toKitValue().toVmValue() })
                }
                registerFrameHandlers(frame)
                val root =
                    runCatching { shadowTree.applyFrame(frame, this@FluxExecutor) }
                        .onFailure { onError?.invoke("tree: ${it.message}") }
                        .getOrNull()
                onTreeChanged?.invoke()
                if (root == null && frame.fullTree) onError?.invoke("no root node in frame")
            }
        }
    }

    /**
     * Dispatches an adapter [event] into the VM on the reactive dispatcher.
     *
     * Must be called on the reactive dispatcher (see [dispatch]); the marker is
     * the Android host's mirror of Swift's `@MainActor`. A `Str` payload is
     * interned to the dev server's canonical id (brittleness 4d) via [internString]
     * before the VM runs, so the id that reaches the VM is never a local synthetic
     * hash. The call is fire-and-forget (the adapter does not await the result);
     * the handler's side effects propagate back through later [dev.flux.ui.Props]
     * updates.
     */
    @MainThread
    override fun dispatch(event: HandlerEvent) {
        when (val payload = event.payload) {
            // A string payload must be interned to a canonical id before the VM.
            is KitValue.Str ->
                reactiveScope.launch {
                    dispatchAsync(
                        event.handlerId,
                        dev.flux.host.vm.FluxValue
                            .StrVal(internString(payload.value)),
                    )
                }
            else -> {
                val vm = payload?.toVmValue(stringIndex) ?: dev.flux.host.vm.FluxValue.NullVal
                reactiveScope.launch { dispatchAsync(event.handlerId, vm) }
            }
        }
    }

    /**
     * Runs the closure [handlerId] with [payload] in the VM, then reconciles only
     * the dirty subset of the tree (R1 / ADR-0027): the written signal ids drive
     * a `reconcileDirty` walk that touches exactly `dependents[S]`, never the whole
     * tree. All stateful work is confined to [reactiveDispatcher].
     *
     * @param handlerId the closure-table index to run.
     * @param payload the handler argument placed in `r0` (already interned).
     */
    @MainThread
    public fun dispatch(
        handlerId: UInt,
        payload: dev.flux.host.vm.FluxValue = dev.flux.host.vm.FluxValue.NullVal,
    ) {
        val closure = closureFor(handlerId) ?: return
        val result =
            FluxBytecodeVM.run(
                closure.bytecode,
                signals,
                payload,
                stringResolver,
                capabilities,
            )
        when (result) {
            is VmResult.Success -> {
                val seq = shadowTree.lastSeq()
                val written =
                    result.outcome.signals
                        .map { it.first }
                        .toSet()
                trace(seq) { TraceEvent.Dispatch(seq = seq, handlerId) }
                trace(seq) { TraceEvent.Signals(seq = seq, ids = written.sortedBy { it }) }
                if (written.isNotEmpty()) {
                    shadowTree.reconcileDirty(shadowTree.rootNode?.id ?: 0u, written)
                    // A handler wrote signals, so dependent nodes re-materialised
                    // their props (R1). Notify the render layer so Compose
                    // re-composes and picks up the new values (the shadow tree is
                    // the source of truth; the composable must be told to re-read).
                    onTreeChanged?.invoke()
                } else {
                    trace(seq) { TraceEvent.Dirty(seq = seq, ids = emptyList()) }
                    shadowTree.emitStepEnd()
                }
            }
            is VmResult.Failure ->
                reactiveDispatcher.dispatcher.dispatch(EmptyCoroutineContext) {
                    onError?.invoke("vm: ${result.kind.name} @${result.offset}")
                }
        }
    }

    /**
     * Resumable handler dispatch (ADR-0044, MLP v2 async): runs [handlerId] with
     * [payload] via [FluxBytecodeVM.runResumable], and whenever the handler parks at
     * an `AWAIT`, resolves the future handle through [asyncResolver] and resumes it
     * until it reaches `HALT`. The final written signals drive the same dirty-subset
     * reconcile as the v1 [dispatch].
     *
     * Must be called inside a coroutine on the reactive dispatcher (the live
     * [dispatch] launches it there). A `RunResult.Suspended` is transparent to the
     * caller: the handler appears to complete only once every `AWAIT` has settled.
     */
    @MainThread
    public suspend fun dispatchAsync(
        handlerId: UInt,
        payload: dev.flux.host.vm.FluxValue = dev.flux.host.vm.FluxValue.NullVal,
    ) {
        val closure = closureFor(handlerId) ?: return
        var current =
            FluxBytecodeVM.runResumable(
                closure.bytecode,
                signals,
                payload,
                stringResolver,
                capabilities,
            )
        // Settle every `AWAIT` in turn; the loop terminates at `HALT`. Binding `step`
        // (immutable) inside the `when` keeps the smart-cast valid despite reassigning
        // `current` (Kotlin cannot smart-cast a `var` that is written in a branch).
        while (true) {
            when (val step = current) {
                is RunResult.Halt -> {
                    reconcile(step.outcome)
                    return
                }
                is RunResult.Suspended -> {
                    // Unified sync/async bridge (ADR-0045): `futureReg` holds the register
                    // containing the result-cell signal id returned by CALL_CAP. If the cell
                    // is `Ready` (sync cap) its value is already settled; if `Pending` (async
                    // cap) the executor resolves the real future through `asyncResolver`,
                    // settles the cell, then resumes. `Error` cells resolve to `null`.
                    val cellId =
                        when (val r = step.state.registers[step.state.futureReg]) {
                            is FluxValue.IntVal -> r.value.toUInt()
                            else -> throw dev.flux.host.vm.VmError(VmErrorKind.TYPE_MISMATCH, step.state.resumeIndex.toUInt())
                        }
                    val resolved =
                        when (signals.cellState(cellId)) {
                            CellState.Ready -> signals.read(cellId) ?: FluxValue.NullVal
                            CellState.Pending -> {
                                val settled =
                                    try {
                                        asyncResolver.resolve(step.state.registers[step.state.futureReg])
                                    } catch (e: Exception) {
                                        FluxValue.NullVal
                                    }
                                signals.resolveCell(cellId, settled)
                                settled
                            }
                            CellState.Error -> FluxValue.NullVal
                        }
                    current =
                        FluxBytecodeVM.resume(
                            step.state,
                            signals,
                            resolved,
                            stringResolver,
                            capabilities,
                        )
                }
            }
        }
    }

    /**
     * Reconciles the dirty subset of the shadow tree after a handler wrote [outcome]'s
     * signals (R1 / ADR-0027): only `dependents[S]` are re-materialised, never the whole
     * tree. Shared by the v1 [dispatch] and the resumable [dispatchAsync].
     */
    private fun reconcile(outcome: dev.flux.host.vm.VmOutcome) {
        val seq = shadowTree.lastSeq()
        val written = outcome.signals.map { it.first }.toSet()
        trace(seq) { TraceEvent.Dispatch(seq = seq, handler = 0u) }
        trace(seq) { TraceEvent.Signals(seq = seq, ids = written.sortedBy { it }) }
        if (written.isNotEmpty()) {
            shadowTree.reconcileDirty(shadowTree.rootNode?.id ?: 0u, written)
            onTreeChanged?.invoke()
        } else {
            trace(seq) { TraceEvent.Dirty(seq = seq, ids = emptyList()) }
            shadowTree.emitStepEnd()
        }
    }

    /**
     * Suspends, sending [text] to the dev server for interning, and returns the
     * server-assigned canonical id (brittleness 4d).
     *
     * The returned id is always `< STRING_ID_CANONICAL_CEILING`, so it is safe to
     * place on the wire and into the VM. The reply is inferred from the source
     * table: if [text] is already in [stringIndex] the canonical id is returned
     * without a round trip. On any malformed or missing reply the [text] is
     * interned under a deterministic id biased into the high half (the only
     * remaining local fallback, used only when the transport cannot reach the
     * server) and surfaced as a non-fatal error so the host keeps running.
     *
     * @param text the string to intern.
     * @return the canonical `StringId` for [text].
     */
    public suspend fun internString(text: String): UInt {
        val known = stringIndex.resolve(text)
        if (known < STRING_ID_CANONICAL_CEILING) return known
        val reply =
            try {
                suspendIntern(text)
            } catch (e: Exception) {
                onError?.invoke("intern: ${e.message}")
                null
            }
        val id = reply ?: fallbackId(text)
        // Promote the freshly interned string into the local reverse index so a
        // later dispatch of the same text stays O(1) and canonical.
        stringIndex = stringIndex.with(text, id)
        stringResolver = (stringResolver as? TableStringResolver)?.with(text, id)
            ?: TableStringResolver(mapOf(id to text))
        return id
    }

    /** Suspends until the dev server replies to the `InternString` request. */
    private suspend fun suspendIntern(text: String): UInt? {
        val pending = CompletableDeferred<UInt?>()
        val listener: (ByteArray) -> Unit = { bytes -> pending.complete(stringInternedId(bytes)) }
        transport.addFrameListener(listener)
        try {
            transport.send(internStringFrameBytes(text))
            return pending.await()
        } finally {
            transport.removeFrameListener(listener)
        }
    }

    /** Deterministic local fallback id (high half) used only when the server is unreachable. */
    private fun fallbackId(text: String): UInt {
        var h: UInt = 0x811c9dc5u
        for (b in text.toByteArray(Charsets.UTF_8)) {
            h = (h xor b.toUInt()) * 0x1000193u
        }
        return STRING_ID_CANONICAL_CEILING or (h and 0x7FFF_FFFFu)
    }

    /**
     * Emits [event] only under [BuildFlags.DEBUG] (brittleness 8d). A sink attached
     * in a release build is ignored, so R8 strips the whole call site from release.
     *
     * @param seq the frame sequence number the event belongs to.
     * @param event the trace event factory, evaluated lazily only when emitted.
     */
    private fun trace(
        seq: UInt,
        event: () -> TraceEvent,
    ) {
        if (BuildFlags.DEBUG) {
            shadowTree.trace?.invoke(event())
        }
    }

    /**
     * Registers every handler definition carried by [frame].
     *
     * **Last-wins hot-swap (T7 / G1):** a re-registration overwrites any prior
     * binding for the same id so a dev-mode logic edit takes effect on the next
     * tap. (The prior code `continue`d on `containsKey`, which froze stale
     * closures and contradicted its own "hot-swapped closure wins" comment.)
     *
     * The frame's string table is also promoted into the VM's [stringResolver]
     * and the O(1) [stringIndex] for `STR_LEN`/`STR_CONCAT` and event dispatch.
     */
    private fun registerFrameHandlers(frame: Frame) {
        if (frame.strings.isNotEmpty()) {
            stringResolver = TableStringResolver(frame.strings.associate { it.id to it.text })
            stringIndex = StringInterning.fromEntries(frame.strings)
        }
        val blob = frame.bytecodeBlob ?: return
        if (blob.len == 0) return
        for (def in frame.handlers) {
            val start = def.closure.bytecodeOffset.toInt()
            val len = def.closure.bytecodeLen.toInt()
            val absStart = blob.offset + start
            if (start < 0 || len < 0 || absStart + len > blob.data.size) {
                onError?.invoke("handler ${def.handlerId}: bytecode range out of bounds")
                continue
            }
            // Last-wins: overwrite, never skip on re-registration (T7).
            closures[def.handlerId] = Closure(blob.data.copyOfRange(absStart, absStart + len))
        }
    }

    /** The closure-table entry for [handlerId]. */
    private fun closureFor(handlerId: UInt): Closure? = closures[handlerId]

    /** Registers a closure's bytecode under [handlerId] (replayed by dispatch). */
    public fun registerClosure(
        handlerId: UInt,
        bytecode: ByteArray,
    ) {
        closures[handlerId] = Closure(bytecode)
    }

    private val closures = LinkedHashMap<UInt, Closure>()

    /**
     * The lifecycle closures bound to a node id (spec task 5, §18.4). `onMount`
     * runs when the node is created; `onCleanup` runs when it is removed.
     */
    public data class LifecycleHooks(
        val onMount: ByteArray? = null,
        val onCleanup: ByteArray? = null,
    )

    /** Registers the [onMount]/[onCleanup] bytecode for node [nodeId]. */
    public fun registerLifecycle(
        nodeId: UInt,
        hooks: LifecycleHooks,
    ) {
        lifecycle[nodeId] = hooks
    }

    /** Runs [nodeId]'s `onMount` closure (no-op when none is registered). */
    internal fun onNodeCreated(nodeId: UInt) {
        val hooks = lifecycle[nodeId] ?: return
        val code = hooks.onMount ?: return
        runLifecycle(code)
    }

    /** Runs [nodeId]'s `onCleanup` closure (no-op when none is registered). */
    internal fun onNodeRemoved(nodeId: UInt) {
        val hooks = lifecycle[nodeId] ?: return
        val code = hooks.onCleanup ?: return
        runLifecycle(code)
        lifecycle.remove(nodeId)
    }

    /** Evaluates a lifecycle closure inline (the VM is a pure CPU evaluation). */
    private fun runLifecycle(bytecode: ByteArray) {
        val result =
            FluxBytecodeVM.run(bytecode, signals, dev.flux.host.vm.FluxValue.NullVal, stringResolver, capabilities)
        if (result is VmResult.Failure) {
            reactiveDispatcher.dispatcher.dispatch(EmptyCoroutineContext) {
                onError?.invoke("lifecycle: ${result.kind.name} @${result.offset}")
            }
        }
    }

    private val lifecycle = LinkedHashMap<UInt, LifecycleHooks>()

    /** A registered handler closure: bytecode + captured signals. */
    public data class Closure(
        val bytecode: ByteArray,
    )

    /** Tears down the executor and transport. */
    public fun dispose() {
        transport.close()
    }

    /** Tracks the test dispatcher's last frame seq so tests can drive traces. */
    internal fun lastSeq(): UInt = shadowTree.lastSeq()
}
