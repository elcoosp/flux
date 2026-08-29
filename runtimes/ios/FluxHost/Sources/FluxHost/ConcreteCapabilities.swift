//  ConcreteCapabilities.swift
//  FLUX-045 (LANE-C, Phase 6) — native bodies for the six concrete capabilities
//  (Push / Biometric / Background / FileSystem / DeepLink / Sensors), ids 6..=11.
//
//  ADR-0049 naming is already applied in this module (FluxValue / VmError /
//  FluxExecutor / StringResolver / FluxFrame / Opcode); this file only ADDS the
//  six concrete capability impls and a factory that composes them with the MLP
//  dev set (1..=5 + async ref 2,99) already present in Registry.swift.
//
//  This file is intentionally NEW (not an edit to Registry.swift) so it slots in
//  alongside the in-flight registry work without colliding. It builds a full
//  registry via the module-internal `CapabilityRegistry.init(entries:store:)`.
//
//  Contract (ADR-0044/0045): a denied grant yields a typed VmError, never a
//  crash; async capabilities (Push.register, Background.schedule) allocate a
//  Pending result cell and return its id immediately, resolving it via
//  `SignalStore.resolveCell`. The dev host has no real OS providers (no APNs /
//  LocalAuthentication / BGTask / pasteboard), so the bodies below are
//  deterministic dev-safe echoes; the real OS calls are flagged RELEASE-TODO
//  and belong in the app shell.

import Foundation

/// FileSystem contents are persisted into the signal store under a deterministic
/// high signal id derived from the interned path id, well below the cell
/// allocator's 1_000_000 ceiling (see InMemorySignals) so they never collide.
private func fileSignalID(_ pathID: UInt32) -> UInt32 { 900_000 &+ pathID }

extension CapabilityRegistry {
    /// The six concrete capabilities (FLUX-045), as `(capId, methodId, impl)`
    /// triples keyed by deterministic ids matching `stdlib/capabilities.flux`
    /// and `CAPABILITY_IDL` (crates/flux-types/src/capabilities.rs):
    /// - Push (6):       register(6,1) [async], getToken(6,2)
    /// - Biometric (7):  authenticate(7,1)
    /// - Background (8): schedule(8,1) [async], cancel(8,2)
    /// - FileSystem (9): read(9,1), write(9,2), delete(9,3)
    /// - DeepLink (10):  open(10,1)
    /// - Sensors (11):   read(11,1)
    ///
    /// Each impl returns the **signal id** of its result cell. Synchronous ones
    /// write the value and return the id; the two async ones allocate a Pending
    /// cell, return its id, and resolve it inline with a deterministic dev value
    /// (the host would await the real provider in release mode).
    static func concreteCapabilityEntries() -> [(UInt32, UInt16, CapabilityImpl)] {
        [
            // MARK: Push (cap 6)
            (6, 1, { _, _, _, signals in
                // Push.register — async. RELEASE-TODO: UNUserNotificationCenter
                //   .current().requestAuthorization { granted, _ in resolveCell(id, token) }
                let id = signals.allocateCell()
                signals.markPending(id)
                signals.resolveCell(id, .str(42)) // simulated device-token id (dev)
                return id
            }),
            (6, 2, { _, _, _, signals in
                // Push.getToken — surface the last simulated token (signal 42) or null.
                let id = signals.allocateCell()
                signals.write(id, signals.read(42) ?? .null)
                return id
            }),

            // MARK: Biometric (cap 7)
            (7, 1, { _, _, _, signals in
                // Biometric.authenticate — synchronous dev echo. RELEASE-TODO:
                // LAContext().evaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, ...)
                // A denied grant MUST yield a typed error, not a crash.
                let id = signals.allocateCell()
                signals.write(id, .bool(true)) // dev: assume granted
                return id
            }),

            // MARK: Background (cap 8)
            (8, 1, { _, _, _, signals in
                // Background.schedule — async. RELEASE-TODO: BGTaskScheduler
                //   .shared.register(...) + submit(...)
                let id = signals.allocateCell()
                signals.markPending(id)
                signals.resolveCell(id, .str(43)) // simulated task id (dev)
                return id
            }),
            (8, 2, { _, _, _, signals in
                // Background.cancel — dev-safe echo.
                let id = signals.allocateCell()
                signals.write(id, .bool(true))
                return id
            }),

            // MARK: FileSystem (cap 9)
            (9, 1, { _, _, arg, signals in
                // FileSystem.read(path) — contents persisted under a derived signal id.
                guard case let .record(fields) = arg, !fields.isEmpty else { throw VmError.typeMismatch(offset: 0) }
                guard case let .str(pathID) = fields[0].value else { throw VmError.typeMismatch(offset: 0) }
                let id = signals.allocateCell()
                signals.write(id, signals.read(fileSignalID(pathID)) ?? .null)
                return id
            }),
            (9, 2, { _, _, arg, signals in
                // FileSystem.write(path, data) — persist into the signal store.
                // RELEASE-TODO: FileManager.default.createFile(atPath:contents:) for documents.
                guard case let .record(fields) = arg, fields.count >= 2 else { throw VmError.typeMismatch(offset: 0) }
                guard case let .str(pathID) = fields[0].value else { throw VmError.typeMismatch(offset: 0) }
                let data = fields[1].value
                signals.write(fileSignalID(pathID), data)
                let id = signals.allocateCell()
                signals.write(id, data)
                return id
            }),
            (9, 3, { _, _, arg, signals in
                // FileSystem.delete(path) — clear the persisted value.
                guard case let .record(fields) = arg, !fields.isEmpty else { throw VmError.typeMismatch(offset: 0) }
                guard case let .str(pathID) = fields[0].value else { throw VmError.typeMismatch(offset: 0) }
                signals.write(fileSignalID(pathID), .null)
                let id = signals.allocateCell()
                signals.write(id, .null)
                return id
            }),

            // MARK: DeepLink (cap 10)
            (10, 1, { _, _, arg, signals in
                // DeepLink.open(url) — record the target url (signal 44) for the
                // reconciler. RELEASE-TODO: UIApplication.shared.open(url, options: [:], completionHandler:)
                signals.write(44, arg)
                return 44
            }),

            // MARK: Sensors (cap 11)
            (11, 1, { _, _, _, signals in
                // Sensors.read — dev returns an empty record; RELEASE-TODO:
                // CMMotionManager().startAccelerometerUpdates(...) sampling.
                let id = signals.allocateCell()
                signals.write(id, .record([]))
                return id
            }),
        ]
    }

