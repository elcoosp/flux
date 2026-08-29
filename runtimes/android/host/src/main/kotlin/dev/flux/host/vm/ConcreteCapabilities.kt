package dev.flux.host.vm

import dev.flux.host.vm.FluxValue.BoolVal
import dev.flux.host.vm.FluxValue.NullVal
import dev.flux.host.vm.FluxValue.RecordVal
import dev.flux.host.vm.FluxValue.StrVal
import dev.flux.host.vm.VmErrorKind.TYPE_MISMATCH

/**
 * FLUX-045 (LANE-C, Phase 6) — native bodies for the six concrete capabilities
 * (Push / Biometric / Background / FileSystem / DeepLink / Sensors), ids 6..=11.
 *
 * ADR-0049 naming is already applied in this module (FluxValue / VmError /
 * FluxExecutor / StringResolver / FluxFrame / Opcode); this file only ADDS the
 * six concrete capability impls and a factory that composes them with the MLP
 * dev set (1..=5 + 12/13 escape hatches + async ref 2,99) in CapabilityRegistry.
 *
 * This file is intentionally NEW (not an edit to CapabilityRegistry.kt) so it
 * slots in alongside the in-flight registry work without colliding. It builds a
 * full registry via [CapabilityRegistry.fromEntries].
 *
 * Contract (ADR-0044/0045): a denied grant yields a typed [VmError], never a
 * crash; async capabilities (Push.register, Background.schedule) allocate a
 * Pending result cell and return its id immediately, resolving it via
 * [SignalStore.resolveCell]. The dev host has no real OS providers, so the
 * bodies below are deterministic dev-safe echoes; real OS calls are flagged
 * RELEASE-TODO and belong in the app shell.
 */

/**
 * The six concrete capabilities (FLUX-045) as `(capId, methodId, impl)` triples
 * keyed by deterministic ids matching `stdlib/capabilities.flux` and
 * `CAPABILITY_IDL` (crates/flux-types/src/capabilities.rs):
 * - Push (6):       register(6,1) [async], getToken(6,2)
 * - Biometric (7):  authenticate(7,1)
 * - Background (8): schedule(8,1) [async], cancel(8,2)
 * - FileSystem (9): read(9,1), write(9,2), delete(9,3)
 * - DeepLink (10):  open(10,1)
 * - Sensors (11):   read(11,1)
 *
 * Each impl returns the **signal id** of its result cell. Synchronous ones write
 * the value and return the id; the two async ones allocate a Pending cell, return
 * its id, and resolve it inline with a deterministic dev value.
 */
public fun concreteCapabilityEntries(): List<Pair<CapabilityKey, CapabilityImpl>> {
    val entries = mutableListOf<Pair<CapabilityKey, CapabilityImpl>>()

    // MARK: Push (cap 6)
    // Push.register — async. RELEASE-TODO: NotificationCenter / APNs token fetch.
    entries += CapabilityKey(6u, 1u.toUShort()) to CapabilityImpl { _args, signals ->
        val id = signals.allocateCell()
        signals.markPending(id)
        signals.resolveCell(id, StrVal(42u)) // simulated device-token id (dev)
        id
    }
    // Push.getToken — surface the last simulated token (signal 42) or null.
    entries += CapabilityKey(6u, 2u.toUShort()) to CapabilityImpl { _args, signals ->
        val id = signals.allocateCell()
        signals.write(id, signals.read(42u) ?: NullVal)
        id
    }

    // MARK: Biometric (cap 7)
    // Biometric.authenticate — synchronous dev echo. RELEASE-TODO:
    // BiometricPrompt / LocalAuthentication. A denied grant MUST yield a typed
    // error, never a crash.
    entries += CapabilityKey(7u, 1u.toUShort()) to CapabilityImpl { _args, signals ->
        val id = signals.allocateCell()
        signals.write(id, BoolVal(true)) // dev: assume granted
        id
    }

    // MARK: Background (cap 8)
    // Background.schedule — async. RELEASE-TODO: WorkManager / BGTaskScheduler.
    entries += CapabilityKey(8u, 1u.toUShort()) to CapabilityImpl { _args, signals ->
        val id = signals.allocateCell()
        signals.markPending(id)
        signals.resolveCell(id, StrVal(43u)) // simulated task id (dev)
        id
    }
    entries += CapabilityKey(8u, 2u.toUShort()) to CapabilityImpl { _args, signals ->
        val id = signals.allocateCell()
        signals.write(id, BoolVal(true))
        id
    }

    // MARK: FileSystem (cap 9)
    // FileSystem.read(path) — contents persisted under a derived signal id.
    entries += CapabilityKey(9u, 1u.toUShort()) to CapabilityImpl { args, signals ->
        val pathId = firstStrId(args) ?: throw VmError(TYPE_MISMATCH, 0u)
        val id = signals.allocateCell()
        signals.write(id, signals.read(fileSignalId(pathId)) ?: NullVal)
        id
    }
    // FileSystem.write(path, data) — persist into the signal store.
    // RELEASE-TODO: context.filesDir / FileManager document write.
    entries += CapabilityKey(9u, 2u.toUShort()) to CapabilityImpl { args, signals ->
        val rec = args as? RecordVal ?: throw VmError(TYPE_MISMATCH, 0u)
        val pathId = (rec.fields.firstOrNull()?.value as? StrVal)?.id
            ?: throw VmError(TYPE_MISMATCH, 0u)
        val data = rec.fields.getOrNull(1)?.value ?: throw VmError(TYPE_MISMATCH, 0u)
        signals.write(fileSignalId(pathId), data)
        val id = signals.allocateCell()
        signals.write(id, data)
        id
    }
    // FileSystem.delete(path) — clear the persisted value.
    entries += CapabilityKey(9u, 3u.toUShort()) to CapabilityImpl { args, signals ->
        val pathId = firstStrId(args) ?: throw VmError(TYPE_MISMATCH, 0u)
        signals.write(fileSignalId(pathId), NullVal)
        val id = signals.allocateCell()
        signals.write(id, NullVal)
        id
    }

    // MARK: DeepLink (cap 10)
    // DeepLink.open(url) — record the target (signal 44) for the reconciler.
    // RELEASE-TODO: startActivity / UIApplication.open.
    entries += CapabilityKey(10u, 1u.toUShort()) to CapabilityImpl { args, signals ->
        signals.write(44u, args)
        44u
    }

    // MARK: Sensors (cap 11)
    // Sensors.read — dev returns an empty record; RELEASE-TODO:
    // SensorManager / CMMotionManager sampling.
    entries += CapabilityKey(11u, 1u.toUShort()) to CapabilityImpl { _args, signals ->
        val id = signals.allocateCell()
        signals.write(id, RecordVal(emptyList()))
        id
    }

    return entries
}

