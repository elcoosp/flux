//  FluxWebSocketTransport.swift
//  Dev-mode wire transport: a WebSocket client over `URLSessionWebSocketTask`
//  (Appendix D; FR-017 reconnect UX).
//
//  Frames arrive as binary messages, are decoded by `FrameDeserializer`, and
//  handed to the runtime on the main actor. On a dropped connection the
//  transport surfaces a `.reconnecting` status and retries every second until
//  the socket re-opens (Appendix D §D.13) — it never crashes the host. The dev
//  server pushes a fresh full `Init` frame on each new connection, so a
//  reconnect implicitly re-requests the tree (matches the Android onResume
//  rebind model; there is no separate client→server "request Init" opcode).
//
//  All callbacks run on the main actor to respect `FluxRuntime`'s `@MainActor`
//  confinement (P1).

import Foundation
import FluxHost

/// The live WebSocket transport backed by `URLSessionWebSocketTask`.
@MainActor
public final class FluxWebSocketTransport: FluxTransport {
    /// The `ws://` dev-server URL.
    private let url: URL

    private let session: URLSession
    private var socket: URLSessionWebSocketTask?
    private var retryTask: Task<Void, Never>?

    public private(set) var status: ConnectionStatus = .connecting
    public var onFrame: (@MainActor (Data) -> Void)?
    public var onStatusChange: (@MainActor (ConnectionStatus) -> Void)?

    /// Seconds between reconnect attempts (FR-017: retry every 1 second).
    private let retryInterval: TimeInterval = 1.0

    /// Creates a transport for [url] (e.g. `ws://127.0.0.1:9001`).
    public init(url: URL, session: URLSession = .shared) {
        self.url = url
        self.session = session
    }

    public func connect() {
        // Idempotent: a live or already-opening socket is not replaced.
        guard socket == nil else { return }
        transition(to: .connecting)
        let task = session.webSocketTask(with: url)
        socket = task
        task.resume()
        receiveLoop(task)
    }

    public func send(_ bytes: Data) {
        socket?.send(.data(bytes)) { [weak self] error in
            guard let self, error != nil else { return }
            Task { @MainActor in self.handleDrop() }
        }
    }

    public func close() {
        retryTask?.cancel()
        retryTask = nil
        socket?.cancel(with: .goingAway, reason: nil)
        socket = nil
        transition(to: .connecting)
    }

    // MARK: - Internal

    /// Continuously pulls messages from the socket until it closes or fails.
    /// The `URLSession` callback arrives off the main actor; we hop to the main
    /// actor before touching any `@MainActor` state (`onFrame`, `receiveLoop`,
    /// `handleDrop`).
    private func receiveLoop(_ task: URLSessionWebSocketTask) {
        task.receive { [weak self] result in
            guard let self else { return }
            Task { @MainActor in
                switch result {
                case .success(.data(let data)):
                    self.onFrame?(data)
                    self.receiveLoop(task)
                case .success(.string):
                    // The wire is binary (Appendix D); a text frame is unexpected
                    // but harmless — ignore and keep listening.
                    self.receiveLoop(task)
                case .failure:
                    self.handleDrop()
                @unknown default:
                    self.handleDrop()
                }
            }
        }
    }

    /// Handles a dropped socket: surface `.reconnecting` and retry every second.
    private func handleDrop() {
        socket?.cancel(with: .goingAway, reason: nil)
        socket = nil
        transition(to: .reconnecting)
        scheduleReconnect()
    }

    /// Schedules a single reconnect attempt after `retryInterval` seconds.
    private func scheduleReconnect() {
        retryTask?.cancel()
        retryTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64((self?.retryInterval ?? 1.0) * 1_000_000_000))
            guard !Task.isCancelled else { return }
            self?.connect()
        }
    }

    /// Updates `status` and forwards the change on the main actor.
    private func transition(to newStatus: ConnectionStatus) {
        guard status != newStatus else { return }
        status = newStatus
        onStatusChange?(newStatus)
    }
}
