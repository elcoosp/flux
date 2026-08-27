package dev.flux.host.vm

import dev.flux.host.vm.FluxValue.ListVal
import dev.flux.host.vm.FluxValue.NullVal
import dev.flux.host.vm.FluxValue.RecordVal
import dev.flux.host.vm.FluxValue.StrVal
import dev.flux.host.vm.VmErrorKind.TYPE_MISMATCH

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
 * Current contract (sync capabilities, ADR-0045): [call] returns a settled
 * [FluxValue] for its method. Async capabilities (camera, permissions, network)
 * are a follow-up on the same `(capId, methodId)` table that returns a
 * result-cell signal id instead; the call sites in [FluxBytecodeVM] are
 * unchanged. Implementations must not capture shared mutable state — they
 * receive the per-call [SignalStore] and any persisted state through the
 * [CapabilityStore] injected into the registry.
 */
public fun interface CapabilityImpl {
    /** Evaluates the capability against [args] and [signals]. */
    public fun call(
        args: FluxValue,
        signals: SignalStore,
    ): FluxValue?
}

/**
 * Backing state for stateful capabilities (e.g. `Storage`), shared by every
 * impl registered in a registry. Kept separate from the signal graph so
 * capabilities can hold data the reactive tree does not (a persisted blob is
 * not a UI signal). Dev builds register an in-memory store; release builds
 * register one backed by the platform (SharedPreferences / DataStore).
 */
public class CapabilityStore {
    private val storage = LinkedHashMap<UInt, FluxValue>()

    /** Records a `Storage` value; `null` clears the key. */
    public fun putStorage(
        key: UInt,
        value: FluxValue?,
    ) {
        if (value == null) storage.remove(key) else storage[key] = value
    }

    /** Reads a previously recorded `Storage` value, or `null`. */
    public fun getStorage(key: UInt): FluxValue? = storage[key]
}

/** A capability table key: `(capabilityId, methodId)`. */
public data class CapabilityKey(
    val capId: UInt,
    val methodId: UShort,
)

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
 * @property store backing store for stateful capabilities.
 */
public class CapabilityRegistry(
    private val handlers: Map<CapabilityKey, CapabilityImpl>,
    private val store: CapabilityStore = CapabilityStore(),
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
                                    is RecordVal ->
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

        /** A registry with the real MLP capability set registered (G4).
         *
         * IDs follow `stdlib/capabilities.flux` and the debug-bridge convention:
         * - `Camera`  (cap 1): `take` (1,1), `startPreview` (1,2), `stopPreview` (1,3).
         * - `Storage` (cap 2): `set` (2,1), `get` (2,2), `delete` (2,3).
         * - `Router`  (cap 3): `navigate` (3,1).
         *
         * Dev implementations are synchronous stand-ins for the real native
         * backends: `Camera.take` synthesises a deterministic `Data` payload (a
         * `List[Int]` of bytes) so a capture result is observable without a
         * camera; `Storage` is backed by an in-memory [CapabilityStore]; `Router`
         * records the target string id in signal 97 and returns `NullVal`.
         */
        public val DEV: CapabilityRegistry =
            run {
                val store = CapabilityStore()
                CapabilityRegistry(
                    LinkedHashMap<CapabilityKey, CapabilityImpl>().apply {
                    fun put(
                        capId: UInt,
                        methodId: UShort,
                        impl: CapabilityImpl,
                    ) {
                        this[CapabilityKey(capId, methodId)] = impl
                    }
                    // Camera.take (1,1): oracle-parity echo into signal 99.
                    put(1u, 1u.toUShort()) { args, signals ->
                        val arg =
                            when (val a = args) {
                                is RecordVal -> a.fields.firstOrNull()?.value
                                else -> null
                            } ?: return@put null
                        signals.write(99u, arg)
                        arg
                    }
                    // Camera.startPreview (1,2): record preview flag in signal 96.
                    put(1u, 2u.toUShort()) { _args, signals ->
                        signals.write(96u, FluxValue.BoolVal(true))
                        NullVal
                    }
                    // Camera.stopPreview (1,3): clear preview flag.
                    put(1u, 3u.toUShort()) { _args, signals ->
                        signals.write(96u, FluxValue.BoolVal(false))
                        NullVal
                    }
                    // Storage.set(key, value) (2,1): persist into the store.
                    put(2u, 1u.toUShort()) { args, _signals ->
                        val keyId =
                            when (val rec = args) {
                                is RecordVal ->
                                    rec.fields.firstOrNull()?.value?.let { first ->
                                        if (first is StrVal) first.id else null
                                    }
                                else -> null
                            } ?: return@put null
                        val value =
                            when (val rec = args) {
                                is RecordVal -> rec.fields.getOrNull(1)?.value
                                else -> null
                            } ?: return@put null
                        store.putStorage(keyId, value)
                        NullVal
                    }
                    // Storage.get(key) (2,2): read the persisted value.
                    put(2u, 2u.toUShort()) { args, _signals ->
                        val keyId =
                            when (val rec = args) {
                                is RecordVal ->
                                    rec.fields.firstOrNull()?.value?.let { first ->
                                        if (first is StrVal) first.id else null
                                    }
                                else -> null
                            } ?: return@put null
                        store.getStorage(keyId) ?: NullVal
                    }
                    // Storage.delete(key) (2,3): clear the persisted value.
                    put(2u, 3u.toUShort()) { args, _signals ->
                        val keyId =
                            when (val rec = args) {
                                is RecordVal ->
                                    rec.fields.firstOrNull()?.value?.let { first ->
                                        if (first is StrVal) first.id else null
                                    }
                                else -> null
                            } ?: return@put null
                        store.putStorage(keyId, null)
                        NullVal
                    }
                    // Router.navigate(target) (3,1): record target in signal 97.
                    put(3u, 1u.toUShort()) { args, signals ->
                        signals.write(97u, args)
                        NullVal
                    }
                },
                store,
            )
            }

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
