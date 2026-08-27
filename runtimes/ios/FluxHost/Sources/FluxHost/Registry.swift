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
    _ argument: VMValue,
    _ signals: inout SignalStore
) throws -> UInt32

/// Backing state for stateful capabilities (e.g. `Storage`), shared by every
/// impl registered in a registry. Kept separate from the signal graph so
/// capabilities can hold data the reactive tree does not (a persisted blob is
/// not a UI signal)._dev builds register an in-memory store; release builds
/// register one backed by the platform (UserDefaults / file manager).
final class CapabilityStore {
    /// Persisted `Storage` values, keyed by their interned string id.
    private var storage: [UInt32: VMValue] = [:]

    /// Records a `Storage` value; `nil` clears the key.
    func putStorage(_ key: UInt32, _ value: VMValue?) {
        if let value {
            storage[key] = value
        } else {
            storage.removeValue(forKey: key)
        }
    }

    /// Reads a previously recorded `Storage` value, or `nil`.
    func getStorage(_ key: UInt32) -> VMValue? { storage[key] }
}

/// A data-driven registry mapping `(capId, methodId)` pairs to their
/// `CapabilityImpl` (G4). Replaces the previous hardcoded `if capID == 1,
/// methodID == 1` branch so new capabilities (Camera / Storage / Router) slot
/// in via table entries rather than literal comparisons in the interpreter.
///
/// `CapabilityRegistry.dev` carries the MLP placeholder + real in-memory
/// implementations; the dev server forwards requests that need a real native
/// backend over the WebSocket in a later pass, and release builds register
/// code-generated native implementations. The registry is an immutable value
/// (its entries are fixed at construction); the placeholder `CapabilityImpl`
/// closures capture no shared mutable state (they only touch the `signals`
/// argument passed per call, or the injected `store`), so it is safe to share
/// across actors. Swift's concurrency checker cannot prove a closure holding an
/// `inout` parameter is `Sendable`, hence the explicit opt-out.
final class CapabilityRegistry: @unchecked Sendable {
    /// The backing `(capId, methodId)` → impl table.
    private let table: [(capId: UInt32, methodId: UInt16, impl: CapabilityImpl)]

    /// Stateful capability backing store (e.g. `Storage`), shared by impls.
    private let store: CapabilityStore

    /// Creates a registry from explicit entries.
    /// - Parameters:
    ///   - entries: `(capId, methodId, impl)` triples; later entries win on
    ///     duplicate keys.
    ///   - store: backing store for stateful capabilities; a fresh in-memory
    ///     store when omitted.
    init(entries: [(UInt32, UInt16, CapabilityImpl)] = [], store: CapabilityStore = CapabilityStore()) {
        self.table = entries
        self.store = store
    }

    /// Looks up the implementation for `(capId, methodId)`, or `nil` if no
    /// capability is registered for that pair.
    func lookup(_ capId: UInt32, _ methodId: UInt16) -> CapabilityImpl? {
        table.last(where: { $0.capId == capId && $0.methodId == methodId })?.impl
    }

    /// A registry with the MLP placeholder capabilities registered (G4).
    ///
    /// IDs follow `stdlib/capabilities.flux` and the debug-bridge convention:
    /// - `Camera`  (cap 1): `take` (1,1), `startPreview` (1,2), `stopPreview` (1,3).
    /// - `Storage` (cap 2): `set` (2,1), `get` (2,2), `delete` (2,3).
    /// - `Router`  (cap 3): `navigate` (3,1).
    ///
    /// Dev implementations are synchronous stand-ins for the real native
    /// backends: `Camera.take` synthesises a deterministic `Data` payload
    /// (a `List[Int]` of bytes) so a capture result is observable without a
    /// camera; `Storage` is backed by an in-memory `CapabilityStore`; `Router`
    /// records the target string id in signal 97 and returns `.null` (navigation
    /// is driven by the reconciler). The `Camera.take` (1,1) echo of its first
    /// argument into signal 99 is preserved for `flux-vm-ref` oracle parity
    /// (`call_cap_basic`).
    static let dev: CapabilityRegistry = {
        // The backing store is captured directly by the stateful impl closures
        // (Storage). It is a `class` (reference type), so each closure shares
        // the same instance the registry owns. Declared locally because a
        // `static` property initializer cannot reference the instance's `store`.
        let store = CapabilityStore()
        return CapabilityRegistry(entries: [
            (1, 1, { _, _, arg, signals in
                // Oracle-parity echo: capture args.fields[0] into signal 99 and
                // return that result-cell id. CALL_CAP passes a Record (spec §E.1);
                // `call_cap_basic` reads field 0, so we echo that.
                guard case let .record(fields) = arg, let first = fields.first else {
                    throw VMError.typeMismatch(offset: 0)
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
                // value is the second. Persist into the in-memory store, then expose
                // the value through signal 95 (the Storage result cell).
                guard case let .record(fields) = arg, fields.count >= 2 else {
                    throw VMError.typeMismatch(offset: 0)
                }
                let key = fields[0].value
                let value = fields[1].value
                guard case let .str(keyId) = key else { throw VMError.typeMismatch(offset: 0) }
                store.putStorage(keyId, value)
                signals.write(95, value)
                return 95
            }),
            (2, 2, { _, _, arg, signals in
                // Storage.get(key): read the persisted value, defaulting to `null`, and
                // expose it through signal 95.
                guard case let .record(fields) = arg, !fields.isEmpty else {
                    throw VMError.typeMismatch(offset: 0)
                }
                guard case let .str(keyId) = fields[0].value else { throw VMError.typeMismatch(offset: 0) }
                let value = store.getStorage(keyId) ?? .null
                signals.write(95, value)
                return 95
            }),
            (2, 3, { _, _, arg, signals in
                // Storage.delete(key): clear the persisted value and surface `null`.
                guard case let .record(fields) = arg, !fields.isEmpty else {
                    throw VMError.typeMismatch(offset: 0)
                }
                guard case let .str(keyId) = fields[0].value else { throw VMError.typeMismatch(offset: 0) }
                store.putStorage(keyId, nil)
                signals.write(95, .null)
                return 95
            }),
            (3, 1, { _, _, arg, signals in
                // Router.navigate(target): record the target string id in signal 97;
                // the reconciler consumes it. Returns signal 97's id.
                signals.write(97, arg)
                return 97
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
    }()
}
