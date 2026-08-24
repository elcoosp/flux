package dev.flux.ui

/**
 * The event an adapter hands back to the host when a native interaction fires.
 *
 * Adapters never evaluate handler bytecode themselves. On a tap, text change,
 * or any bound event they construct a [HandlerEvent] and call
 * [FluxExecutor.dispatch], which the host executor routes to the embedded VM.
 * This keeps the adapter layer free of VM knowledge and matches the Swift
 * contract where adapters call `executor.dispatch(handlerId)`.
 *
 * @property handlerId The closure-table index of the handler to run.
 * @property payload Optional event data (e.g. the new string for a text field).
 */
public data class HandlerEvent(
    val handlerId: UInt,
    val payload: FluxValue? = null,
)

/**
 * The narrow slice of the host executor an adapter kit is allowed to touch.
 *
 * This interface is the boundary between the adapter layer (FLUX-009) and the
 * host runtime (FLUX-007). Adapters hold it through a [WeakReference] so the
 * executor can be torn down without leaking the whole shadow tree. The
 * production runtime supplies a real implementation; tests supply a fake (see
 * `FluxExecutorFake`).
 */
public interface FluxExecutor {
    /**
     * Dispatches [event] to the VM for evaluation.
     *
     * Must be safe to call after the executor has been disposed; the
     * implementation should ignore dispatch requests rather than throw. The
     * adapter does not await the result — handler side effects propagate back
     * through subsequent [Props] updates.
     */
    fun dispatch(event: HandlerEvent)
}
