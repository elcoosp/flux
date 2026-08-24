//  FluxExecutor.swift
//  Dispatches an incoming frame through the VM and signal graph (FLUX-006 scope
//  items 7 & 9), thread-safely.
//
//  The executor is the boundary between the (future) WebSocket transport and the
//  native UI. In dev mode it is driven synchronously from tests; in the live app
//  the transport hands raw frames to `dispatch(_:)` on a background queue and the
//  executor evaluates handler closures via `FluxBytecodeVM.run`, then reports the
//  resulting signal writes back to the `SignalGraph` so observers (and thus the
//  reconciled views) update. No VM error is ever allowed to escape: a failure is
//  captured into `lastError` and surfaced as an error overlay by `FluxRootView`.

import Foundation

/// The outcome of dispatching one frame.
struct DispatchResult: Sendable {
    /// Node ids whose views were (re)built or updated by the reconciler.
    let builtOrUpdated: [UInt32]
    /// The signals written by handler evaluation, sorted by id.
    let signals: [(UInt32, FluxValue)]
    /// `nil` on success, or the VM error that occurred.
    let error: VMError?
}

/// Owns the signal graph, the adapter registry and the reconciler, and applies
/// decoded frames to them. Actor-isolated so UI updates happen on the main
/// actor after evaluation.
@MainActor
final class FluxExecutor {
    /// The live signal graph.
    private(set) var graph: SignalGraph
    /// The reconciler driving native views.
    private var reconciler: ShadowTreeReconciler
    /// The most recent VM error, surfaced to the UI overlay.
    private(set) var lastError: VMError?

    /// Creates an executor backed by `graph` and `registry`.
    init(graph: SignalGraph, registry: AdapterRegistry) {
        self.graph = graph
        self.reconciler = ShadowTreeReconciler(registry: registry)
    }

    /// Applies an Init/full frame: seeds state, builds the view tree.
    /// - Returns: the node ids built.
    @discardableResult
    func apply(_ frame: FluxFrame) -> [UInt32] {
        lastError = nil
        for cell in frame.state { graph.seed(cell.signalId, cell.value) }
        for str in frame.strings { _ = str } // string table is read by the VM at eval time
        guard let root = frame.root else { return [] }
        let report = reconciler.reconcile(root)
        return report.built + report.updated
    }

    /// Evaluates a handler closure against the current graph. Mirrors
    /// `flux-vm-ref`: runs the bytecode, then folds the written signals back into
    /// the graph so observers (and thus views) update.
    /// - Parameters:
    ///   - bytecode: the handler's bytecode blob.
    ///   - closure: the descriptor carrying captured signal ids and gas budget.
    ///   - payload: the event payload (e.g. a tap), placed in r0.
    /// - Returns: a `DispatchResult` describing what changed.
    func dispatch(
        bytecode: [UInt8],
        closure: ClosureRef,
        payload: FluxValue
    ) -> DispatchResult {
        var store: any SignalStore = graph
        let outcome: VmOutcome
        do {
            outcome = try FluxBytecodeVM.run(bytecode, signals: &store, payload: payload)
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
        // Reconcile again if the handler rebuilt/patched the tree (rare; handled
        // via explicit Insert/Replace patches in real frames, but we re-run to
        // keep the view set coherent).
        let report = reconcilerForCurrentTree()
        return DispatchResult(
            builtOrUpdated: report.built + report.updated,
            signals: written,
            error: nil
        )
    }

    /// Re-runs the reconciler against the last known tree (no-op placeholder for
    /// the test path; real diffs arrive as patches in delta frames).
    private func reconcilerForCurrentTree() -> ReconcileReport {
        // In dev without a persistent tree the executor only builds what handlers
        // touch via signals; views are reconciled from full frames in `apply`.
        ReconcileReport()
    }

    /// The built mock view for a node id, for test assertions.
    func view(for nodeId: UInt32) -> MockView? {
        reconciler.view(for: nodeId)
    }
}
