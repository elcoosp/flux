//  Registry.swift
//  VM-side extension points for the native Flux VM: string resolution and the
//  capability (`CALL_CAP`) registry (Appendix E §E.1, §E.2).
//
//  Both are injected into `FluxBytecodeVM.run` rather than referenced through
//  globals, so the VM stays a pure function of its inputs (testable, and safe
//  to run on any actor). The dev runtime wires real tables in; tests pass
//  purpose-built ones.

import Foundation

/// A capability implementation invoked by `CALL_CAP`.
///
/// Receives the `(capId, methodId)` that selected it, the call's argument
/// register value, and a mutable view of the live signal store so it can read
/// or write state (e.g. a `Storage` capability persisting a value). Returns the
/// value placed into the caller's result register.
typealias CapabilityImpl = (
    _ capId: UInt32,
    _ methodId: UInt16,
    _ argument: VMValue,
    _ signals: inout SignalStore
) throws -> VMValue

/// A data-driven registry mapping `(capId, methodId)` pairs to their
/// `CapabilityImpl` (G4). Replaces the previous hardcoded `if capID == 1,
/// methodID == 1` branch so new capabilities (Camera / Storage / Router) slot
/// in via table entries rather than literal comparisons in the interpreter.
///
/// `CapabilityRegistry.dev` carries the MLP placeholder implementations; the
/// dev server forwards to real native handlers over the WebSocket in a later
/// pass, and release builds register code-generated native implementations.
///
/// The registry is an immutable value (its entries are fixed at construction);
/// the placeholder `CapabilityImpl` closures capture no shared mutable state
/// (they only touch the `signals` argument passed per call), so it is safe to
/// share across actors. Swift's concurrency checker cannot prove a closure
/// holding an `inout` parameter is `Sendable`, hence the explicit opt-out.
final class CapabilityRegistry: @unchecked Sendable {
    /// The backing `(capId, methodId)` → impl table.
    private let table: [(capId: UInt32, methodId: UInt16, impl: CapabilityImpl)]

    /// Creates a registry from explicit entries.
    /// - Parameter entries: `(capId, methodId, impl)` triples; later entries
    ///   win on duplicate keys.
    init(entries: [(UInt32, UInt16, CapabilityImpl)] = []) {
        self.table = entries
    }

    /// Looks up the implementation for `(capId, methodId)`, or `nil` if no
    /// capability is registered for that pair.
    func lookup(_ capId: UInt32, _ methodId: UInt16) -> CapabilityImpl? {
        table.last(where: { $0.capId == capId && $0.methodId == methodId })?.impl
    }

    /// A registry with the MLP placeholder capabilities registered.
    ///
    /// - `Camera.take` (cap 1, method 1): echoes its first argument into signal
    ///   99 and returns it, standing in for a real capture in dev builds.
    /// - `Storage.set` (cap 2, method 1): writes its first argument into signal
    ///   98 (the persisted-value cell) and returns it.
    /// - `Router.navigate` (cap 3, method 1): records the target string id in
    ///   signal 97 and returns `.null` (navigation is driven by the reconciler).
    static let dev: CapabilityRegistry = CapabilityRegistry(entries: [
        (1, 1, { _, _, arg, signals in
            signals.write(99, arg)
            return arg
        }),
        (2, 1, { _, _, arg, signals in
            signals.write(98, arg)
            return arg
        }),
        (3, 1, { _, _, arg, signals in
            signals.write(97, arg)
            return .null
        }),
    ])
}
