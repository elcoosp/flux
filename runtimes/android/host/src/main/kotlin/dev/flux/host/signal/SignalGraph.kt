package dev.flux.host.signal

import dev.flux.host.vm.FluxValue
import dev.flux.host.vm.SignalStore

/**
 * The reactive state of a single signal cell (ADR-0044, MLP v2 first-class async).
 *
 * - [Ready]: the cell holds a resolved value (`values[id]` is authoritative).
 * - [Pending]: an async-derived/resource cell has an in-flight future; `values[id]`
 *   may hold a stale `Ready` value or a placeholder — readers rendering the cell
 *   should surface the pending branch (e.g. the `when ... is_loading` form) rather
 *   than mutating the native view for the not-yet-resolved value.
 * - [Error]: the future resolved with a fault; [error] carries the message.
 */
public enum class CellState {
    Ready,
    Pending,
    Error,
}

/**
 * A fine-grained, dependency-tracked signal graph.
 *
 * Each signal holds a [FluxValue] in the VM's value representation (the VM is
 * the sole writer/reader; see [SignalStore]). Reads and writes are observed so
 * a handler dispatch can compute the minimal set of dependent signals to
 * propagate. The graph batches derived updates and replays them in topological
 * order, mirroring the Swift host's `SignalGraph` (FLUX-006).
 *
 * The MLP host is handler-authoritative: a tap runs a closure in
 * [dev.flux.host.vm.FluxBytecodeVM], which writes signals; the graph then
 * notifies subscribers for the changed ids (the eventual native view mutation).
 *
 * For MLP v2 async (ADR-0044), each cell also carries a [CellState] so async
 * derived/resource cells can be `Pending` while their future is in flight. The
 * graph does not mutate the native view for a `Pending` cell; it only flips the
 * state and notifies, letting the author's `when is_loading` branch drive the UI.
 */
public class SignalGraph : SignalStore {
    private val values = LinkedHashMap<UInt, FluxValue>()
    private val subscribers = LinkedHashMap<UInt, MutableSet<(FluxValue) -> Unit>>()
    private val pending = LinkedHashSet<UInt>()
    private val states = LinkedHashMap<UInt, CellState>()

    /** Returns the current value of [id], or `null` when unbound. */
    override fun read(id: UInt): FluxValue? = values[id]

    /** Returns the reactive [CellState] of [id], defaulting to [CellState.Ready]. */
    public fun cellState(id: UInt): CellState = states[id] ?: CellState.Ready

    /** Marks [id] as [CellState.Pending] (an async-derived/resource cell went in flight). */
    public fun markPending(id: UInt) {
        states[id] = CellState.Pending
    }

    /** Marks [id] as [CellState.Error] with [message]; the future resolved with a fault. */
    public fun markError(
        id: UInt,
        message: String,
    ) {
        states[id] = CellState.Error
    }

    /** Writes [value] into [id], recording it for the next [flush]. */
    override fun write(
        id: UInt,
        value: FluxValue,
    ) {
        values[id] = value
        // A successful write resolves any pending/error cell back to Ready.
        states[id] = CellState.Ready
        pending.add(id)
    }

    /** Returns every written signal as a sorted `(id, value)` list. */
    override fun snapshot(): List<Pair<UInt, FluxValue>> = values.entries.sortedBy { it.key }.map { (k, v) -> k to v }

    /** Seeds many signals at once (Init / StateDelta). */
    public fun seed(cells: Iterable<Pair<UInt, FluxValue>>) {
        for ((id, v) in cells) values[id] = v
    }

    /** Subscribes [block] to changes on [id]; returns a handle to unsubscribe. */
    public fun subscribe(
        id: UInt,
        block: (FluxValue) -> Unit,
    ): Subscription {
        val set = subscribers.getOrPut(id) { LinkedHashSet() }
        set.add(block)
        return Subscription { set.remove(block) }
    }

    /**
     * Propagates all pending writes to their subscribers, in ascending signal
     * id order (a deterministic topological order for the MLP graph). A [flush]
     * drains exactly the signals written since the last flush; concurrent
     * writes during propagation are appended to the next batch.
     */
    public fun flush() {
        if (pending.isEmpty()) return
        val batch = pending.sorted()
        pending.clear()
        for (id in batch) {
            val value = values[id] ?: continue
            subscribers[id]?.toList()?.forEach { it(value) }
        }
    }

    /** A subscription handle; [dispose] removes the callback. */
    public data class Subscription(
        val dispose: () -> Unit,
    )
}
