//  FluxRuntime.swift
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
import FluxUIKit

/// The outcome of dispatching one frame.
struct DispatchResult: Sendable {
    /// Node ids whose views were (re)built or updated by the reconciler.
    let builtOrUpdated: [UInt32]
    /// The signals written by handler evaluation, sorted by id.
    let signals: [(UInt32, VMValue)]
    /// `nil` on success, or the VM error that occurred.
    let error: VMError?
}

/// Owns the signal graph, the adapter registry, the string table and the
/// reconciler, and applies decoded frames to them. Main-actor isolated so all
/// UIKit view mutations happen on the main actor after evaluation.
///
/// Conforms to `FluxRuntime`: native controls (via the kit's
/// `HandlerTarget`) call `dispatch(_:)` with a `FluxEvent`, which evaluates the
/// bound handler closure and reconciles the result.
@MainActor
final class FluxRuntime: FluxExecutor {
    /// The live signal graph.
    private(set) var graph: SignalGraph
    /// The reconciler driving the real UIKit views.
    private var reconciler: ShadowTreeReconciler
    /// The interned string table from the most recent Init frame.
    private(set) var table: StringTable
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
    /// The most recent VM error, surfaced to the UI overlay.
    private(set) var lastError: VMError?
    /// The report from the most recent `dispatch`'s dirty-set reconcile (R1), for
    /// test assertions; empty when the dispatch touched no signal-dependent node.
    private(set) var lastReconcile: ReconcileReport = ReconcileReport()

    /// Creates an executor backed by `graph` and an `AdapterRegistry` built from
    /// `table`.
    init(graph: SignalGraph, registry: AdapterRegistry) {
        self.graph = graph
        self.table = StringTable()
        self.reconciler = ShadowTreeReconciler(registry: registry, executor: nil)
        self.handlerClosures = [:]
        self.currentNodes = [:]
        self.currentRootId = nil
        self.lastError = nil
        self.reconciler.setExecutor(self)
    }

    /// Applies an Init/full frame: seeds state, builds the string table, builds
    /// the view tree.
    /// - Returns: the node ids built or updated.
    @discardableResult
    func apply(_ frame: FluxFrame) -> [UInt32] {
        lastError = nil
        for cell in frame.state { graph.seed(cell.signalId, cell.value) }
        for str in frame.strings { table.intern(str.stringId, str.value) }
        if let root = frame.root {
            currentNodes = frame.nodes
            currentRootId = root.id
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
        return report.built + report.updated
    }

    /// Evaluates a pre-decoded instruction stream against the live graph and
    /// returns the outcome (or the `VMError` that faulted). Does NOT reconcile —
    /// shared by both event dispatch and lifecycle hooks. Uses the concrete
    /// `SignalGraph` by reference (R3) so the dispatch hot path never boxes it into
    /// an `any SignalStore` existential.
    private func evaluate(
        instructions: [Instruction],
        payload: VMValue
    ) -> (outcome: VmOutcome?, error: VMError?) {
        var store = graph
        do {
            let outcome = try FluxBytecodeVM.run(
                instructions,
                signals: &store,
                payload: payload,
                stringTable: table,
                capRegistry: .dev
            )
            return (outcome, nil)
        } catch let err as VMError {
            return (nil, err)
        } catch {
            // Should be unreachable: run only throws VMError.
            return (nil, VMError(kind: .invalidDispatch, offset: 0))
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
        payload: VMValue
    ) -> DispatchResult {
        let instructions = (try? Instruction.decode(bytecode)) ?? []
        return dispatch(instructions: instructions, payload: payload)
    }

    /// Evaluates a pre-decoded instruction stream (R3) against the current graph.
    /// - Returns: a `DispatchResult` describing what changed (signals written).
    func dispatch(
        instructions: [Instruction],
        payload: VMValue
    ) -> DispatchResult {
        let (outcome, error) = evaluate(instructions: instructions, payload: payload)
        guard let outcome else {
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

    /// `FluxRuntime` entry point: evaluate the handler bound to
    /// `event.handlerId` against the current signal graph, then reconcile only the
    /// dirty subset of the tree (Perf R1).
    ///
    /// The event is always delivered on the main actor (the kit's
    /// `HandlerTarget` asserts isolation), so dispatching a handler — which
    /// evaluates bytecode and mutates UIKit views — is safe here.
    func dispatch(_ event: FluxEvent) {
        guard let entry = handlerClosures[event.handlerId] else {
            lastError = VMError(kind: .invalidDispatch, offset: 0)
            lastReconcile = ReconcileReport()
            return
        }
        // Reuse the cached decode from registration (R3); fall back to a one-off
        // decode only if caching was impossible.
        let instructions = entry.decoded ?? (try? Instruction.decode(entry.bytecode)) ?? []
        let payload: VMValue = event.payload.map { toRuntime($0, table: &table) } ?? .null
        let result = dispatch(instructions: instructions, payload: payload)
        // R1: re-reconcile only the nodes whose signal dependencies were just
        // written, instead of re-walking the whole tree on every tap.
        let dirty = Set(result.signals.map { $0.0 })
        if !dirty.isEmpty, let rootId = currentRootId {
            let report = reconciler.reconcileDirty(rootId: rootId, nodes: currentNodes, signalIds: dirty)
            lastReconcile = report
        } else {
            lastReconcile = ReconcileReport()
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
