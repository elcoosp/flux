//  FluxExecutor.swift
//  Dispatches an incoming frame through the VM and signal graph (FLUX-006 scope
//  items 7 & 9) and drives the real `FluxUIKit` adapters (FLUX-016), on the
//  main actor.
//
//  The executor is the boundary between the (future) WebSocket transport and the
//  native UI. In dev mode it is driven synchronously from tests; in the live app
//  the transport hands raw frames to `apply(_:)` and `dispatch(_:)` on the main
//  actor, the VM evaluates handler closures via `FluxBytecodeVM.run`, then the
//  resulting signal writes are reconciled back into the live UIKit tree. No VM
//  error is ever allowed to escape: a fault is captured into `lastError` and
//  surfaced as an error overlay by `FluxRootView`.

import Foundation
import UIKit
import FluxUIKit

/// The outcome of dispatching one frame.
struct DispatchResult: Sendable {
    /// Node ids whose views were (re)built or updated by the reconciler.
    let builtOrUpdated: [UInt32]
    /// The signals written by handler evaluation, sorted by id.
    let signals: [(UInt32, FluxValue)]
    /// `nil` on success, or the VM error that occurred.
    let error: VmError?
}

/// Resolves an awaited future handle to its settled value (ADR-0044, MLP v2 async).
///
/// When a handler suspends on `AWAIT`, the VM captures the future handle from the
/// instruction's `future_reg` register and hands it here. The conforming type bridges
/// that handle to the real asynchronous work (a network capability, a timer, a
/// `CapabilityRegistry` async impl) and returns the resolved `FluxValue`. The runtime
/// then resumes the handler with the value in `r0`.
///
/// The default [`PassthroughAsyncResolver`] treats the handle as already-resolved, so
/// synchronous-style `await` (and headless tests) work without a transport. A live
/// host overrides `asyncResolver` with a bridge to its real async capability surface.
public protocol AsyncResolver: Sendable {
    /// Resolves `future` to its settled value, possibly after awaiting real async work.
    ///
    /// `future` is the value the handler's `AWAIT` parked on — the result-cell
    /// signal id (an `.int`) that the `CALL_CAP` returned. A resolver reads that
    /// cell from the graph, performs the real async work (network, timer,
    /// capability), and returns the value the handler should resume with in `r0`.
    func resolve(_ future: FluxValue) async -> FluxValue
}

/// Default resolver: the awaited value is treated as already settled.
///
/// Used headless and in tests where `await` is exercised without a real async
/// backend; the future handle flows straight back as the resolved value. The
/// live host replaces `FluxExecutor.asyncResolver` with a bridge (e.g.
/// `DelayAsyncResolver` or a `CapabilityAsyncResolver`) to real async work.
public struct PassthroughAsyncResolver: AsyncResolver {
    public func resolve(_ future: FluxValue) async -> FluxValue { future }
}

/// Resolves an awaited `Pending` cell after a real wall-clock delay.
///
/// Demonstrates that `AWAIT` genuinely parks the handler until the future
/// settles (rather than running synchronously), using `Task.sleep` on the
/// injected closure so the wait is test-injectable. The resolved value is
/// `Null` — the oracle's `(2,99)` async capability leaves the cell empty, and a
/// `Null` resume is the faithful "no payload" settle. A real capability (LANE-C)
/// supplies a value-bearing resolver via `CapabilityAsyncResolver` instead.
public struct DelayAsyncResolver: AsyncResolver {
    /// Seconds to wait before settling an otherwise-empty `Pending` cell.
    let delay: TimeInterval
    /// The suspend closure; defaults to `Task.sleep` but is injectable so tests
    /// can assert the wait without burning real time.
    let suspend: @Sendable (TimeInterval) async -> Void

    init(delay: TimeInterval = 0.05, suspend: @escaping @Sendable (TimeInterval) async -> Void = { try? await Task.sleep(nanoseconds: UInt64($0 * 1_000_000_000)) }) {
        self.delay = delay
        self.suspend = suspend
    }

    public func resolve(_ future: FluxValue) async -> FluxValue {
        await suspend(delay)
        // The cell id the handler parked on; a real capability-keyed resolver
        // would read it via the graph and return the capability's result. Here
        // the oracle's async stub leaves it empty, so settle to `Null`.
        return .null
    }
}

