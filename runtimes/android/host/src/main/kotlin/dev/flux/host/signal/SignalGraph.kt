package dev.flux.host.signal

import dev.flux.host.vm.FluxValue
import dev.flux.host.vm.SignalStore

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
 */
public class SignalGraph : SignalStore {
    private val values = LinkedHashMap<UInt, FluxValue>()
    private val subscribers = LinkedHashMap<UInt, MutableSet<(FluxValue) -> Unit>>()
    private val pending = LinkedHashSet<UInt>()

    /** Returns the current value of [id], or `null` when unbound. */
    override fun read(id: UInt): FluxValue? = values[id]

    /** Writes [value] into [id], recording it for the next [flush]. */
    override fun write(
        id: UInt,
        value: FluxValue,
    ) {
        values[id] = value
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
