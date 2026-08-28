//  Telemetry.swift
//  DevTools bidirectional debug telemetry (spec §3, §4).
//
//  Defines the host-side `TelemetryEvent` / `DebugCommand` model and a
//  thread-safe `TelemetryBridge` that batches events and encodes them into the
//  `Telemetry` (`0x10`) / `DebugCommand` (`0x11`) frames defined by
//  `flux-ir-serde` (Appendix D §D.12, ADR-0039). The byte layout here is kept
//  bit-identical to `crates/flux-ir-serde/src/telemetry.rs` so the Rust dev
//  server decodes host frames without a translation step.
//
//  ┌───────────────────────────────────────────────────────────────────────┐
//  │ RELEASE COMPILE-OUT (brittleness 8c). The entire telemetry/"trace" sink  │
//  │ is wrapped in `#if DEBUG`. In a Release build none of this — the        │
//  │ `TelemetryEvent`/`DebugCommand` models, the `TelemetryBridge`, the       │
//  │ `DevToolsSocket`, the `encodeValue` helper, nor the `fluxDevtoolsEmit`   │
//  │ no-op — is compiled or linked. The VM (`FluxBytecodeVM`) and signal graph│
//  │ (`SignalGraph`) emit only inside their own `#if DEBUG` blocks, which now │
//  │ reference a symbol that does not exist in Release, so the optimiser       │
//  │ drops every trace call site and leaves zero dead telemetry code linked.  │
//  └───────────────────────────────────────────────────────────────────────┘

#if DEBUG

import Foundation

/// A native view layout rectangle (device points). Mirrors the wire `Rect`.
public struct Rect: Sendable, Equatable {
    /// Left edge (x origin).
    public let x: Double
    /// Top edge (y origin).
    public let y: Double
    /// Width.
    public let width: Double
    /// Height.
    public let height: Double

    public init(x: Double, y: Double, width: Double, height: Double) {
        self.x = x
        self.y = y
        self.width = width
        self.height = height
    }
}

/// Protocol a DevTools transport conforms to in order to receive VM telemetry.
public protocol VMTelemetrySink: AnyObject {
    /// Receives one decoded telemetry event from the host VM / signal graph.
    func emit(_ event: TelemetryEvent)
}

/// A debug telemetry event emitted by the host runtime (raw, host → server).
///
/// Mirrors `flux_ir_serde::TelemetryEvent` (Appendix D §D.12 §2.2). Source-span
/// enrichment happens server-side, so these carry raw IDs only.
public enum TelemetryEvent {
    /// Emitted after each VM instruction executes.
    case vmStep(bytecodeOffset: UInt32, opcode: UInt8, registers: [FluxValue], gasRemaining: UInt32)
    /// Emitted when a signal's value changes.
    case signalWrite(signalId: UInt32, oldValue: FluxValue, newValue: FluxValue, triggeredEffectIds: [UInt32])
    /// Emitted when the reconciler mutates a native view.
    case viewMutation(nodeId: UInt32, nativeViewId: UInt64, mutationKind: UInt8, frame: Rect?)
    /// Emitted when a handler starts or finishes.
    case handlerInvocation(handlerId: UInt32, isStart: Bool, gasUsed: UInt32?)

