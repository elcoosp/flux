package dev.flux.host.vm

/**
 * The signal graph a handler reads from and writes to.
 *
 * Implementations back the fine-grained reactivity layer ([SignalGraph]); the
 * [FluxBytecodeVM] only depends on this interface so tests can supply a
 * deterministic store.
 */
public interface SignalStore {
    /** Returns the current value of [id], or `null` if unbound. */
    public fun read(id: UInt): FluxValue?

    /** Writes [value] into [id]. */
    public fun write(
        id: UInt,
        value: FluxValue,
    )

    /**
     * Returns every written signal as a sorted `(id, value)` list.
     *
     * The VM uses this to populate the outcome snapshot; the oracle needs a
     * total snapshot, not a diff, so the Kotlin runtime can compare final state
     * against the golden vectors.
     */
    public fun snapshot(): List<Pair<UInt, FluxValue>>
}

/**
 * In-memory [SignalStore] used by tests and the dev server.
 *
 * @property cells the signal cells, keyed by id.
 */
public class InMemorySignals(
    private val cells: MutableMap<UInt, FluxValue> = LinkedHashMap(),
) : SignalStore {
    override fun read(id: UInt): FluxValue? = cells[id]

    override fun write(
        id: UInt,
        value: FluxValue,
    ) {
        cells[id] = value
    }

    override fun snapshot(): List<Pair<UInt, FluxValue>> = cells.entries.sortedBy { it.key }.map { (k, v) -> k to v }

    public companion object {
        /** Builds a store from an iterator of `(id, value)` pairs. */
        public fun fromSignals(signals: Iterable<Pair<UInt, FluxValue>>): InMemorySignals {
            val map = LinkedHashMap<UInt, FluxValue>()
            for ((id, v) in signals) map[id] = v
            return InMemorySignals(map)
        }
    }
}