/// A key -> resolver map for capability-driven async (LANE-C extension point).
///
/// `CALL_CAP` returns a result-cell id; the executor parks on it. This registry
/// lets a live host register a resolver per capability-method key so a camera /
/// network / location call resolves through its real OS bridge. The resolver
/// receives the cell id and a read-only snapshot of the cell's current value.
public struct CapabilityAsyncResolver: AsyncResolver {
    /// A single capability's resolution strategy.
    typealias Resolve = @Sendable (UInt32, FluxValue) async -> FluxValue

    private let resolvers: [UInt32: Resolve]
    /// Used when no keyed resolver matches (or none are registered). The oracle's
    /// `(2,99)` async capability leaves the cell empty, so the default settles to
    /// `Null` after a real (tiny) suspension, mirroring `DelayAsyncResolver`.
    private let `default`: Resolve

    /// Builds a resolver from `(capId << 16 | methodId)` -> resolver closures, with
    /// an optional `default` used when no key matches.
    init(_ entries: [UInt32: Resolve] = [:], default: @escaping Resolve = { _, _ in
        try? await Task.sleep(nanoseconds: 1_000_000)
        return .null
    }) {
        self.resolvers = entries
        self.default = `default`
    }

    /// The composite key used to look a resolver up.
    static func key(capId: UInt32, methodId: UInt32) -> UInt32 {
        (capId << 16) | (methodId & 0xFFFF)
    }

    public func resolve(_ future: FluxValue) async -> FluxValue {
        guard case let .int(cellId) = future else { return .null }
        // LANE-C: the `(capId, methodId)` that owns `cellId` is looked up via the
        // graph's capability table; its keyed resolver runs the real OS bridge
        // (camera/network/location) and returns the settled value. The key is not
        // derivable from the cell id here, so the live host registers resolvers
        // keyed by capability and this entry point dispatches by key. Until that
        // wiring lands, the `default` resolver settles the (empty) reference cell.
        if let resolver = resolvers[UInt32(cellId)] {
            return await resolver(UInt32(cellId), future)
        }
        return await `default`(UInt32(cellId), future)
    }
}

