//  Registry.swift
//  VM-side extension points for the native Flux VM: string resolution and the
//  capability (`CALL_CAP`) registry (Appendix E §E.1, §E.2).
//
//  Both are injected into `FluxBytecodeVM.run` rather than referenced through
//  globals, so the VM stays a pure function of its inputs (testable, and safe
//  to run on any actor). The dev runtime wires real tables in; tests pass
//  purpose-built ones.

import Foundation

/// A capability implementation invoked by `CALL_CAP` (ADR-0045, unified sync/async bridge).
///
/// Receives the `(capId, methodId)` that selected it, the call's argument register
/// value, and a mutable view of the live signal store so it can create a result cell.
/// Returns the **signal id** of that result cell — never the value directly:
/// - a **synchronous** method writes `Ready(value)` into the cell and returns its id;
/// - an **asynchronous** method creates the cell (state `Pending`) and returns its id
///   immediately; the host resolves it later via `SignalStore.resolveCell`, which
///   resumes any awaiting handler.
///
/// One signature serves both shapes; the VM never branches on sync-vs-async.
typealias CapabilityImpl = (
    _ capId: UInt32,
    _ methodId: UInt16,
    _ argument: FluxValue,
    _ signals: inout SignalStore
) throws -> UInt32

/// Backing state for stateful capabilities (e.g. `Storage`), shared by every
/// impl registered in a registry. Kept separate from the signal graph so
/// capabilities can hold data the reactive tree does not (a persisted blob is
/// not a UI signal).
///
/// `CapabilityStore` is now a thin named alias over `InMemoryStorageBackend`
/// — the injection seam the registry uses. Dev/test builds register an
/// in-memory store; the app shell registers a `UserDefaultsStorageBackend` so
/// `Storage.set`/`get`/`delete` persist across process restarts (Task 1,
/// LANE-C). Both conform to `StorageBackend`, so the impls never know which
/// they are talking to.
typealias CapabilityStore = InMemoryStorageBackend

/// A data-driven registry mapping `(capId, methodId)` pairs to their
/// `CapabilityImpl` (G4). Replaces the previous hardcoded `if capID == 1,
/// methodID == 1` branch so new capabilities (Camera / Storage / Router) slot
/// in via table entries rather than literal comparisons in the interpreter.
///
/// `CapabilityRegistry.dev` (now `makeDev(backend:)`) carries the MLP
/// placeholder + real in-memory implementations; `Storage` is backed by the
/// injected `StorageBackend`. The registry is an immutable value (its entries
/// are fixed at construction); the placeholder `CapabilityImpl` closures
/// capture no shared mutable state (they only touch the `signals` argument
/// passed per call, or the injected `store`), so it is safe to share across
/// actors. Swift's concurrency checker cannot prove a closure holding an
/// `inout` parameter is `Sendable`, hence the explicit opt-out.
final class CapabilityRegistry: @unchecked Sendable {
    /// The backing `(capId, methodId)` → impl table.
    private let table: [(capId: UInt32, methodId: UInt16, impl: CapabilityImpl)]

    /// Stateful capability backing store (e.g. `Storage`), shared by impls.
    private let store: any StorageBackend

    /// Creates a registry from explicit entries.
    /// - Parameters:
    ///   - entries: `(capId, methodId, impl)` triples; later entries win on
    ///     duplicate keys.
    ///   - store: backing store for stateful capabilities; a fresh in-memory
    ///     store when omitted.
    init(entries: [(UInt32, UInt16, CapabilityImpl)] = [], store: any StorageBackend = InMemoryStorageBackend()) {
        self.table = entries
        self.store = store
    }

    /// Looks up the implementation for `(capId, methodId)`, or `nil` if no
    /// capability is registered for that pair.
    func lookup(_ capId: UInt32, _ methodId: UInt16) -> CapabilityImpl? {
        table.last(where: { $0.capId == capId && $0.methodId == methodId })?.impl
    }

