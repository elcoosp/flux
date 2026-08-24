//  FluxExecutor.swift
//  FluxUIKit — the executor contract the adapters call back into.

import Foundation

/// The host coordinator an adapter notifies when a native control fires.
///
/// In the dev runtime (FLUX-006) this is the `FluxExecutor`, which evaluates
/// the bound VM handler on a background queue and applies resulting view
/// mutations on the main queue. Adapters hold the executor by **weak**
/// reference so they cannot keep the executor (and the signal graph) alive
/// past its lifetime — see each adapter's documented `weak var executor`.
@MainActor
public protocol FluxExecutor: AnyObject {
    /// Dispatch an event for evaluation. Must not retain the caller.
    func dispatch(_ event: FluxEvent)
}