/// Owns the signal graph, the adapter registry, the string table and the
/// reconciler, and applies decoded frames to them. Main-actor isolated so all
/// UIKit view mutations happen on the main actor after evaluation.
///
/// Conforms to `FluxExecutor`: native controls (via the kit's
/// `HandlerTarget`) call `dispatch(_:)` with a `FluxEvent`, which evaluates the
/// bound handler closure and reconciles the result.
@MainActor
public final class FluxExecutor: FluxUIKit.FluxExecutor {
    /// The live signal graph.
    private(set) var graph: SignalGraph
    /// Seeds a signal in the live graph (used by the reconciler to bind a
    /// `ForEach` row's `item` slot to each list element during expansion).
    internal func seedSignal(_ id: UInt32, _ value: FluxValue) {
        graph.write(id, value)
    }
    /// The reconciler driving the real UIKit views.
    private var reconciler: ShadowTreeReconciler
    /// The interned string table from the most recent Init frame.
    private(set) var table = MaterializationStringTable()
    /// The async-future resolver for `await` (ADR-0044). Defaults to the synchronous
    /// pass-through; a live host replaces it with a bridge to its real async
    /// capability surface (network/timer/etc.).
    public var asyncResolver: any AsyncResolver = PassthroughAsyncResolver()
    /// The OS-permission gate for `CALL_CAP` (FLUX-049). Defaults to
    /// `AllowAllPermissionChecker`; the production host injects a real OS-backed
    /// checker (`AuthorizationStatus`) so a denied grant surfaces as a red banner
    /// rather than a crash.
    public var permissionChecker: PermissionChecker = AllowAllPermissionChecker()
    /// The shared pending-`Http`-request store (FLUX-047); consulted by the
    /// default async resolver so a fetched cell resolves through the network.
    private let httpRequests = HttpRequestStore()
    /// The production `Http` transport (FLUX-047); `URLSessionHttpTransport`.
    private let httpTransport: HttpTransport = URLSessionHttpTransport()
    /// Handler id → (closure descriptor + bytecode blob + pre-decoded instruction
    /// stream). The decoded `[Instruction]` is produced once at registration (R3)
    /// and reused on every dispatch, so the per-tap hot path never re-decodes. A
    /// re-registration replaces the entry and invalidates the cache.
    private var handlerClosures: [UInt32: (closure: ClosureRef, bytecode: [UInt8], decoded: [Instruction]?)]
    /// The most recent full frame's node table, kept so handler dispatches can
    /// re-reconcile the affected views after a signal write.
    private var currentNodes: [UInt32: ShadowNode]
    /// The most recent full root id.
    private var currentRootId: UInt32?
    /// Monotonic counter for FLUX_FRAME_DUMP filenames.
    private var frameSequenceNumber: Int = 0
    /// The most recent VM error, surfaced to the UI overlay.
    public private(set) var lastError: VmError?
    /// The most recent server-side compile/type error, delivered via an `Error`
    /// (0x03) frame. `nil` until a recompile fails. Surfaced as a banner overlay
    /// while the last successfully-rendered tree stays on screen (Appendix E §E.6).
    public private(set) var serverError: ServerError?
    /// Resolved source-map paths (file id → path), refreshed each frame.
    private var sourcePaths: [UInt32: String] = [:]
    /// The handler currently being dispatched, for mapping a VM fault to its
    /// closure's source excerpt.
    private var currentHandlerId: UInt32?
    /// The unified error surfaced to the overlay (ADR-0057): collapses `lastError`
    /// (VM fault) and `serverError` (compile error) into one `FluxError` with an
    /// ADR-0057 source excerpt when available. `nil` when no fault is active.
    public var lastFluxError: FluxError? {
        if let server = serverError {
            var span: SourceSpan?
            if let s = server.span { span = SourceSpan(fileID: s.fileId, line: s.start, column: s.end) }
            var excerpt: FluxErrorExcerpt?
            if let ex = server.excerpt {
                let path = sourcePaths[ex.fileId] ?? "file \(ex.fileId)"
                excerpt = FluxErrorExcerpt(path: path, line: ex.line, column: ex.column, snippet: ex.snippet)
            }
            return FluxError(message: server.message, kind: .compile, span: span, excerpt: excerpt)
        }
        if let vm = lastError {
            var span: SourceSpan?
            var excerpt: FluxErrorExcerpt?
            if let closure = currentHandlerId.flatMap({ handlerClosures[$0]?.closure }) {
                span = SourceSpan(fileID: closure.span.fileId, line: closure.span.start, column: closure.span.end)
                if let ex = closure.excerpt {
                    let path = sourcePaths[ex.fileId] ?? "file \(ex.fileId)"
                    excerpt = FluxErrorExcerpt(path: path, line: ex.line, column: ex.column, snippet: ex.snippet)
                }
            }
            return FluxError(
                message: "\(vm.kind.name) at byte offset \(vm.offset)",
                kind: .vm,
                span: span,
                excerpt: excerpt
            )
        }
        return nil
    }
    /// The report from the most recent `dispatch`'s dirty-set reconcile (R1), for
    /// test assertions; empty when the dispatch touched no signal-dependent node.
    private(set) var lastReconcile: ReconcileReport = ReconcileReport()
    /// The root native view of the currently applied tree, or `nil` before any
    /// `Init` frame has built one. The host mounts this on screen.
    ///
    /// Delegates to the reconciler's own root tracking so a Delta that replaces
    /// the root (node ids unstable across edits) still presents the live root
    /// view rather than a stale, destroyed id.
    public var rootView: UIView? {
        reconciler.rootView as? UIView
    }
    /// Invoked on the main actor after a successful frame application or a
    /// dirty-set reconcile, so the host can mount/update the on-screen view
    /// without polling. Set by the mount controller; `nil` when no host is
    /// attached (e.g. headless tests).
    public var onTreeChanged: (@MainActor () -> Void)?

