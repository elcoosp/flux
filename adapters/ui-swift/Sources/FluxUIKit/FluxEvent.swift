//  FluxEvent.swift
//  FluxUIKit — event payload delivered to a bound handler.

/// An event handed to the executor when a native control fires.
///
/// Adapters construct one of these and forward it to their weak `executor`
/// reference via `FluxExecutor.dispatch(_:)`. The runtime (FLUX-006) turns it
/// into a VM handler evaluation against `handlerId`, scoped to `nodeId`.
public struct FluxEvent: Sendable, Hashable {
    /// The bound handler to evaluate.
    public let handlerId: FluxHandlerId
    /// The IR node that fired the event (scope for signal writes).
    public let nodeId: FluxNodeId
    /// Optional payload — e.g. a `TextField`'s new text as `.str`. A tap carries `nil`.
    public let payload: FluxValue?

    /// Construct an event.
    public init(handlerId: FluxHandlerId, nodeId: FluxNodeId, payload: FluxValue? = nil) {
        self.handlerId = handlerId
        self.nodeId = nodeId
        self.payload = payload
    }
}