    /// A registry with the MLP capability set registered (G4).
    ///
    /// IDs follow `stdlib/capabilities.flux` and the debug-bridge convention:
    /// - `Camera`  (cap 1): `takePicture` (1,1), `startPreview` (1,2), `stopPreview` (1,3).
    /// - `Storage` (cap 2): `setItem` (2,1), `getItem` (2,2), `removeItem` (2,3).
    /// - `Router`  (cap 3): `navigate` (3,1).
    /// - `Clipboard` (cap 4): `setString` (4,1), `getString` (4,2).
    /// - `Geolocation` (cap 5): `getCurrentPosition` (5,1).
    ///
    /// `Storage` is backed by the injected `StorageBackend` (dev/test:
    /// in-memory; app shell: `UserDefaults`) — see Task 1 (LANE-C). `Camera.takePicture`
    /// (1,1) preserves the oracle-parity echo of its first argument into signal
    /// 99 so `flux-vm-ref`'s `call_cap_basic` vector stays green. `startPreview`/
    /// `stopPreview` manage a preview flag (signal 96) and are no-ops for capture
    /// in headless builds. `Router.navigate` (3,1) records the target string id
    /// in signal 97 (reconciler-driven). `Clipboard`/`Geolocation` expose their
    /// synchronous result through dedicated cells (94/93 and 92); the dev/test
    /// bodies use deterministic in-memory echoes since the MLP dev host has no
    /// real pasteboard/location (real OS access is a release-mode concern).
    ///
    /// - Parameter backend: the `Storage` persistence backend; defaults to an
    ///   in-memory store (dev/test). Pass `UserDefaultsStorageBackend` for a
    ///   persist-to-disk registry.
    static func makeDev(backend: any StorageBackend = InMemoryStorageBackend()) -> CapabilityRegistry {
        let store = backend
        return CapabilityRegistry(entries: [
            (1, 1, { _, _, arg, signals in
                // Oracle-parity echo: capture args.fields[0] into signal 99 and
                // return that result-cell id. CALL_CAP passes a Record (spec §E.1);
                // `call_cap_basic` reads field 0, so we echo that. The dev-safe
                // camera bridge (real capture behind UIImagePickerController /
                // PHPhotoLibrary) is intentionally NOT wired here so the oracle
                // vector stays deterministic (Task 2, LANE-C): headless/test
                // builds keep this echo; the app shell supplies real capture via
                // a separate `CameraCapability` that still writes field 0 → 99.
                guard case let .record(fields) = arg, let first = fields.first else {
                    throw VmError.typeMismatch(offset: 0)
                }
                signals.write(99, first.value)
                return 99
            }),
            (1, 2, { _, _, _, signals in
                // startPreview: records that preview is active (signal 96 = preview flag).
                signals.write(96, .bool(true))
                return 96
            }),
            (1, 3, { _, _, _, signals in
                signals.write(96, .bool(false))
                return 96
            }),
            (2, 1, { _, _, arg, signals in
                // Storage.set(key, value): key is the first record field (a Str id),
                // value is the second. Persist into the backend store, then expose
                // the value through signal 95 (the Storage result cell).
                guard case let .record(fields) = arg, fields.count >= 2 else {
                    throw VmError.typeMismatch(offset: 0)
                }
                let key = fields[0].value
                let value = fields[1].value
                guard case let .str(keyId) = key else { throw VmError.typeMismatch(offset: 0) }
                store.put(keyId, value)
                signals.write(95, value)
                return 95
            }),
            (2, 2, { _, _, arg, signals in
                // Storage.get(key): read the persisted value, defaulting to `null`, and
                // expose it through signal 95.
                guard case let .record(fields) = arg, !fields.isEmpty else {
                    throw VmError.typeMismatch(offset: 0)
                }
                guard case let .str(keyId) = fields[0].value else { throw VmError.typeMismatch(offset: 0) }
                let value = store.get(keyId) ?? .null
                signals.write(95, value)
                return 95
            }),
            (2, 3, { _, _, arg, signals in
                // Storage.delete(key): clear the persisted value and surface `null`.
                guard case let .record(fields) = arg, !fields.isEmpty else {
                    throw VmError.typeMismatch(offset: 0)
                }
                guard case let .str(keyId) = fields[0].value else { throw VmError.typeMismatch(offset: 0) }
                store.put(keyId, nil)
                signals.write(95, .null)
                return 95
            }),
            (3, 1, { _, _, arg, signals in
                // Router.navigate(target): record the target string id in signal 97;
                // the reconciler consumes it. Returns signal 97's id.
                signals.write(97, arg)
                return 97
            }),
            (4, 1, { _, _, arg, signals in
                // Clipboard.set(value): echo the value into signal 94 (the
                // Clipboard result cell). The MLP dev host has no real pasteboard,
                // so the dev body is a deterministic echo; release mode would
                // forward to UIPasteboard / ClipboardManager.
                signals.write(94, arg)
                return 94
            }),
            (4, 2, { _, _, _, signals in
                // Clipboard.get(): surface the last set value (signal 94) through
                // signal 93; default to `null` when nothing was set.
                let value = signals.read(94) ?? .null
                signals.write(93, value)
                return 93
            }),
            (5, 1, { _, _, _, signals in
                // Geolocation.get(): the MLP dev host has no real location
                // provider, so surface a deterministic `null` (no fix available)
                // through signal 92. Release mode would resolve CLLocationManager
                // / FusedLocationProvider and write the coordinate here.
                signals.write(92, .null)
                return 92
            }),
            (2, 99, { _, _, _, signals in
                // Reference async capability: allocates a fresh result cell, marks it
                // `Pending`, and returns its id immediately (ADR-0045). The host
                // resolves it later via `SignalStore.resolveCell`, resuming the
                // awaiting handler. Mirrors the oracle's `async_deferred` (cap 2, method 99).
                let id = signals.allocateCell()
                signals.markPending(id)
                return id
            }),
        ], store: store)
    }

    /// The MLP dev registry: `Storage` backed by an in-memory store.
    ///
    /// Kept for source compatibility; new call sites should prefer
    /// `makeDev(backend:)` so the app shell can pass a persistent backend.
    static let dev: CapabilityRegistry = makeDev()
}