    /// Creates an executor backed by `graph` and an `AdapterRegistry` built from
    /// `table`.
    public init(graph: SignalGraph, registry: AdapterRegistry) {
        self.graph = graph
        self.table = MaterializationStringTable()
        self.reconciler = ShadowTreeReconciler(registry: registry, executor: nil, table: table)
        self.handlerClosures = [:]
        self.currentNodes = [:]
        self.currentRootId = nil
        self.lastError = nil
        self.reconciler.setExecutor(self)
        // FLUX-047: the default async resolver settles parked `Http` cells (cap 14)
        // through the real network using the live string table; non-Http futures
        // fall through to the pass-through (the cell id itself is settled).
        // The resolver is always invoked on the main actor (inside
        // `runHandlerAsync`), so reading the per-frame `table` via
        // `MainActor.assumeIsolated` is safe and yields the *current* table
        // (capturing it by value at init would hand the resolver a stale, empty
        // table and silently break URL/body id resolution on the live app).
        self.asyncResolver = HttpAsyncResolver(
            store: httpRequests,
            transport: httpTransport,
            tableProvider: { [weak self] in
                MainActor.assumeIsolated { self?.table ?? MaterializationStringTable() }
            }
        )
        // When DevTools connects, replay the current shadow tree so its component
        // tree populates immediately (FLUX-039: snapshot-on-connect).
        fluxDevtoolsOnConnect = { [weak self] in
            self?.reconciler.emitSnapshot()
        }
    }

    /// Applies a raw wire frame (decoding it first) and drives the reconciler.
    /// Convenience for the transport path: the app shell hands received bytes to
    /// this method instead of importing the decoder directly.
    /// - Throws: `WireError` on malformed input (the fault is also captured into
    ///   `lastError` so the UI can surface it).
    @discardableResult
    public func applyFrame(_ bytes: Data) throws -> [UInt32] {
        #if DEBUG
        // FLUX_FRAME_DUMP: writes the received frame bytes to a file for offline
        // inspection. Set the env var to a directory path before app launch.
        if let dumpDir = ProcessInfo.processInfo.environment["FLUX_FRAME_DUMP"] {
            let seq = frameSequenceNumber
            frameSequenceNumber += 1
            let path = (dumpDir as NSString).appendingPathComponent("frame_\(seq)_init.bin")
            do {
                try (bytes as Data).write(to: URL(fileURLWithPath: path))
                NSLog("[FluxRT] frame dump written to %@ (%d bytes)", path, bytes.count)
            } catch {
                NSLog("[FluxRT] frame dump FAILED: %@", error.localizedDescription)
            }
        }
        #endif
        let frame = try FrameDeserializer.decode([UInt8](bytes))
        return apply(frame)
    }

    /// Routes one received wire frame to the right consumer.
    ///
    /// A `StringInterned` reply (frame kind 0x08, brittleness 4c) is a no-op
    /// under the current local-interning model: the host never sends an
    /// `InternString` request, so a reply is unexpected and is dropped without
    /// reaching the tree. Any other frame kind is decoded and applied to the
    /// shadow tree.
    /// - Parameter data: the raw frame bytes from the transport.
    public func handleFrame(_ data: Data) {
        let bytes = [UInt8](data)
        #if DEBUG
        let kind: UInt8 = bytes.count > 5 ? bytes[5] : 0
        NSLog("[FluxRT] handleFrame: \(bytes.count) bytes, kind=\(kind)")
        #endif
        // A `StringInterned` reply (0x08) is irrelevant under local interning —
        // the host never issued an `InternString` request. Drop it so it cannot
        // disturb the live tree; every other frame is decoded + applied below.
        if bytes.count >= 6, bytes[5] == frameKindStringInterned {
            return
        }
        do {
            _ = try applyFrame(data)
        } catch {
            #if DEBUG
            NSLog("[frame] applyFrame threw: \(error)")
            #endif
        }
    }

