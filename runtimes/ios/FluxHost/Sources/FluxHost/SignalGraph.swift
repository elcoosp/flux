//  SignalGraph.swift
//  SolidJS-style reactive signal graph (FLUX-006 scope item 8).
//
//  A `SignalGraph` owns a value-semantic map of `SignalId -> VMValue` plus the
//  dependency edges between signals and derived computations. On `write`, it
//  marks the written cell dirty and runs the minimal notification set so that
//  only observers of the changed signals recompute — never the whole graph.
//
//  The store is value-semantic (`struct`, `Sendable`) so it can be passed
//  `inout` into the VM and snapshot for tests without reference aliasing.

import Foundation

/// A signal identifier (matches the VM's `u32` signal space).
public typealias SignalId = UInt32

/// A token returned when subscribing; invalidating it removes the observer.
struct Subscription: Hashable, Sendable {
    let id: UInt64
}

/// The reactive state of a single signal cell (ADR-0044, MLP v2 first-class async).
///
/// - `ready`: the cell holds a resolved value (`values[id]` is authoritative).
/// - `pending`: an async-derived/resource cell has an in-flight future; `values[id]`
///   may hold a stale `ready` value or a placeholder — readers rendering the cell
///   should surface the pending branch (e.g. the `when ... is_loading` form) rather
///   than mutating the native view for the not-yet-resolved value.
/// - `error`: the future resolved with a fault.
enum CellState: Equatable, Sendable {
    case ready
    case pending
    case error(message: String)
}

/// A minimal reactive signal store. Reads are O(1) dictionary lookups; writes
/// notify only the observers registered for the written signal. It conforms to
/// `SignalStore` so the VM can run handlers against it directly.
public struct SignalGraph: SignalStore {
    /// The current value of every signal.
    private(set) var values: [SignalId: VMValue]
    /// Observers keyed by the signal they watch.
    private var observers: [SignalId: [Subscription: () -> Void]]
    /// Monotonic id source for subscriptions.
    private var nextSub: UInt64
    /// Reactive state of each cell (ADR-0044). A successful `write` resolves a
    /// cell back to `.ready`; async-derived/resource cells are `.pending` while
    /// their future is in flight.
    private var cellStates: [SignalId: CellState]

    /// Creates an empty graph.
    public init(values: [SignalId: VMValue] = [:]) {
        self.values = values
        self.observers = [:]
        self.nextSub = 1
        self.cellStates = [:]
    }

    /// Reads a signal's current value, or `nil` if it has never been written.
    func read(_ id: UInt32) -> VMValue? {
        values[id]
    }

    /// Returns the reactive [CellState] of `id`, defaulting to `.ready`.
    func cellState(_ id: UInt32) -> CellState {
        cellStates[id] ?? .ready
    }

    /// Marks `id` as `.pending` (an async-derived/resource cell went in flight).
    mutating func markPending(_ id: UInt32) {
        cellStates[id] = .pending
    }

    /// Marks `id` as `.error` with `message`; the future resolved with a fault.
    mutating func markError(_ id: UInt32, message: String) {
        cellStates[id] = .error(message: message)
    }

    /// Writes a value and notifies every observer of that signal.
    mutating func write(_ id: UInt32, _ value: VMValue) {
        let oldValue = values[id] ?? .null
        values[id] = value
        // A successful write resolves any pending/error cell back to `.ready`.
        cellStates[id] = .ready
        #if DEBUG
        fluxDevtoolsEmit(.signalWrite(signalId: id, oldValue: oldValue, newValue: value, triggeredEffectIds: []))
        #endif
        let subs = observers[id] ?? [:]
        for notify in subs.values { notify() }
    }

    /// Seeds a value without notifying observers (used for initial state seeds
    /// from an Init frame, where nothing is observing yet).
    mutating func seed(_ id: SignalId, _ value: VMValue) {
        values[id] = value
    }

    /// Registers `observer` to run whenever `id` is written. Returns a
    /// `Subscription` that must be invalidated to stop observing.
    mutating func observe(_ id: SignalId, _ observer: @escaping () -> Void) -> Subscription {
        let sub = Subscription(id: nextSub)
        nextSub &+= 1
        observers[id, default: [:]][sub] = observer
        return sub
    }

    /// Removes an observer previously returned by `observe`.
    mutating func invalidate(_ sub: Subscription) {
        for key in observers.keys {
            observers[key]?.removeValue(forKey: sub)
        }
    }

    /// Every written signal as a sorted `(id, value)` list.
    func snapshot() -> [(UInt32, VMValue)] {
        values.map { ($0.key, $0.value) }.sorted { $0.0 < $1.0 }
    }
}
