package dev.flux.host.vm

import dev.flux.host.vm.FluxValue.NullVal
import dev.flux.host.vm.FluxValue.RecordVal
import dev.flux.host.vm.FluxValue.StrVal
import dev.flux.host.vm.HttpRequestStore
import dev.flux.host.vm.VmErrorKind.TYPE_MISMATCH
import dev.flux.host.transport.HttpOkHttpTransport

/**
 * A host capability implementation invoked by the `CALL_CAP` opcode (Appendix E §E.1),
 * unified sync/async bridge (ADR-0045).
 *
 * [call] receives the call arguments and a mutable view of the live signal store so
 * it can create a result cell. It returns the **signal id** of that result cell —
 * never the value directly:
 * - a **synchronous** method writes `Ready(value)` into the cell and returns its id;
 * - an **asynchronous** method creates the cell (state `Pending`) and returns its id
 *   immediately; the host resolves it later via [SignalStore.resolveCell], which
 *   resumes any awaiting handler.
 *
 * One signature serves both shapes; the VM never branches on sync-vs-async.
 */
public fun interface CapabilityImpl {
    /** Evaluates the capability against [args] and [signals]; returns the result-cell id. */
    public fun call(
        args: FluxValue,
        signals: SignalStore,
    ): UInt
}

/**
 * Backing state for stateful capabilities (e.g. `Storage`), shared by every
 * impl registered in a registry. Kept separate from the signal graph so
 * capabilities can hold data the reactive tree does not (a persisted blob is
 * not a UI signal).
 *
 * `CapabilityStore` is now a thin named alias over [InMemoryStorageBackend] —
 * the injection seam the registry uses. Dev/test builds register an in-memory
 * store; the app shell registers a [FileStorageBackend] so `Storage.set`/`get`/
 * `delete` persist across process restarts (Task 1, LANE-C). Both conform to
 * [StorageBackend], so the impls never know which they are talking to.
 */