    /// Applies an Init/full frame: seeds state, builds the string table, builds
    /// the view tree.
    /// - Returns: the node ids built or updated.
    @discardableResult
    func apply(_ frame: FluxFrame) -> [UInt32] {
        // Housekeeping frames (`Heartbeat`/`InternString`/`StringInterned`)
        // carry no tree data. Ignore them without disturbing the live tree or
        // wiping a previously-displayed fault (a heartbeat must never clear an
        // error banner).
        guard !frame.isControl else { return [] }
        // A server `Error` frame reports a failed recompile. Surface it as a
        // banner and keep the last good tree on screen; do not reconcile an
        // empty payload (that would blank the UI). Appendix E §E.6.
        if let serverError = frame.error {
            #if DEBUG
            NSLog("[FluxRT] apply: SERVER ERROR frame: \(serverError.message)")
            #endif
            self.serverError = serverError
            return []
        }
        lastError = nil
        serverError = nil
        sourcePaths = Dictionary(uniqueKeysWithValues: frame.files.map { ($0.fileId, $0.path) })
        for cell in frame.state { graph.seed(cell.signalId, cell.value) }
        for str in frame.strings { table.store(id: str.stringId, value: str.value) }
        if let root = frame.root {
            currentNodes = frame.nodes
            currentRootId = root.id
        } else if let liveRoot = currentRootId {
            // A patch frame (root == nil) may still replace the root node — this
            // happens whenever node ids are not stable across edits (the differ
            // then emits a `Replace` of the whole tree). The differ's
            // `emit_replace` puts the *new* node's id in `Patch::Replace.id`, so
            // the old root id is gone after application. Retarget `currentRootId`
            // (and merge every replaced/inserted node into `currentNodes`) to the
            // replaced/inserted node that is not a child of any other
            // replaced/inserted node — that is the new root. `currentNodes` must
            // carry the *whole* new subtree (not just the root) so the
            // dirty-set reconcile on a later tap (`reconcileDirty`) can walk it
            // and re-materialise the changed node; otherwise the tap would write
            // the signal but nothing would re-render (FR hot-reload + tap).
            var candidateRoots: Set<UInt32> = []
            var childIds: Set<UInt32> = []
            var replacedNodes: [UInt32: ShadowNode] = [:]
            for patch in frame.patches {
                switch patch {
                case let .replace(_, node), let .insert(_, _, node):
                    candidateRoots.insert(node.id)
                    replacedNodes[node.id] = node
                    for child in node.children {
                        if case let .node(cid) = child { childIds.insert(cid) }
                    }
                default:
                    break
                }
            }
            // Every replaced/inserted node enters `currentNodes` so the tap path
            // has a complete tree to reconcile against.
            for (nid, node) in replacedNodes {
                currentNodes[nid] = node
            }
            if let newRoot = candidateRoots.subtracting(childIds).first {
                currentRootId = newRoot
            }
        }
        // Register every handler carried by the frame so native controls can
        // fire them later (G1 — logic hot-swap). The handler section on the wire
        // ships each handler's bytecode alongside its id; without it a decoded
        // `HandlerDef` has no body to run, so we register only those that carry
        // one and surface the gap through `lastError` when a control fires an
        // unregistered id.
        for def in frame.handlers {
            if let bytecode = def.bytecode {
                registerHandler(def.handlerId, closure: def.closure, bytecode: bytecode)
            }
        }
        // Drive the reconciler unconditionally: a full frame (root != nil)
        // rebuilds the tree, while a patch frame (root == nil) applies only its
        // patches. The reconciler no-ops the tree build when root is absent.
        let report = reconciler.apply(frame)
        #if DEBUG
        NSLog("[FluxRT] apply: rootId=\(currentRootId.map { String($0) } ?? "nil") rootViewBuilt=\(reconciler.rootView != nil) built=\(report.built.count) updated=\(report.updated.count)")
        #endif
        // Signal the host that a new/updated native tree is ready to mount.
        onTreeChanged?()
        return report.built + report.updated
    }

    /// Evaluates a pre-decoded instruction stream against the live graph and
    /// returns the outcome (or the `VmError` that faulted). Does NOT reconcile —
    /// shared by both event dispatch and lifecycle hooks. Uses the concrete
    /// `SignalGraph` by reference (R3) so the dispatch hot path never boxes it into
    /// an `any SignalStore` existential. The VM interns derived strings (a
    /// `STR_CONCAT` result or a `TO_STRING` rendering) *locally* into the shared
    /// `MaterializationStringTable` it is passed (brittleness 4c), mirroring the
    /// Android host — no round-trip to the dev server — so the evaluation itself
    /// is fully synchronous.
    private func evaluate(
        instructions: [Instruction],
        payload: FluxValue
    ) -> (outcome: VmOutcome?, error: VmError?) {
        var store = graph
        do {
            let outcome = try FluxBytecodeVM.run(
                instructions,
                signals: &store,
                payload: payload,
                stringTable: table,
                capRegistry: .dev,
                permissions: permissionChecker
            )
            return (outcome, nil)
        } catch let err as VmError {
            return (nil, err)
        } catch {
            // Should be unreachable: run only throws VmError.
            return (nil, VmError(kind: .invalidDispatch, offset: 0))
        }
    }

