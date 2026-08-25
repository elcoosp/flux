package dev.flux.app

import dev.flux.app.shadow.ShadowTree
import dev.flux.app.signal.SignalGraph
import dev.flux.app.transport.FluxTransport
import dev.flux.app.vm.CapabilityRegistry
import dev.flux.app.vm.FluxBytecodeVM
import dev.flux.app.vm.StringResolver
import dev.flux.app.vm.TableStringResolver
import dev.flux.app.vm.VmResult
import dev.flux.app.wire.Frame
import dev.flux.app.wire.FrameDeserializer
import dev.flux.app.wire.StringInterning
import dev.flux.app.wire.WireError
import dev.flux.app.wire.toKitValue
import dev.flux.app.wire.toVmValue
import dev.flux.ui.HandlerEvent
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.lang.ref.WeakReference
import kotlin.coroutines.EmptyCoroutineContext
import dev.flux.ui.FluxExecutor as KitExecutor

/**
 * The host executor: the single hub that ties the VM, signal graph, shadow tree
 * and transport together.
 *
 * Per FLUX-007, VM evaluation and patch application run on [Dispatchers.Default]
 * (a background coroutine pool); native view mutations are posted back to
 * [Dispatchers.Main] so they touch Android views from the main thread only.
 * Adapters reach the executor through a [WeakReference] (via [asKitExecutor]),
 * so the shadow tree — which outlives individual executor instances across
 * hot-swaps — cannot pin a stale executor.
 *
 * @property shadowTree the render tree the executor drives.
 * @property signals the signal graph the VM reads/writes (also the VM's [dev.flux.app.vm.SignalStore]).
 * @property transport the dev-mode frame transport.
 * @property vmScope the coroutine scope for background VM/patch work.
 * @property mainDispatcher the dispatcher used for native view mutations.
 */