    /// Encodes this event as a length-prefixed union body (no frame header).
    func encode(into data: inout Data) {
        let start = data.count
        // Reserve a 4-byte length slot; back-patch after the body is written.
        data.append(contentsOf: [UInt8](repeating: 0, count: 4))
        switch self {
        case let .vmStep(offset, opcode, registers, gas):
            data.append(0x01)
            data.append(contentsOf: offset.bytesLE())
            data.append(opcode)
            for reg in registers.prefix(16) {
                encodeValue(reg, into: &data)
            }
            data.append(contentsOf: gas.bytesLE())
        case let .signalWrite(id, oldV, newV, effects):
            data.append(0x02)
            data.append(contentsOf: id.bytesLE())
            encodeValue(oldV, into: &data)
            encodeValue(newV, into: &data)
            data.append(contentsOf: UInt16(effects.count).bytesLE())
            for effect in effects {
                data.append(contentsOf: effect.bytesLE())
            }
        case let .viewMutation(node, native, kind, frame):
            data.append(0x03)
            data.append(contentsOf: node.bytesLE())
            data.append(contentsOf: native.bytesLE())
            data.append(kind)
            if let rect = frame {
                data.append(0x01)
                data.append(contentsOf: rect.x.bitPattern.bytesLE())
                data.append(contentsOf: rect.y.bitPattern.bytesLE())
                data.append(contentsOf: rect.width.bitPattern.bytesLE())
                data.append(contentsOf: rect.height.bitPattern.bytesLE())
            } else {
                data.append(0x00)
            }
        case let .handlerInvocation(handler, isStart, gas):
            data.append(0x04)
            data.append(contentsOf: handler.bytesLE())
            data.append(isStart ? 1 : 0)
            if let g = gas {
                data.append(0x01)
                data.append(contentsOf: g.bytesLE())
            } else {
                data.append(0x00)
            }
        }
        let len = UInt32(data.count - start - 4)
        let lenBytes = len.bytesLE()
        data.replaceSubrange(start ..< start + 4, with: lenBytes)
    }

    /// Encodes this event into a full `Telemetry` frame (Appendix D §D.12).
    func toFrameBytes(version: UInt8 = 1) -> Data {
        var data = Data()
        data.append(contentsOf: FluxTelemetryMagic.bytesLE())
        data.append(version)
        data.append(0x10) // FRAME_TELEMETRY
        data.append(contentsOf: UInt16(1).bytesLE()) // event_count
        self.encode(into: &data)
        return data
    }
}

/// The `MAGIC` header for every Flux wire frame (Appendix D §D.1).
let FluxTelemetryMagic: UInt32 = 0x465C_5558

/// A control command sent DevTools → host (Appendix D §D.12 §2.3).
public enum DebugCommand {
    case pause
    case resume
    case step
    case setBreakpoint(bytecodeOffset: UInt32)
    case clearBreakpoint(bytecodeOffset: UInt32)
    case requestSnapshot

    /// Encodes this command into a full `DebugCommand` frame (kind `0x11`).
    func toFrameBytes(commandId: UInt32, version: UInt8 = 1) -> Data {
        var payload = Data()
        switch self {
        case .pause: payload.append(0x01)
        case .resume: payload.append(0x02)
        case .step: payload.append(0x03)
        case let .setBreakpoint(offset):
            payload.append(0x04)
            payload.append(contentsOf: offset.bytesLE())
        case let .clearBreakpoint(offset):
            payload.append(0x05)
            payload.append(contentsOf: offset.bytesLE())
        case .requestSnapshot: payload.append(0x06)
        }
        var data = Data()
        data.append(contentsOf: FluxTelemetryMagic.bytesLE())
        data.append(version)
        data.append(0x11) // FRAME_DEBUG_COMMAND
        data.append(contentsOf: commandId.bytesLE())
        data.append(contentsOf: UInt16(payload.count).bytesLE())
        data.append(payload)
        return data
    }
}

/// Thread-safe batching queue that flushes telemetry to the WebSocket.
///
/// The VM and signal graph call `emit` from their evaluation context; the
/// bridge serializes onto its own queue and batches (every 16 ms or on a
/// 10-event threshold) so instrumentation never blocks the VM or UI thread
/// (spec §3.3).
public final class TelemetryBridge: VMTelemetrySink {
    private let queue = DispatchQueue(label: "dev.flux.telemetry")
    private var pending: [TelemetryEvent] = []

    /// Appends an event to the batch, flushing when the threshold is reached.
    public func emit(_ event: TelemetryEvent) {
        queue.async {
            self.pending.append(event)
            if self.pending.count >= 10 {
                self.flush()
            }
        }
    }

    /// Encodes and forwards all pending events (called on the bridge queue).
    func flush() {
        guard !pending.isEmpty else { return }
        let batch = pending
        pending.removeAll()
        var frame = Data()
        frame.append(contentsOf: FluxTelemetryMagic.bytesLE())
        frame.append(1)
        frame.append(0x10)
        frame.append(contentsOf: UInt16(batch.count).bytesLE())
        for event in batch {
            event.encode(into: &frame)
        }
        // The concrete send is performed by the host transport, which retains
        // this bridge and observes `pending` via `takePending()`.
        onFlush?(frame)
    }

