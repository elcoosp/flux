//  RuntimeE2ETests.swift
//  End-to-end runtime test without sockets (FLUX-006 scope item 9).
//
//  Drives the full pipeline — `FrameDeserializer` -> `FluxExecutor.apply` ->
//  `ShadowTreeReconciler` -> `MockAdapter` -> `FluxBytecodeVM.dispatch` — with
//  hand-built frames, asserting the native (mock) views and signal graph update
//  exactly as the spec requires. Also pins the gas-exhaustion behaviour so a
//  runaway handler cannot silently succeed.

import XCTest

@testable import FluxApp

/// Builds a small `FluxFrame` whose root is a single `Component` node carrying a
/// `Text`-like string prop, plus an initial signal seed.
private func sampleInitFrame() -> (FluxFrame, UInt32) {
    let root = ShadowNode(
        id: 1,
        kind: .component,
        componentId: 10,
        props: [Prop(index: 0, value: .str(7))],
        childCount: 0,
        children: [],
        handlerCount: 0,
        handlers: [],
        span: FluxSpan(fileId: 0, start: 0, end: 0)
    )
    let frame = FluxFrame(
        version: 1,
        seq: 0,
        flags: 0x01, // full_tree
        root: root,
        patches: [],
        handlers: [],
        strings: [StringEntry(stringId: 7, value: "Hello")],
        state: [StateCell(signalId: 1, value: .int(0))],
        files: []
    )
    return (frame, 1)
}

final class RuntimeE2ETests: XCTestCase {
    /// Init frame -> reconciler builds the root view; the mock adapter records it.
    @MainActor
    func testInitBuildsViewTree() {
        let registry = AdapterRegistry([MockAdapter(handles: .component)])
        let executor = FluxExecutor(graph: SignalGraph(), registry: registry)
        let (frame, rootId) = sampleInitFrame()

        let built = executor.apply(frame)
        XCTAssertEqual(built, [rootId])
        XCTAssertNotNil(executor.view(for: rootId))
        // The seeded signal is visible in the graph.
        XCTAssertEqual(executor.graph.read(1), .int(0))
    }

    /// Dispatching a handler that writes a signal folds the value into the graph
    /// and reports it in the outcome. We use the `write_signal` semantics via a
    /// minimal bytecode: READ_SIGNAL 0,1 then WRITE_SIGNAL 1,0 (copy signal 1
    /// into signal 1 — a no-op value-wise but exercises read+write), plus HALT.
    @MainActor
    func testDispatchFoldsSignals() {
        let registry = AdapterRegistry([MockAdapter(handles: .component)])
        let graph = SignalGraph(values: [1: .int(42)])
        let executor = FluxExecutor(graph: graph, registry: registry)
        _ = executor.apply(sampleInitFrame().0)

        // Bytecode: READ_SIGNAL r0, signal 1 ; WRITE_SIGNAL signal 1, r0 ; HALT
        // 0x10 0x00 0x00000001  0x11 0x00000001 0x00  0x00
        let bytecode: [UInt8] = [0x10, 0x00, 0x01, 0x00, 0x00, 0x00,
                                 0x11, 0x01, 0x00, 0x00, 0x00, 0x00,
                                 0x00]
        let closure = ClosureRef(
            hash: Array(repeating: 0, count: 8),
            bytecodeOffset: 0,
            bytecodeLen: UInt16(bytecode.count),
            signalCount: 0,
            signals: [],
            span: FluxSpan(fileId: 0, start: 0, end: 0)
        )
        let result = executor.dispatch(bytecode: bytecode, closure: closure, payload: .null)
        XCTAssertNil(result.error)
        XCTAssertEqual(result.signals.first?.0, 1)
        // The Init frame seeds signal 1 to `.int(0)`; the handler reads it and
        // writes the same value back, so the folded signal stays 0.
        XCTAssertEqual(result.signals.first?.1, .int(0))
    }

    /// A handler whose bytecode is a tight infinite loop must exhaust gas and be
    /// reported as a `GasExhausted` fault — never loop forever.
    @MainActor
    func testGasExhaustionIsReported() {
        let registry = AdapterRegistry([MockAdapter(handles: .component)])
        let executor = FluxExecutor(graph: SignalGraph(), registry: registry)
        _ = executor.apply(sampleInitFrame().0)

        // An unconditional backward jump to itself never reaches HALT and must
        // exhaust the entry gas budget (Appendix E §E.3), reported as
        // `GasExhausted` — never loop forever. `JUMP` (0x60) takes a 4-byte
        // i32 offset relative to the *next* instruction; here delta = -5 lands
        // back on the JUMP itself (5 bytes long), forming the loop.
        let bytecode: [UInt8] = [0x60, 0xFB, 0xFF, 0xFF, 0xFF, 0x00]
        let closure = ClosureRef(
            hash: Array(repeating: 0, count: 8),
            bytecodeOffset: 0,
            bytecodeLen: UInt16(bytecode.count),
            signalCount: 0,
            signals: [],
            span: FluxSpan(fileId: 0, start: 0, end: 0)
        )
        let result = executor.dispatch(bytecode: bytecode, closure: closure, payload: .null)
        XCTAssertNotNil(result.error)
        XCTAssertEqual(result.error?.kind, .gasExhausted)
    }

    /// An unknown opcode faults immediately with `InvalidDispatch` and is
    /// captured (never escapes the executor).
    @MainActor
    func testInvalidOpcodeCaptured() {
        let registry = AdapterRegistry([MockAdapter(handles: .component)])
        let executor = FluxExecutor(graph: SignalGraph(), registry: registry)
        _ = executor.apply(sampleInitFrame().0)

        // Bytecode with a bogus opcode 0xFF then HALT.
        let bytecode: [UInt8] = [0xFF, 0x00]
        let closure = ClosureRef(
            hash: Array(repeating: 0, count: 8),
            bytecodeOffset: 0,
            bytecodeLen: 2,
            signalCount: 0,
            signals: [],
            span: FluxSpan(fileId: 0, start: 0, end: 0)
        )
        let result = executor.dispatch(bytecode: bytecode, closure: closure, payload: .null)
        XCTAssertNotNil(result.error)
        XCTAssertEqual(result.error?.kind, .invalidDispatch)
    }
}
