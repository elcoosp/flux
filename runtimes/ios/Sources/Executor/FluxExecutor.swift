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
    /// Handler id → (closure descriptor + bytecode blob), registered by
    /// Init/delta frames so native controls can fire them later.
    private var handlerClosures: [UInt32: (closure: ClosureRef, bytecode: [UInt8])]
    /// The most recent full frame's node table, kept so handler dispatches can
    /// re-reconcile the affected views after a signal write.
    private var currentNodes: [UInt32: ShadowNode]
    /// The most recent full root id.
    private var currentRootId: UInt32?
    /// The most recent VM error, surfaced to the UI overlay.
    private(set) var lastError: VMError?

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

    /// Evaluates a handler closure against the current graph. Mirrors
    /// `flux-vm-ref`: runs the bytecode, then folds the written signals back into
    /// the graph and reconciles the affected views.
    /// - Returns: a `DispatchResult` describing what changed.
    func dispatch(
        bytecode: [UInt8],
        closure: ClosureRef,
        payload: VMValue
    ) -> DispatchResult {
        var store: any SignalStore = graph
        let outcome: VmOutcome
        do {
            outcome = try FluxBytecodeVM.run(
                bytecode,
                signals: &store,
                payload: payload,
                stringTable: table,
                capRegistry: .dev
            )
        } catch let err as VMError {
            lastError = err
            return DispatchResult(builtOrUpdated: [], signals: [], error: err)
        } catch {
            // Should be unreachable: run only throws VMError.
            lastError = VMError(kind: .invalidDispatch, offset: 0)
            return DispatchResult(builtOrUpdated: [], signals: [], error: lastError)
        }
        // Fold VM-written signals back into the live graph.
        let written = outcome.signals
        for (id, value) in written { graph.write(id, value) }
        // Re-reconcile the current tree so any view whose props read a changed
        // signal is updated in place (never recreated).
        let report = reconciler.apply(currentFrame())
        return DispatchResult(
            builtOrUpdated: report.built + report.updated,
            signals: written,
            error: nil
        )
    }

    /// `FluxRuntime` entry point: evaluate the handler bound to
    /// `event.handlerId` against the current signal graph, then reconcile.
    ///
    /// The event is always delivered on the main actor (the kit's
    /// `HandlerTarget` asserts isolation), so dispatching a handler — which
    /// evaluates bytecode and mutates UIKit views — is safe here.
    func dispatch(_ event: FluxEvent) {
        guard let (closure, bytecode) = handlerClosures[event.handlerId] else {
            lastError = VMError(kind: .invalidDispatch, offset: 0)
            return
        }
        let payload: VMValue = event.payload.map { toRuntime($0, table: &table) } ?? .null
        _ = dispatch(bytecode: bytecode, closure: closure, payload: payload)
    }

    /// The built native view for a node id, for test assertions (real UIKit
    /// views, not mocks).
    func view(for nodeId: UInt32) -> AnyObject? {
        reconciler.view(for: nodeId)
    }

    /// Registers handler bytecode so native controls can fire it later.
    func registerHandler(_ id: UInt32, closure: ClosureRef, bytecode: [UInt8]) {
        handlerClosures[id] = (closure: closure, bytecode: bytecode)
    }

    /// Evaluates a lifecycle handler (e.g. `onMount`/`onCleanup`, §18.4) by its
    /// handler id, without a native event payload. Used by the reconciler when a
    /// node is created or removed. A missing or unregistered id is a no-op
    /// (lifecycle blocks are optional); a VM fault is captured into `lastError`
    /// like any other dispatch.
    func runLifecycle(_ handlerId: UInt32) {
        guard let (_, bytecode) = handlerClosures[handlerId] else {
            // No body registered for this lifecycle hook; nothing to run.
            return
        }
        _ = dispatch(bytecode: bytecode, closure: ClosureRef(
            hash: [], bytecodeOffset: 0, bytecodeLen: UInt16(bytecode.count),
            signalCount: 0, signals: [], span: FluxSpan(fileId: 0, start: 0, end: 0)
        ), payload: .null)
    }

    /// Reconstructs a frame carrying the last full tree so the reconciler can
    /// re-apply it after a signal write (without re-decoding).
    private func currentFrame() -> FluxFrame {
        let root = currentRootId.flatMap { currentNodes[$0] }
        return FluxFrame(
            version: 1, seq: 0, flags: 0,
            root: root, nodes: currentNodes,
            patches: [], handlers: [],
            strings: [], state: [],
            files: []
        )
    }
}
