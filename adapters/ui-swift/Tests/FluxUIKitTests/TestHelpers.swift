//  TestHelpers.swift
//  FluxUIKitTests — shared doubles for adapter tests (FLUX-008).

import Foundation
@testable import FluxUIKit

/// A mock executor that records the events it was asked to dispatch.
///
/// Tests assert on `dispatched` to verify an adapter forwards native control
/// events to the runtime through its weak executor reference.
@MainActor
final class MockExecutor: FluxExecutor {
    /// The events received so far, in order.
    private(set) var dispatched: [FluxEvent] = []
    /// Whether `dispatch` was ever called.
    var didDispatch: Bool { !dispatched.isEmpty }

    func dispatch(_ event: FluxEvent) { dispatched.append(event) }
}