    /// A production registry composing the MLP dev set (1..=5 + async ref 2,99)
    /// with the six concrete capabilities (6..=11). Used by the app shell; the
    /// dev/test harness continues to use `makeDev(backend:)`.
    ///
    /// `Registry.swift` owns the dev entry list (private table), so this factory
    /// reconstructs the full combined set through the module-internal init. Keep
    /// the dev rows below in sync with `Registry.swift:makeDev` if the MLP set
    /// changes.
    static func makeProduction(backend: any StorageBackend = InMemoryStorageBackend()) -> CapabilityRegistry {
        CapabilityRegistry(entries: combinedEntries(), store: backend)
    }

    /// Combined dev (1..=5 + async ref 2,99) + concrete (6..=11) entry list.
    private static func combinedEntries() -> [(UInt32, UInt16, CapabilityImpl)] {
        let dev: [(UInt32, UInt16, CapabilityImpl)] = [
            (1, 1, { _, _, arg, signals in
                guard case let .record(fields) = arg, let first = fields.first else { throw VmError.typeMismatch(offset: 0) }
                signals.write(99, first.value); return 99 }),
            (1, 2, { _, _, _, signals in signals.write(96, .bool(true)); return 96 }),
            (1, 3, { _, _, _, signals in signals.write(96, .bool(false)); return 96 }),
            (2, 1, { _, _, arg, signals in
                guard case let .record(fields) = arg, fields.count >= 2 else { throw VmError.typeMismatch(offset: 0) }
                signals.write(95, fields[1].value); return 95 }),
            (2, 2, { _, _, arg, signals in
                guard case let .record(fields) = arg, !fields.isEmpty else { throw VmError.typeMismatch(offset: 0) }
                signals.write(95, signals.read(95) ?? .null); return 95 }),
            (2, 3, { _, _, arg, signals in
                guard case let .record(fields) = arg, !fields.isEmpty else { throw VmError.typeMismatch(offset: 0) }
                signals.write(95, .null); return 95 }),
            (3, 1, { _, _, arg, signals in signals.write(97, arg); return 97 }),
            (4, 1, { _, _, arg, signals in signals.write(94, arg); return 94 }),
            (4, 2, { _, _, _, signals in let v = signals.read(94) ?? .null; signals.write(93, v); return 93 }),
            (5, 1, { _, _, _, signals in signals.write(92, .null); return 92 }),
            (2, 99, { _, _, _, signals in let id = signals.allocateCell(); signals.markPending(id); return id }),
        ]
        return dev + concreteCapabilityEntries()
    }
}
