package dev.flux.host.vm

/**
 * A host capability implementation invoked by the `CALL_CAP` opcode (Appendix E §E.1).
 *
 * A [call] receives the resolved call arguments and returns the value the VM
 * should place in the instruction's result register. Capabilities model
 * side-effecting or platform-only operations (camera, storage, router
 * navigation, …) that the pure bytecode VM cannot perform itself. The same
 * [CapabilityImpl] signature is used by dev mode (which forwards to the dev
 * server over WebSocket RPC) and release mode (which calls a native impl);
 * only the routing table differs.
 *
 * The [call] receives the [SignalStore] so capabilities may perform the writes
 * the `flux-vm-ref` oracle's built-in capabilities do (e.g. the default
 * `(1,1)` capability writes `arg[0]` into signal 99 — see [CapabilityRegistry.default]).
 *
 * @property capId the capability id this impl serves.
 * @property methodId the method id within the capability this impl serves.
 */
public data class CapabilityKey(
    val capId: UInt,
    val methodId: UShort,
)

/**
 * A capability implementation: given a call's argument value and the live
 * signal store, produces the result value written into the VM's result
 * register.
 *
 * Returning `null` signals "not applicable" and lets the dispatcher fall
 * through to a `TYPE_MISMATCH` fault.
 */
public fun interface CapabilityImpl {
    /** Evaluates the capability against [args] and [signals]. */
    public fun call(
        args: FluxValue,
        signals: SignalStore,
    ): FluxValue?
}

/**
 * The `(capId, methodId) → impl` registry injected into [FluxBytecodeVM] for
 * the `CALL_CAP` opcode (Appendix E §E.1, spec task G4).
 *
 * The MLP host replaces the previous hardcoded `if (capId == 1u && methodId ==
 * 1u)` test with a data-driven lookup: every capability — dev-mode RPC
 * forwarders and release-mode native impls alike — is registered here as a
 * `(capId, methodId)` key. An unregistered key is a `TYPE_MISMATCH` fault,
 * matching the oracle's contract that a capability must exist to be called.
 *
 * @property handlers the capability table.
 */
public class CapabilityRegistry(
    private val handlers: Map<CapabilityKey, CapabilityImpl>,
) {
    /** Looks up the impl for [capId]/[methodId], or `null` when unregistered. */
    public fun lookup(
        capId: UInt,
        methodId: UShort,
    ): CapabilityImpl? = handlers[CapabilityKey(capId, methodId)]

    public companion object {
        /**
         * The oracle-faithful default table. Contains the `flux-vm-ref`
         * built-in `(1,1)` capability — it writes `arg[0]` into signal 99 and
         * returns `arg[0]` — so the golden `call_cap_basic` vector stays green
         * without a live dev server. Tests and release code extend or replace
         * this with the full capability set.
         */
        public fun default(): CapabilityRegistry =
            fromEntries(
                listOf(
                    CapabilityKey(1u, 1u.toUShort()) to
                        CapabilityImpl { args, signals ->
                            val arg =
                                when (val a = args) {
                                    is FluxValue.RecordVal ->
                                        if (a.fields.isEmpty()) {
                                            null
                                        } else {
                                            a.fields[0].value
                                        }
                                    else -> null
                                } ?: return@CapabilityImpl null
                            signals.write(99u, arg)
                            arg
                        },
                ),
            )

        /** An empty registry: every `CALL_CAP` faults as `TYPE_MISMATCH`. */
        public val EMPTY: CapabilityRegistry = CapabilityRegistry(emptyMap())

        /**
         * Builds a registry from [entries], keyed by `(capId, methodId)`.
         *
         * @param entries the capability bindings; duplicate keys are last-wins.
         */
        public fun fromEntries(entries: Iterable<Pair<CapabilityKey, CapabilityImpl>>): CapabilityRegistry {
            val map = LinkedHashMap<CapabilityKey, CapabilityImpl>()
            for ((key, impl) in entries) map[key] = impl
            return CapabilityRegistry(map)
        }
    }
}