    /// Runs a handler with resumable semantics, driving every `AWAIT` to completion
    /// (ADR-0044, MLP v2 async).
    ///
    /// Unlike `evaluate` (which runs the v1 non-suspending VM), this uses
    /// `FluxBytecodeVM.runResumable` and loops: on each `Suspended` continuation it
    /// reads the future handle from `state.futureReg`, asks `asyncResolver` to settle
    /// it (real async work), then `resume`s the handler with the value in `r0`. The
    /// loop terminates at `HALT`. Does NOT reconcile — the caller folds signals and
    /// reconciles, exactly like the v1 `dispatch` path.
    ///
    /// - Returns: the final `VmOutcome`, or the `VmError` that faulted.
    private func runHandlerAsync(
        bytecode: [UInt8],
        decoded: [Instruction]?,
        payload: FluxValue
    ) async -> (outcome: VmOutcome?, error: VmError?) {
        var store = graph
        // Use the registration-time decoded cache (R3) when present; otherwise
        // fall back to the raw-bytecode path (which also handles decode errors
        // the same way `dispatch(bytecode:)` does). This avoids re-decoding the
        // handler on every tap (dispatch hot path).
        var current: Result<RunResult, VmError>
        if let program = decoded {
            current = FluxBytecodeVM.runResumable(
                program,
                signals: &store,
                payload: payload,
                stringTable: table,
                capRegistry: .dev,
                permissions: permissionChecker,
                programBytes: bytecode
            )
        } else {
            current = FluxBytecodeVM.runResumable(
                bytecode,
                signals: &store,
                payload: payload,
                stringTable: table,
                capRegistry: .dev,
                permissions: permissionChecker
            )
        }
        while true {
            switch current {
            case let .success(.halt(outcome)):
                graph = store
                return (outcome, nil)
            case let .success(.suspended(state)):
                // Unified sync/async bridge (ADR-0045): `futureReg` holds the
                // result-cell id returned by `CALL_CAP`. If the cell is `ready`
                // (sync cap) its value is already settled; if `pending` (async
                // cap) the executor resolves the real future through `asyncResolver`,
                // settles the cell, then resumes. `error` cells resolve to `null`.
                let cellId: UInt32
                if case let .int(id) = state.registers[Int(state.futureReg)] {
                    cellId = UInt32(id)
                } else {
                    return (nil, VmError(kind: .typeMismatch, offset: Int(state.resumeOffset)))
                }
                let resolved: FluxValue
                switch store.cellState(cellId) {
                case .ready:
                    resolved = store.read(cellId) ?? .null
                case .pending:
                    let settled = await asyncResolver.resolve(.int(Int64(cellId)))
                    store.resolveCell(cellId, settled)
                    resolved = settled
                case .error:
                    resolved = .null
                }
                current = FluxBytecodeVM.resume(
                    state,
                    signals: &store,
                    value: resolved,
                    stringTable: table,
                    capRegistry: .dev,
                    permissions: permissionChecker
                )
            case let .failure(err):
                return (nil, err)
            }
        }
    }

    /// Evaluates a handler closure against the current graph. Mirrors
    /// `flux-vm-ref`: runs the bytecode and folds the written signals back into
    /// the graph. It does NOT re-reconcile — that is driven by the dispatch path
    /// via the dirty-set (R1) so only signal-dependent subtrees are touched.
    ///
    /// This overload decodes `bytecode` on the fly (used by callers/tests that
    /// hold raw bytes, e.g. conformance vectors); the `dispatch(event:)` hot path
    /// uses `dispatch(instructions:payload:)` with the registration-time cache (R3).
    /// - Returns: a `DispatchResult` describing what changed (signals written).
    func dispatch(
        bytecode: [UInt8],
        closure: ClosureRef,
        payload: FluxValue
    ) -> DispatchResult {
        let instructions = (try? Instruction.decode(bytecode)) ?? []
        return dispatch(instructions: instructions, payload: payload)
    }

    /// Evaluates a pre-decoded instruction stream (R3) against the current graph.
    /// - Returns: a `DispatchResult` describing what changed (signals written).
    func dispatch(
        instructions: [Instruction],
        payload: FluxValue
    ) -> DispatchResult {
        let (outcome, error) = evaluate(instructions: instructions, payload: payload)
        guard let outcome else {
            #if DEBUG
            NSLog("[executor] dispatch/evaluate FAILED: \(String(describing: error))")
            #endif
            lastError = error
            return DispatchResult(builtOrUpdated: [], signals: [], error: error)
        }
        // Fold VM-written signals back into the live graph.
        let written = outcome.signals
        for (id, value) in written { graph.write(id, value) }
        return DispatchResult(
            builtOrUpdated: [],
            signals: written,
            error: nil
        )
    }

