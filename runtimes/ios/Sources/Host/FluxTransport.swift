//  FluxTransport.swift
//  iOS host connection model (FR-017 reconnect UX).
//
//  Defines the transport abstraction the host depends on, plus a small
//  observable connection-state machine that drives the "Reconnecting…" banner.
//  The concrete socket lives in `FluxWebSocketTransport`; tests inject a
//  `MockTransport` so the state machine is exercised without a live socket
//  (mirrors Android's `MockTransport` unit-test contract; real sockets land in
//  FLUX-023).

import Foundation
import FluxHost

/// The connection state surfaced to the UI (FR-017).
public enum ConnectionStatus: Equatable, Sendable {
    /// Socket opening; nothing received yet.
    case connecting
    /// Open and receiving frames.
    case connected
    /// Socket dropped; a retry is scheduled every second.
    case reconnecting
}

/// The bidirectional wire transport the runtime receives frames over.
///
/// Mirrors the Android `FluxTransport` interface (FLUX-007): the host subscribes
/// to raw frame bytes and may push dispatch events back. The concrete
/// implementation (`FluxWebSocketTransport`) uses `URLSessionWebSocketTask`.
@MainActor
public protocol FluxTransport: AnyObject {
    /// The latest connection status (drives the reconnect banner).
    var status: ConnectionStatus { get }

    /// Invoked on the main actor for every received message.
    var onFrame: (@MainActor (Data) -> Void)? { get set }

    /// Invoked on the main actor whenever `status` changes.
    var onStatusChange: (@MainActor (ConnectionStatus) -> Void)? { get set }

    /// Opens the connection and begins delivering frames.
    func connect()

    /// Sends a raw dispatch message (tap/event) back to the server.
    func send(_ bytes: Data)

    /// Closes the connection and cancels any pending retry.
    func close()
}

/// Owns the connection-status state for the host UI.
///
/// A `@MainActor` observable value the SwiftUI layer reads to show/hide the
/// "Reconnecting…" banner. It is driven entirely by transport status changes,
/// so it is unit-testable with an injected `MockTransport` (no socket needed).
@MainActor
public final class HostConnectionState: ObservableObject {
    /// The current status; `true` when the banner should be visible.
    @Published public private(set) var status: ConnectionStatus = .connecting

    /// Whether the reconnect banner is currently shown (FR-017).
    public var isReconnecting: Bool { status == .reconnecting }

    /// Binds the state to a transport: every status change is mirrored here and
    /// `onStatusChange` is forwarded, all on the main actor.
    public func bind(_ transport: FluxTransport) {
        status = transport.status
        transport.onStatusChange = { [weak self] newStatus in
            self?.status = newStatus
        }
    }

    /// Forces a status (used by tests to drive the banner without a socket).
    public func setStatus(_ status: ConnectionStatus) {
        self.status = status
    }
}