public class FluxExecutor(
    private val shadowTree: ShadowTree,
    private val signals: SignalGraph,
    private val transport: FluxTransport,
    private val vmScope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Default),
    private val mainDispatcher: CoroutineDispatcher = Dispatchers.Main,
) : KitExecutor {
    /** Invoked on the main thread after a successful frame application. */
    public var onTreeChanged: (() -> Unit)? = null

    /** Invoked (on any thread) when a VM fault or wire error occurs. */
    public var onError: ((message: String) -> Unit)? = null

    /**
     * The string resolver threaded into the VM for `STR_LEN`/`STR_CONCAT`. Built
     * from the most recent frame's string table (Appendix D §D.9) so handler
     * bytecode resolves real literals rather than the decimal proxy.
     */
    private var stringResolver: StringResolver = TableStringResolver(emptyMap())

    /**
     * The reverse string index (perf task 7, P2): maps a resolved `String`
     * back to its canonical wire `StringId` so native event dispatch into the
     * VM is O(1) and stable, rather than re-hashing per event.
     */
    private var stringIndex: StringInterning = StringInterning.empty()

    /**
     * The `(capId, methodId) → impl` capability table threaded into the VM for
     * `CALL_CAP` (spec task 4). Seeded with the oracle-faithful defaults; dev
     * mode may later register additional RPC-forwarding capabilities.
     */
    private val capabilities: CapabilityRegistry = CapabilityRegistry.default()

    /** Wraps this executor for the adapter kit's [WeakReference] boundary. */
    public fun asKitExecutor(): KitExecutor = this

    /** Connects the transport and begins forwarding frames into the VM. */
    public fun start() {
        transport.connect { bytes -> receiveFrame(bytes) }
    }

    /** Applies a raw frame on the background dispatcher, then refreshes views. */
    public fun receiveFrame(bytes: ByteArray) {
        vmScope.launch {
            val frame =
                try {
                    FrameDeserializer.deserialize(bytes)
                } catch (e: WireError) {
                    onError?.invoke("wire: ${e.message}")
                    return@launch
                }
            if (frame.stateDelta.isNotEmpty()) {
                signals.seed(frame.stateDelta.map { (id, v) -> id to v.toKitValue().toVmValue() })
            }
            // Gap G1 (spec task 1): register every handler body shipped in the
            // frame so bound events can dispatch into the VM. Each `HandlerDef`
            // slices its bytecode out of the shared blob by offset/length.
            registerFrameHandlers(frame)
            val root =
                runCatching { shadowTree.applyFrame(frame, this@FluxExecutor) }
                    .onFailure { onError?.invoke("tree: ${it.message}") }
                    .getOrNull()
            withContext(mainDispatcher) {
                onTreeChanged?.invoke()
                if (root == null && frame.fullTree) onError?.invoke("no root node in frame")
            }
        }
    }

    /** Dispatches an adapter [event] into the VM on the background dispatcher. */
    override fun dispatch(event: HandlerEvent) {
        val payload = event.payload?.toVmValue(stringIndex) ?: dev.flux.app.vm.FluxValue.NullVal
        dispatch(event.handlerId, payload)
    }

    /** Runs the closure [handlerId] with [payload] in the VM, then flushes signals. */
    public fun dispatch(
        handlerId: UInt,
        payload: dev.flux.app.vm.FluxValue = dev.flux.app.vm.FluxValue.NullVal,
    ) {
        val closure = closureFor(handlerId) ?: return
        // The VM is a pure CPU evaluation; running it inline (rather than
        // through an async boundary) keeps signal writes deterministic and
        // observable. Only fault reporting is posted to [mainDispatcher] so a
        // red error overlay is raised on the UI thread (Appendix E §E.6).
        val result =
            FluxBytecodeVM.run(
                closure.bytecode,
                signals,
                payload,
                stringResolver,
                capabilities,
            )
        when (result) {
            is VmResult.Success -> signals.flush()
            is VmResult.Failure ->
                mainDispatcher.dispatch(EmptyCoroutineContext) {
                    onError?.invoke("vm: ${result.kind.name} @${result.offset}")
                }
        }
    }

    /**
     * Registers every handler definition carried by [frame] (Gap G1). Each
     * [dev.flux.app.wire.HandlerDef] names a handler id and a `ClosureRef`
     * indexing the frame's shared bytecode blob; we slice the bytecode out and
     * record it in the closure table unless a newer binding for the same id
     * already exists (a hot-swapped closure wins). The frame's string table is
     * also promoted into the VM's [stringResolver] for `STR_LEN`/`STR_CONCAT`.
     */
    private fun registerFrameHandlers(frame: Frame) {
        if (frame.strings.isNotEmpty()) {
            stringResolver = TableStringResolver(frame.strings.associate { it.id to it.text })
            stringIndex = StringInterning.fromEntries(frame.strings)
        }
        val blob = frame.bytecodeBlob ?: return
        if (blob.bytes.isEmpty()) return
        for (def in frame.handlers) {
            val start = def.closure.bytecodeOffset.toInt()
            val len = def.closure.bytecodeLen.toInt()
            if (start < 0 || len < 0 || start + len > blob.bytes.size) {
                onError?.invoke("handler ${def.handlerId}: bytecode range out of bounds")
                continue
            }
            if (closures.containsKey(def.handlerId)) continue
            closures[def.handlerId] = Closure(blob.bytes.copyOfRange(start, start + len))
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
     * runs when the node is created; `onCleanup` runs when it is removed. The
     * dev server ships these the same way it ships handlers; the host registers
     * them here and the [ShadowTree] triggers them through [onNodeCreated] /
     * [onNodeRemoved].
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
            FluxBytecodeVM.run(bytecode, signals, dev.flux.app.vm.FluxValue.NullVal, stringResolver, capabilities)
        if (result is VmResult.Failure) {
            mainDispatcher.dispatch(EmptyCoroutineContext) {
                onError?.invoke("lifecycle: ${result.kind.name} @${result.offset}")
            }
        } else {
            signals.flush()
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
}