    /// `FluxExecutor` entry point: evaluate the handler bound to
    /// `event.handlerId` against the current signal graph, then reconcile only the
    /// dirty subset of the tree (Perf R1).
    ///
    /// The event is always delivered on the main actor (the kit's
    /// `HandlerTarget` asserts isolation), so dispatching a handler — which
    /// evaluates bytecode and mutates UIKit views — is safe here. The byte-VM
    /// evaluation is synchronous: the VM interns derived strings locally into the
    /// shared string table (brittleness 4c), never round-tripping to the dev
    /// server. Only the native event payload conversion (`toRuntime`) is `async`,
    /// because it interns event strings through the dev server's `InternString`
    /// RPC; that runs off the synchronous entry point inside a `Task`, so the
    /// kit's `dispatch` call returns immediately and never blocks the UI thread.
    public func dispatch(_ event: FluxEvent) {
        #if DEBUG
        let payloadDesc = event.payload.map { "\($0)" } ?? "nil"
        NSLog("[fluxdbg:dispatch] handlerId=\(event.handlerId) nodeId=\(event.nodeId) payload=\(payloadDesc)")
        #endif
        guard let entry = handlerClosures[event.handlerId] else {
            lastError = VmError(kind: .invalidDispatch, offset: 0)
            lastReconcile = ReconcileReport()
            return
        }
        // For a handler inside an expanded ForEach row, re-seed the shared
        // `itemSlot` with that row's element before the VM reads it. Every
        // derived row shares the same slot id; without this the slot holds
        // the last row's value and `tasks.remove(task)` removes the wrong
        // element (last instead of tapped). The reconciler tracks the per-row
        // (slot, element) for every derived node id.
        if let ctx = reconciler.itemContext(for: event.nodeId) {
            #if DEBUG
            NSLog("[FluxRT] dispatch: seeding ForEach itemSlot \(ctx.slot) with element for node \(event.nodeId)")
            #endif
            graph.write(ctx.slot, ctx.element)
        }
        currentHandlerId = event.handlerId
        // Hand the raw `entry.bytecode` AND the registration-time decoded cache
        // to `runHandlerAsync`, which uses the cache (R3) so the handler is not
        // re-decoded on every tap; it falls back to re-decoding the raw bytes
        // only when the cache is absent.
        Task { @MainActor in
            // Convert the native event payload to the runtime's id-based value,
            // interning any resolved string locally into the shared table — no
            // round-trip to the dev server (brittleness 4c).
            let payload: FluxValue = if let kitPayload = event.payload {
                toRuntime(kitPayload, table: &table)
            } else {
                .null
            }
            #if DEBUG
            NSLog("[fluxdbg:dispatch] payload->runtime value=\(payload)")
            #endif
            // Run the handler with resumable semantics (ADR-0044): every `AWAIT`
            // is settled by `asyncResolver` and the handler is resumed until `HALT`.
            // Pass the cached decoded instructions (R3) so the handler is not
            // re-decoded on every tap.
            let (outcome, error) = await runHandlerAsync(
                bytecode: entry.bytecode,
                decoded: entry.decoded,
                payload: payload
            )
            guard let outcome else {
                #if DEBUG
                NSLog("[executor] async dispatch FAILED: \(String(describing: error))")
                #endif
                lastError = error
                lastReconcile = ReconcileReport()
                onTreeChanged?()
                return
            }
            // Fold VM-written signals back into the live graph (runHandlerAsync has
            // already committed its working copy, but folding keeps `graph` and the
            // reconcile in lockstep for any observer that read mid-flight).
            let written = outcome.signals
            for (id, value) in written { graph.write(id, value) }
            #if DEBUG
            let writtenDesc = written.map { "S\($0.0)=\(FluxExecutor.describe($0.1, table: table))" }.joined(separator: ", ")
            NSLog("[FluxRT] dispatch wrote signals: [ \(writtenDesc) ] currentRootId=\(currentRootId.map { String($0) } ?? "nil")")
            #endif
            // R1: re-reconcile only the nodes whose signal dependencies were just
            // written, instead of re-walking the whole tree on every tap.
            let dirty = Set(written.map { $0.0 })
            if !dirty.isEmpty, let rootId = currentRootId {
                let report = reconciler.reconcileDirty(rootId: rootId, signalIds: dirty)
                #if DEBUG
                NSLog("[FluxRT] dispatch reconcileDirty: dirty=\(dirty) built=\(report.built.count) updated=\(report.updated.count) detached=\(report.detached.count)")
                #endif
                lastReconcile = report
            } else {
                #if DEBUG
                UserDefaults.standard.set("[dispatch] rootId=\(currentRootId.map { String($0) } ?? "nil") dirty=[] (no signals written) built=[] updated=[] detached=[]\n", forKey: "flux_dispatch")
                #endif
                lastReconcile = ReconcileReport()
            }
            // A signal-dependent reconcile may have re-parented native views; the
            // host should re-present the (unchanged-identity) root view.
            onTreeChanged?()
        }
    }

