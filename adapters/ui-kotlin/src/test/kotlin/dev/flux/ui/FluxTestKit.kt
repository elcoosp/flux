package dev.flux.ui

/**
 * Test double for [FluxExecutor]. Records every [HandlerEvent] dispatched so
 * adapter tests can assert that taps and text changes reach the VM boundary
 * without a real runtime. Safe to dispatch against after disposal (it simply
 * records nothing more).
 */
public class FluxExecutorFake : FluxExecutor {
    /** All events received since construction (or the last [clear]). */
    public val events: MutableList<HandlerEvent> = mutableListOf()

    /** True once [dispose] has been called. */
    public var disposed: Boolean = false
        private set

    override fun dispatch(event: HandlerEvent) {
        if (disposed) return
        events.add(event)
    }

    /** Marks this fake disposed; subsequent dispatches are ignored. */
    public fun dispose() {
        disposed = true
    }

    /** Removes all recorded events. */
    public fun clear() {
        events.clear()
    }
}

/**
 * Builds a [Props] from pairs of (index, [FluxValue]) for use in tests, keeping
 * test setup terse while remaining explicit about which indices are set.
 */
public fun propsOf(vararg fields: Pair<UShort, FluxValue>): Props = Props(fields.map { (index, value) -> Props.Field(index, value) })

/** A [Props] carrying a single string field at [index]. */
public fun stringProps(
    index: UShort,
    value: String,
): Props = propsOf(index to FluxValue.Str(value))
