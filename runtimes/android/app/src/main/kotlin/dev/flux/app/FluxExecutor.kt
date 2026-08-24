package dev.flux.app

import dev.flux.app.shadow.ShadowTree
import dev.flux.app.signal.SignalGraph
import dev.flux.app.transport.FluxTransport
import dev.flux.app.vm.FluxBytecodeVM
import dev.flux.app.vm.VmResult
import dev.flux.app.wire.FrameDeserializer
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
        val payload = event.payload?.toVmValue() ?: dev.flux.app.vm.FluxValue.NullVal
        dispatch(event.handlerId, payload)
    }

    /** Runs the closure [handlerId] with [payload] in the VM, then flushes signals. */
    public fun dispatch(
        handlerId: UInt,
        payload: dev.flux.app.vm.FluxValue = dev.flux.app.vm.FluxValue.NullVal,
    ) {
        vmScope.launch {
            val closure = closureFor(handlerId) ?: return@launch
            val result = FluxBytecodeVM.run(closure.bytecode, signals, payload)
            when (result) {
                is VmResult.Success -> signals.flush()
                is VmResult.Failure -> {
                    val msg = "vm: ${result.kind.name} @${result.offset}"
                    withContext(mainDispatcher) { onError?.invoke(msg) }
                }
            }
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

    /** A registered handler closure: bytecode + captured signals. */
    public data class Closure(
        val bytecode: ByteArray,
    )

    /** Tears down the executor and transport. */
    public fun dispose() {
        transport.close()
    }
}
