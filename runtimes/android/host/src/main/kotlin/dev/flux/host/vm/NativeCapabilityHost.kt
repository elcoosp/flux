package dev.flux.host.vm

import dev.flux.host.vm.FluxValue.BoolVal
import dev.flux.host.vm.FluxValue.NullVal
import dev.flux.host.vm.FluxValue.RecordVal
import dev.flux.host.vm.FluxValue.StrVal
import dev.flux.host.vm.VmErrorKind.TYPE_MISMATCH

/**
 * The injectable seam between the host capability registry and the real device
 * OS. The precompiled host core (this `:host` module) is pure-JVM so its unit
 * tests run without an emulator; it therefore contains NO real OS calls. The
 * concrete capabilities (FLUX-045: Push / Biometric / Background / FileSystem /
 * DeepLink / Sensors, ids 6..=11) forward their `CALL_CAP` through this seam.
 *
 * - [DevNativeCapabilityHost] (the default) provides deterministic dev-safe
 *   echoes so the dev host and the headless JVM test-suite behave identically
 *   with zero OS dependencies.
 * - The app shell (`runtimes/android/app`) supplies a real implementation
 *   ([AndroidNativeCapabilityHost]) that calls the actual Android frameworks
 *   (NotificationManager / BiometricPrompt / WorkManager / FileManager /
 *   startActivity / SensorManager) behind this same interface.
 *
 * The contract matches [CapabilityImpl] exactly: the implementation receives the
 * call arguments and a mutable view of the live signal store, performs the work
 * (possibly asynchronously — allocating a `Pending` cell and resolving it later
 * via [SignalStore.resolveCell], per ADR-0045), and returns the **result-cell
 * signal id**. A denied grant must throw a typed [VmError] (ADR-0057), never
 * crash.
 */
public interface NativeCapabilityHost {
    /** True when this host provides a real (non-dev) body for `(capId, methodId)`. */
    public fun handles(capId: UInt, methodId: UShort): Boolean

    /**
     * Runs the capability body for `(capId, methodId)` against [args] and [signals]
     * and returns the result-cell signal id. Only invoked when [handles] is true.
     */
    public fun call(capId: UInt, methodId: UShort, args: FluxValue, signals: SignalStore): UInt
}

/**
 * The default [NativeCapabilityHost]: deterministic dev-safe echoes that need no
 * real OS provider. Mirrors the FLUX-045 wire contract (signal ids 42/43/44 and
 * the 900_000-derived FileSystem id) so the dev handshake and the JVM
 * round-trip tests stay green without an emulator.
 *
 * Real OS behavior lives in `AndroidNativeCapabilityHost` (app shell).
 */
public class DevNativeCapabilityHost : NativeCapabilityHost {
    override fun handles(capId: UInt, methodId: UShort): Boolean =
        capId in 6u..11u

    override fun call(capId: UInt, methodId: UShort, args: FluxValue, signals: SignalStore): UInt {
        return when (capId) {
            6u -> when (methodId) {
                // Push.register (6,1) [async]: allocate a Pending cell, resolve inline
                // with a simulated device-token id (signal 42).
                1u.toUShort() -> {
                    val id = signals.allocateCell()
                    signals.markPending(id)
                    signals.resolveCell(id, StrVal(42u))
                    id
                }
                // Push.getToken (6,2): surface the last simulated token (signal 42) or null.
                2u.toUShort() -> {
                    val id = signals.allocateCell()
                    signals.write(id, signals.read(42u) ?: NullVal)
                    id
                }
                else -> throw VmError(TYPE_MISMATCH, 0u)
            }
            7u -> when (methodId) {
                // Biometric.authenticate (7,1): dev assumes granted; a denied grant MUST
                // yield a typed VmError (CAPABILITY_DENIED), never a crash.
                1u.toUShort() -> {
                    val id = signals.allocateCell()
                    signals.write(id, BoolVal(true))
                    id
                }
                else -> throw VmError(TYPE_MISMATCH, 0u)
            }
            8u -> when (methodId) {
                // Background.schedule (8,1) [async]: allocate a Pending cell, resolve inline
                // with a simulated task id (signal 43).
                1u.toUShort() -> {
                    val id = signals.allocateCell()
                    signals.markPending(id)
                    signals.resolveCell(id, StrVal(43u))
                    id
                }
                // Background.cancel (8,2): dev-safe echo.
                2u.toUShort() -> {
                    val id = signals.allocateCell()
                    signals.write(id, BoolVal(true))
                    id
                }
                else -> throw VmError(TYPE_MISMATCH, 0u)
            }
            9u -> when (methodId) {
                // FileSystem.read (9,1): contents persisted under a derived signal id.
                1u.toUShort() -> {
                    val pathId = firstStrId(args) ?: throw VmError(TYPE_MISMATCH, 0u)
                    val id = signals.allocateCell()
                    signals.write(id, signals.read(fileSignalId(pathId)) ?: NullVal)
                    id
                }
                // FileSystem.write (9,2): persist into the signal store.
                2u.toUShort() -> {
                    val rec = args as? RecordVal ?: throw VmError(TYPE_MISMATCH, 0u)
                    val pathId =
                        (rec.fields.firstOrNull()?.value as? StrVal)?.id
                            ?: throw VmError(TYPE_MISMATCH, 0u)
                    val data = rec.fields.getOrNull(1)?.value ?: throw VmError(TYPE_MISMATCH, 0u)
                    signals.write(fileSignalId(pathId), data)
                    val id = signals.allocateCell()
                    signals.write(id, data)
                    id
                }
                // FileSystem.delete (9,3): clear the persisted value.
                3u.toUShort() -> {
                    val pathId = firstStrId(args) ?: throw VmError(TYPE_MISMATCH, 0u)
                    signals.write(fileSignalId(pathId), NullVal)
                    val id = signals.allocateCell()
                    signals.write(id, NullVal)
                    id
                }
                else -> throw VmError(TYPE_MISMATCH, 0u)
            }
            10u -> when (methodId) {
                // DeepLink.openURL (10,1): record the target (signal 44) for the reconciler.
                1u.toUShort() -> {
                    signals.write(44u, args)
                    44u
                }
                else -> throw VmError(TYPE_MISMATCH, 0u)
            }
            11u -> when (methodId) {
                // Sensors.read (11,1): dev returns an empty record.
                1u.toUShort() -> {
                    val id = signals.allocateCell()
                    signals.write(id, RecordVal(emptyList()))
                    id
                }
                else -> throw VmError(TYPE_MISMATCH, 0u)
            }
            else -> throw VmError(TYPE_MISMATCH, 0u)
        }
    }

    /** Reads the first `StrVal` id from a record argument, or null. */
    private fun firstStrId(args: FluxValue): UInt? =
        (args as? RecordVal)?.fields?.firstOrNull()?.value?.let { if (it is StrVal) it.id else null }

    /** FileSystem contents persisted under a deterministic high signal id. */
    private fun fileSignalId(pathId: UInt): UInt = 900_000u + pathId
}