public typealias CapabilityStore = InMemoryStorageBackend

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
    private val store: StorageBackend = InMemoryStorageBackend(),
) {
    /** Looks up the impl for [capId]/[methodId], or `null` when unregistered. */
    public fun lookup(
        capId: UInt,
        methodId: UShort,
    ): CapabilityImpl? = handlers[CapabilityKey(capId, methodId)]

    public companion object {
        /** The oracle-faithful default table (no `Storage`/`Camera` state). */
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
                                } ?: throw VmError(TYPE_MISMATCH, 0u)
                            signals.write(99u, arg)
                            99u
                        },
                ),
            )

        /**
         * Builds the MLP capability set (G4):
         * - `Camera`  (cap 1): `takePicture` (1,1), `startPreview` (1,2), `stopPreview` (1,3).
         * - `Storage` (cap 2): `setItem` (2,1), `getItem` (2,2), `removeItem` (2,3).
         * - `Router`  (cap 3): `navigate` (3,1).
         * - `Clipboard` (cap 4): `setString` (4,1), `getString` (4,2).
         * - `Geolocation` (cap 5): `getCurrentPosition` (5,1).
         * - `Push` (cap 6): `registerForNotifications` (6,1) [async], `scheduleNotification` (6,2).
         * - `Biometric` (cap 7): `authenticate` (7,1).
         * - `Background` (cap 8): `schedule` (8,1) [async], cancel (8,2).
         * - `FileSystem` (cap 9): `readAsString` (9,1), `writeAsString` (9,2), `delete` (9,3).
         * - `DeepLink` (cap 10): `openURL` (10,1).
         * - `Sensors` (cap 11): `read` (11,1).
         *
         * `Storage` is backed by the injected [StorageBackend] (dev/test:
         * in-memory; app shell: [FileStorageBackend]) — see Task 1 (LANE-C).
         * `Camera.take` (1,1) preserves the oracle-parity echo of its first
         * argument into signal 99 so `flux-vm-ref`'s `call_cap_basic` vector
         * stays green. `startPreview`/`stopPreview` manage a preview flag
         * (signal 96) and are capture no-ops in headless builds. `Router.navigate`
         * (3,1) records the target string id in signal 97 (reconciler-driven).
         * `Clipboard`/`Geolocation` expose their synchronous result through
         * dedicated cells (94/93 and 92); the dev/test bodies use deterministic
         * in-memory echoes since the MLP dev host has no real pasteboard/location
         * (real OS access is a release-mode concern).
         *
         * @param backend the `Storage` persistence backend; defaults to an
         *   in-memory store (dev/test). Pass [FileStorageBackend] for a
         *   persist-to-disk registry.
         */
        public fun makeDev(
            backend: StorageBackend = InMemoryStorageBackend(),
            nativeHost: NativeCapabilityHost = DevNativeCapabilityHost(),
        ): CapabilityRegistry {
            val store = backend
            return CapabilityRegistry(
                LinkedHashMap<CapabilityKey, CapabilityImpl>().apply {
                    fun put(
                        capId: UInt,
                        methodId: UShort,
                        impl: CapabilityImpl,
                    ) {
                        this[CapabilityKey(capId, methodId)] = impl
                    }
                    // Camera.take (1,1): oracle-parity echo into signal 99; return its id.
                    // The dev-safe camera bridge (real capture behind CameraX
                    // ImageCapture) is intentionally NOT wired here so the oracle
                    // vector stays deterministic (Task 2, LANE-C): headless/JVM tests
                    // keep this echo; the app shell supplies real capture via a
                    // separate `CameraCapability` that still writes field 0 → 99.
                    put(1u, 1u.toUShort()) { args, signals ->
                        val arg =
                            when (val a = args) {
                                is RecordVal -> a.fields.firstOrNull()?.value
                                else -> null
                            } ?: throw VmError(TYPE_MISMATCH, 0u)
                        signals.write(99u, arg)
                        99u
                    }
                    // Camera.startPreview (1,2): record preview flag in signal 96; return its id.
                    put(1u, 2u.toUShort()) { _args, signals ->
                        signals.write(96u, FluxValue.BoolVal(true))
                        96u
                    }
                    // Camera.stopPreview (1,3): clear preview flag; return its id.
                    put(1u, 3u.toUShort()) { _args, signals ->
                        signals.write(96u, FluxValue.BoolVal(false))
                        96u
                    }
                    // Storage.set(key, value) (2,1): persist into the backend, expose via signal 95.
                    put(2u, 1u.toUShort()) { args, signals ->
                        val keyId =
                            when (val rec = args) {
                                is RecordVal ->
                                    rec.fields.firstOrNull()?.value?.let { first ->
                                        if (first is StrVal) first.id else null
                                    }
                                else -> null
                            } ?: throw VmError(TYPE_MISMATCH, 0u)
                        val value =
                            when (val rec = args) {
                                is RecordVal -> rec.fields.getOrNull(1)?.value
                                else -> null
                            } ?: throw VmError(TYPE_MISMATCH, 0u)
                        store.put(keyId, value)
                        signals.write(95u, value)
                        95u
                    }
                    // Storage.get(key) (2,2): read the persisted value, expose via signal 95.
                    put(2u, 2u.toUShort()) { args, signals ->
                        val keyId =
                            when (val rec = args) {
                                is RecordVal ->
                                    rec.fields.firstOrNull()?.value?.let { first ->
                                        if (first is StrVal) first.id else null
                                    }
                                else -> null
                            } ?: throw VmError(TYPE_MISMATCH, 0u)
                        val value = store.get(keyId) ?: FluxValue.NullVal
                        signals.write(95u, value)
                        95u
                    }
                    // Storage.delete(key) (2,3): clear the persisted value, expose `null` via signal 95.
                    put(2u, 3u.toUShort()) { args, signals ->
                        val keyId =
                            when (val rec = args) {
                                is RecordVal ->
                                    rec.fields.firstOrNull()?.value?.let { first ->
                                        if (first is StrVal) first.id else null
                                    }
                                else -> null
                            } ?: throw VmError(TYPE_MISMATCH, 0u)
                        store.put(keyId, null)
                        signals.write(95u, FluxValue.NullVal)
                        95u
                    }
                    // Router.navigate(target) (3,1): record target in signal 97; return its id.
                    put(3u, 1u.toUShort()) { args, signals ->
                        signals.write(97u, args)
                        97u
                    }
                    // Clipboard.set(value) (4,1): echo the value into signal 94 (the
                    // Clipboard result cell). The MLP dev host has no real pasteboard,
                    // so the dev body is a deterministic echo; release mode would
                    // forward to UIPasteboard / ClipboardManager.
                    put(4u, 1u.toUShort()) { args, signals ->
                        signals.write(94u, args)
                        94u
                    }
                    // Clipboard.get() (4,2): surface the last set value (signal 94) through
                    // signal 93; default to `null` when nothing was set.
                    put(4u, 2u.toUShort()) { _args, signals ->
                        val value = signals.read(94u) ?: FluxValue.NullVal
                        signals.write(93u, value)
                        93u
                    }
                    // Geolocation.get() (5,1): the MLP dev host has no real location
                    // provider, so surface a deterministic `null` (no fix available)
                    // through signal 92. Release mode would resolve CLLocationManager /
                    // FusedLocationProvider and write the coordinate here.
                    put(5u, 1u.toUShort()) { _args, signals ->
                        signals.write(92u, FluxValue.NullVal)
                        92u
                    }
                    // WebView (12): escape-valve native web content (FLUX-048).
                    // `load` (12,1) records the requested `src` into signal 82 so
                    // the UI kit can mount a sandboxed WebView; no OS permission
                    // required (PermissionKind::None).
                    put(12u, 1u.toUShort()) { args, signals ->
                        val src = (args as? FluxValue.RecordVal)?.fields?.firstOrNull()?.value ?: FluxValue.NullVal
                        signals.write(82u, src)
                        82u
                    }
                    // NativeModule (13): wraps an arbitrary native SDK through the
                    // `.native` escape hatch (FLUX-046). `invoke` (13,1) records the
                    // requested (name, method) into signal 83; gated by
                    // PermissionKind::NativeModule (the LANE-I allow-list), never an
                    // open CALL_NATIVE.
                    put(13u, 1u.toUShort()) { args, signals ->
                        val request = (args as? FluxValue.RecordVal)?.fields?.firstOrNull()?.value ?: FluxValue.NullVal
                        signals.write(83u, request)
                        83u
                    }
                    // Reference async capability (2,99): allocate a fresh Pending cell, return its id
                    // immediately (ADR-0045). The host resolves it later via SignalStore.resolveCell,
                    // resuming the awaiting handler. Mirrors the oracle's `async_deferred`.
                    put(2u, 99u.toUShort()) { _args, signals ->
                        val id = signals.allocateCell()
                        signals.markPending(id)
                        id
                    }
                    // --- FLUX-045: six concrete native capabilities (PRD-Q deferred set) ---
                    // ids 6..=11 are delegated to the injectable [NativeCapabilityHost]
                    // (defaults to [DevNativeCapabilityHost] for dev/test; the app shell
                    // supplies [AndroidNativeCapabilityHost] with real OS calls). This keeps
                    // the pure-JVM `:host` core free of `android.*` imports while the real
                    // device frameworks live in the app shell.
                    for (cap in 6u..11u) {
                        for (method in ushortRange(1u, 3u)) {
                            put(cap, method) { args, signals ->
                                nativeHost.call(cap, method, args, signals)
                            }
                        }
                    }
                    // --- FLUX-047: Http (14) async + Persist (15) sync host bodies. ---
                    // These reuse `store` (the same StorageBackend as `Storage`, since
                    // Persist is a queryable wrapper over it) and a per-registry
                    // HttpRequestStore + HttpOkHttpTransport so a real fetch resolves
                    // through the network. The closures are generated by
                    // [httpPersistEntries] to keep this table focused.
                    val httpStore = HttpRequestStore()
                    val httpTransport: HttpTransport = HttpOkHttpTransport()
                    for ((key, impl) in httpPersistEntries(httpStore, httpTransport, store)) {
                        this[key] = impl
                    }
                },
                store,
            )
        }

        /** Builds a `UShort` range `[start..end]` inclusive (Kotlin has no `UShortRange` literal). */
        private fun ushortRange(start: UInt, end: UInt): List<UShort> =
            (start..end).map { it.toUShort() }

        /**
         * FileSystem contents are persisted into the signal store under a
         * deterministic high signal id derived from the interned path id. The
         * 900_000 offset keeps these ids below the cell allocator's 1_000_000
         * ceiling (see [SignalGraph]) so they never collide with result cells.
         */
        private fun fileSignalId(pathId: UInt): UInt = 900_000u + pathId

        /** The MLP dev registry: `Storage` backed by an in-memory store. */
        public val DEV: CapabilityRegistry = makeDev()

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
