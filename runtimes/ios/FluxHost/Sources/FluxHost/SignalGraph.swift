//  SignalGraph.swift
//  SolidJS-style reactive signal graph (FLUX-006 scope item 8).
//
//  A `SignalGraph` owns a value-semantic map of `SignalId -> FluxValue` plus the
//  dependency edges between signals and derived computations.
//
//  Audit D7: writes commit values immediately (reads see latest state) but
//  notifications are batched and flushed in ascending signal-id order,
//  mirroring Kotlin's SignalGraph (pending + flush).

import Foundation

/// A signal identifier (matches the VM's `u32` signal space).
public typealias SignalId = UInt32

/// A token returned when subscribing; invalidating it removes the observer.
struct Subscription: Hashable, Sendable {
    let id: UInt64
}

/// The reactive state of a single signal cell (ADR-0044, MLP v2 first-class async).
public enum CellState: Equatable, Sendable {
    case ready
    case pending
    case error(message: String)
}

/// A minimal reactive signal store. Reads are O(1) dictionary lookups; writes
/// notify only the observers registered for the written signal. It conforms to
/// `SignalStore` so the VM can run handlers against it directly.
public struct SignalGraph: SignalStore {
    /// The current value of every signal.
    private(set) var values: [SignalId: FluxValue]
    /// Observers keyed by the signal they watch.
    private var observers: [SignalId: [Subscription: () -> Void]]
    /// Monotonic id source for subscriptions.
    private var nextSub: UInt64
    /// Monotonic id source for `allocateCell`, drawn from a high ceiling so it
    /// never collides with fixed ids like 97/99.
    private var nextCell: UInt32 = 1_000_000
    /// Reactive state of each cell (ADR-0044). A successful `write` resolves a
    /// cell back to `.ready`; async-derived/resource cells are `.pending` while
    /// their future is in flight.
    private var cellStates: [SignalId: CellState]
    /// Audit D7: pending notifications batched for ordered flush.
    private var pendingNotifications: Set<SignalId> = []

    /// Creates an empty graph.
    public init(values: [SignalId: FluxValue] = [:]) {
        self.values = values
        self.observers = [:]
        self.nextSub = 1
        self.cellStates = [:]
        self.nextCell = 1_000_000
    }

    /// Reads a signal's current value, or `nil` if it has never been written.
    public func read(_ id: UInt32) -> FluxValue? {
        values[id]
    }

    /// Returns the reactive [CellState] of `id`, defaulting to `.ready`.
    public func cellState(_ id: UInt32) -> CellState {
        cellStates[id] ?? .ready
    }

    /// Marks `id` as `.pending` (an async-derived/resource cell went in flight).
    public mutating func markPending(_ id: UInt32) {
        cellStates[id] = .pending
    }

    /// Marks `id` as `.error` with `message`; the future resolved with a fault.
    mutating func markError(_ id: UInt32, message: String) {
        cellStates[id] = .error(message: message)
    }

    /// Writes a value and records it for the next [flush].
    /// Audit D7: value is committed immediately (reads see latest state), but
    /// notifications are deferred until flush() to match Kotlin batching.
    public mutating func write(_ id: UInt32, _ value: FluxValue) {
        values[id] = value
        cellStates[id] = .ready
        pendingNotifications.insert(id)
        #if DEBUG
        fluxDevtoolsEmit(.signalWrite(signalId: id, oldValue: .null, newValue: value, triggeredEffectIds: []))
        #endif
    }

    /// Audit D7: flushes pending notifications in ascending signal-id order.
    /// Mirrors Kotlin's SignalGraph.flush().
    public mutating func flush() {
        if pendingNotifications.isEmpty { return }
        let batch = pendingNotifications.sorted()
        pendingNotifications.removeAll()
        for id in batch {
            let value = values[id] ?? .null
            let subs = observers[id] ?? [:]
            for notify in subs.values { notify() }
        }
    }

    /// Allocates a fresh, unbound signal id for a new capability result cell (ADR-0045).
    /// Drawn from a high ceiling so it never collides with fixed ids like 97/99.
    public mutating func allocateCell() -> UInt32 {
        nextCell &+= 1
        return nextCell
    }

    /// Resolves `id` to `value`, marking it `.ready` (an async capability finished).
    public mutating func resolveCell(_ id: UInt32, _ value: FluxValue) {
        values[id] = value
        cellStates[id] = .ready
    }

    /// Seeds a value without notifying observers (used for initial state seeds
    /// from an Init frame, where nothing is observing yet).
    mutating func seed(_ id: SignalId, _ value: FluxValue) {
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
    public func snapshot() -> [(UInt32, FluxValue)] {
        values.map { ($0.key, $0.value) }.sorted { $0.0 < $1.0 }
    }
}