/** Reads the first `StrVal` id from a record argument, or null. */
private fun firstStrId(args: FluxValue): UInt? =
    (args as? RecordVal)?.fields?.firstOrNull()?.value?.let { if (it is StrVal) it.id else null }

/** FileSystem contents persisted under a deterministic high signal id. */
private fun fileSignalId(pathId: UInt): UInt = 900_000u + pathId

/**
 * A production registry composing the MLP dev set (1..=5 + 12/13 escape hatches
 * + async ref 2,99) with the six concrete capabilities (6..=11). Used by the app
 * shell; the dev/test harness continues to use [CapabilityRegistry.makeDev].
 *
 * `CapabilityRegistry`'s entry table is private, so the dev rows are reproduced
 * here; keep in sync with `CapabilityRegistry.makeDev` if the MLP set changes.
 */
public fun CapabilityRegistry.Companion.makeProduction(
    backend: StorageBackend = InMemoryStorageBackend(),
): CapabilityRegistry {
    val dev: List<Pair<CapabilityKey, CapabilityImpl>> = listOf(
        CapabilityKey(1u, 1u.toUShort()) to CapabilityImpl { args, signals ->
            val arg = (args as? RecordVal)?.fields?.firstOrNull()?.value
                ?: throw VmError(TYPE_MISMATCH, 0u)
            signals.write(99u, arg); 99u
        },
        CapabilityKey(1u, 2u.toUShort()) to CapabilityImpl { _args, signals ->
            signals.write(96u, BoolVal(true)); 96u
        },
        CapabilityKey(1u, 3u.toUShort()) to CapabilityImpl { _args, signals ->
            signals.write(96u, BoolVal(false)); 96u
        },
        CapabilityKey(2u, 1u.toUShort()) to CapabilityImpl { args, signals ->
            val keyId = firstStrId(args) ?: throw VmError(TYPE_MISMATCH, 0u)
            val value = (args as? RecordVal)?.fields?.getOrNull(1)?.value
                ?: throw VmError(TYPE_MISMATCH, 0u)
            signals.write(95u, value); 95u
        },
        CapabilityKey(2u, 2u.toUShort()) to CapabilityImpl { args, signals ->
            val keyId = firstStrId(args) ?: throw VmError(TYPE_MISMATCH, 0u)
            signals.write(95u, signals.read(95u) ?: NullVal); 95u
        },
        CapabilityKey(2u, 3u.toUShort()) to CapabilityImpl { args, signals ->
            val keyId = firstStrId(args) ?: throw VmError(TYPE_MISMATCH, 0u)
            signals.write(95u, NullVal); 95u
        },
        CapabilityKey(3u, 1u.toUShort()) to CapabilityImpl { args, signals ->
            signals.write(97u, args); 97u
        },
        CapabilityKey(4u, 1u.toUShort()) to CapabilityImpl { args, signals ->
            signals.write(94u, args); 94u
        },
        CapabilityKey(4u, 2u.toUShort()) to CapabilityImpl { _args, signals ->
            signals.write(93u, signals.read(94u) ?: NullVal); 93u
        },
        CapabilityKey(5u, 1u.toUShort()) to CapabilityImpl { _args, signals ->
            signals.write(92u, NullVal); 92u
        },
        CapabilityKey(12u, 1u.toUShort()) to CapabilityImpl { args, signals ->
            val src = (args as? RecordVal)?.fields?.firstOrNull()?.value ?: NullVal
            signals.write(82u, src); 82u
        },
        CapabilityKey(13u, 1u.toUShort()) to CapabilityImpl { args, signals ->
            val request = (args as? RecordVal)?.fields?.firstOrNull()?.value ?: NullVal
            signals.write(83u, request); 83u
        },
        CapabilityKey(2u, 99u.toUShort()) to CapabilityImpl { _args, signals ->
            val id = signals.allocateCell(); signals.markPending(id); id
        },
    )
    return CapabilityRegistry.fromEntries(dev + concreteCapabilityEntries())
}