    /// Hook the host transport sets to actually transmit a batched frame.
    var onFlush: ((Data) -> Void)?

    /// Drains and returns all pending events (test/transport helper).
    func takePending() -> [TelemetryEvent] {
        queue.sync {
            let batch = pending
            pending.removeAll()
            return batch
        }
    }
}

/// The active DevTools telemetry sink, or `nil` when DevTools is disconnected.
///
/// The VM and signal graph call `fluxDevtoolsEmit` after each observable step;
/// when no sink is attached the call is a no-op (zero release impact).
///
/// Marked `nonisolated(unsafe)` because it is externally synchronized: it is
/// assigned exactly once at host startup (before any handler runs) and only
/// ever read afterwards, so the Swift 6 concurrency checker's global-mutable
/// state rule does not apply.
nonisolated(unsafe) public var fluxDevtoolsSink: (any VMTelemetrySink)?

/// Best-effort WebSocket connection from the host to the dev server's `:7333`
/// DevTools endpoint. Telemetry frames are sent here; the server enriches them
/// with source spans and broadcasts to connected DevTools clients. Sends are
/// dropped if the endpoint is unavailable, so a missing dev server never
/// affects the running app (spec §3 Key Principle 1).
final class DevToolsSocket {
    private let session: URLSession
    private var task: URLSessionWebSocketTask?

    init() {
        session = URLSession(configuration: .default)
    }

    func connect(host: String = "127.0.0.1", port: UInt16 = 7333) {
        guard let url = URL(string: "ws://\(host):\(port)/devtools") else { return }
        let task = session.webSocketTask(with: url)
        self.task = task
        task.resume()
    }

    func send(_ data: Data) {
        task?.send(.data(data)) { _ in }
    }

    func close() {
        task?.cancel(with: .goingAway, reason: nil)
        task = nil
    }
}

nonisolated(unsafe) private var devtoolsSocket: DevToolsSocket?

/// Opens the host → DevTools `:7333` channel and installs the telemetry sink so
/// emitted VM/signal events flow to the dev server. Call once at host startup
/// (DEBUG builds only). Safe to call when no dev server is running: the socket
/// retries are not attempted, but sends before connect complete are dropped.
public func fluxDevtoolsConnect(host: String = "127.0.0.1", port: UInt16 = 7333) {
    let socket = DevToolsSocket()
    socket.connect(host: host, port: port)
    devtoolsSocket = socket
    let bridge = TelemetryBridge()
    fluxDevtoolsSetSink(bridge)
    bridge.onFlush = { devtoolsSocket?.send($0) }
}

/// Attaches (or clears) the DevTools telemetry sink.
public func fluxDevtoolsSetSink(_ sink: (any VMTelemetrySink)?) {
    fluxDevtoolsSink = sink
}

/// Emits a telemetry event if a DevTools sink is attached (DEBUG builds only).
public func fluxDevtoolsEmit(_ event: TelemetryEvent) {
    fluxDevtoolsSink?.emit(event)
}

/// Encodes a `FluxValue` into `data` per Appendix D §D.5.
func encodeValue(_ value: FluxValue, into data: inout Data) {
    data.append(value.tag)
    switch value {
    case .null:
        break
    case let .int(v):
        data.append(contentsOf: v.bytesLE())
    case let .float(v):
        data.append(contentsOf: v.bitPattern.bytesLE())
    case let .bool(v):
        data.append(v ? 1 : 0)
    case let .str(id):
        data.append(contentsOf: id.bytesLE())
    case let .handlerRef(id):
        data.append(contentsOf: id.bytesLE())
    case let .list(items):
        data.append(contentsOf: UInt16(items.count).bytesLE())
        for item in items {
            encodeValue(item, into: &data)
        }
    case let .record(fields):
        data.append(contentsOf: UInt16(fields.count).bytesLE())
        for (index, field) in fields {
            data.append(contentsOf: index.bytesLE())
            encodeValue(field, into: &data)
        }
    }
}

extension FixedWidthInteger {
    /// Little-endian byte representation.
    func bytesLE() -> [UInt8] {
        withUnsafeBytes(of: self.littleEndian) { Array($0) }
    }
}

#endif