        // MARK: - Debug instrumentation (Phase 0)

    /// Human-readable description of a `FluxValue`, resolving interned string
    /// ids through the shared string `table` so signal writes can be inspected
    /// in `NSLog` output without manual id→text lookup.
    ///
    /// - Parameter value: the runtime value to describe.
    /// - Parameter table: the shared string table used for resolution.
    /// - Returns: a debug string with resolved string text; unresolved ids are
    ///   shown as `<unresolved:id>` so they are immediately visible.
    @MainActor
    static func describe(_ value: FluxValue, table: MaterializationStringTable) -> String {
        switch value {
        case .null:
            return "null"
        case .int(let n):
            return "\(n)"
        case .float(let f):
            return "\(f)"
        case .bool(let b):
            return "\(b)"
        case .str(let id):
            let resolved = table.lookup(id)
            return "\"\(resolved ?? "<unresolved:0x\(String(id, radix: 16))>")\""
        case .handlerRef(let h):
            return "handler(\(h))"
        case .list(let items):
            let elems = items.map { describe($0, table: table) }.joined(separator: ", ")
            return "[ \(elems) ]"
        case .record(let fields):
            let flds = fields.map { "\($0.propIndex): \(describe($0.value, table: table))" }.joined(separator: ", ")
            return "{ \(flds) }"
        }
    }

    /// The built native view for a node id, for test assertions (real UIKit
    /// views, not mocks).
    func view(for nodeId: UInt32) -> AnyObject? {
        reconciler.view(for: nodeId)
    }

    /// Registers handler bytecode so native controls can fire it later. The
    /// bytecode is decoded once into `[Instruction]` and cached (R3) so dispatch
    /// never re-decodes; re-registering invalidates the prior cache entry.
    func registerHandler(_ id: UInt32, closure: ClosureRef, bytecode: [UInt8]) {
        let decoded = try? Instruction.decode(bytecode)
        handlerClosures[id] = (closure: closure, bytecode: bytecode, decoded: decoded)
    }

    /// Evaluates a lifecycle handler (e.g. `onMount`/`onCleanup`, §18.4) by its
    /// handler id, without a native event payload. Used by the reconciler when a
    /// node is created or removed. A missing or unregistered id is a no-op
    /// (lifecycle blocks are optional); a VM fault is captured into `lastError`
    /// like any other dispatch.
    ///
    /// Crucially this does **not** re-reconcile: it is invoked *from within* a
    /// `reconciler.apply` pass, so re-entering the reconciler would be a
    /// re-entrant `inout self` access (a fatal "access conflict" in Swift's
    /// exclusive-access model). It folds any signal writes back into the graph
    /// (a mount/cleanup block may seed state) and returns.
    func runLifecycle(_ handlerId: UInt32) {
        currentHandlerId = handlerId
        guard let entry = handlerClosures[handlerId] else {
            // No body registered for this lifecycle hook; nothing to run.
            return
        }
        // Reuse the cached decode (R3); fall back to a one-off decode if needed.
        let instructions = entry.decoded ?? (try? Instruction.decode(entry.bytecode)) ?? []
        let (outcome, error) = evaluate(instructions: instructions, payload: .null)
        if let error { lastError = error; return }
        guard let outcome else { return }
        // Fold written signals back into the live graph without re-reconciling.
        for (id, value) in outcome.signals { graph.write(id, value) }
    }
}
