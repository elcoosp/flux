package dev.flux.host.vm

import dev.flux.host.signal.CellState

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
     * Allocates a fresh, unbound signal id for a new capability result cell
     * (ADR-0045). Drawn from a high ceiling so it never collides with the low,
     * fixed ids handlers and golden vectors use (e.g. 99).
     */
    public fun allocateCell(): UInt

    /** Returns the reactive [CellState] of [id], defaulting to [CellState.Ready]. */
    public fun cellState(id: UInt): CellState

    /** Marks [id] as [CellState.Pending] (an async capability has started). */
    public fun markPending(id: UInt)

    /** Resolves [id] to [value], marking it [CellState.Ready] (async finished). */
    public fun resolveCell(
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
    private val cellStates: MutableMap<UInt, CellState> = LinkedHashMap()
    private var nextCell: UInt = 1_000_000u

    override fun read(id: UInt): FluxValue? = cells[id]

    override fun write(
        id: UInt,
        value: FluxValue,
    ) {
        cells[id] = value
        cellStates[id] = CellState.Ready
    }

    override fun allocateCell(): UInt {
        nextCell += 1u
        return nextCell
    }

    override fun cellState(id: UInt): CellState = cellStates[id] ?: CellState.Ready

    override fun markPending(id: UInt) {
        cellStates[id] = CellState.Pending
    }

    override fun resolveCell(
        id: UInt,
        value: FluxValue,
    ) {
        cells[id] = value
        cellStates[id] = CellState.Ready
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
