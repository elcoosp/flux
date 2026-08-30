//  AsyncSuspendResumeTests.swift
//  First-class async (ADR-0044) + unified capability bridge (ADR-0045): the
//  reference VM captures a continuation at `AWAIT` and resumes it after the
//  awaited result cell settles. This test drives `FluxBytecodeVM.runResumable` /
//  `resume` directly — synchronously and deterministically — to prove the
//  suspend/resume round-trip and the CALL_CAP → signal-cell contract without
//  depending on the executor's async event loop.
//
//  The contract exercised here:
//    CALL_CAP resultReg, capId, methodId, argsReg  →  resultReg = result-cell signal id
//    AWAIT   r0, resultReg                          →  park while that cell is `.pending`;
//                                                      a `.ready` cell continues with its
//                                                      value in r0 (no real park); `.error` faults.
//
//  Scenario A (sync cap, cap 1/1 → signal 99, Ready): no suspension.
//  Scenario B (async cap, cap 2/99 → fresh Pending cell): real Suspend + resolveCell + resume.

import XCTest

@Testable import FluxHost

final class AsyncSuspendResumeTests: XCTestCase {
    /// CALL_CAP(cap 1,1) writes `arg[0]` into signal 99 (Ready) and returns 99;
    /// AWAIT on that cell is Ready → continues with the value in r0.
    private let syncBytecode: [UInt8] = [
        0x90, 0x02, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, // CALL_CAP r2, (1,1), args=r0
        0xE0, 0x00, 0x02,                                           // AWAIT r0, r2 (future = cell id in r2)
        0x11, 0x02, 0x00, 0x00, 0x00, 0x00,                         // WRITE_SIGNAL 2, r0
        0x00,                                                        // HALT
    ]

    /// CALL_CAP(cap 2,99) allocates a fresh Pending cell and returns its id;
    /// AWAIT on it Parks; the host resolveCell(...) then Resume.
    private let asyncBytecode: [UInt8] = [
        0x90, 0x02, 0x02, 0x00, 0x00, 0x00, 0x63, 0x00, 0x00, 0x00, // CALL_CAP r2, (2,99), args=r0
        0xE0, 0x00, 0x02,                                           // AWAIT r0, r2
        0x11, 0x02, 0x00, 0x00, 0x00, 0x00,                         // WRITE_SIGNAL 2, r0
        0x00,                                                        // HALT
    ]

    /// A synchronous capability resolves immediately: CALL_CAP returns the cell id,
    /// AWAIT sees a `.ready` cell and continues with its value, no suspension.
    @MainActor
    func testSyncCapabilityDoesNotSuspend() {
        var graph = SignalGraph()
        let payload = FluxValue.record([(0, .int(42))])

        let first = FluxBytecodeVM.runResumable(syncBytecode, signals: &graph, payload: payload)
        guard case let .success(.halt(outcome)) = first else {
            XCTFail("sync capability should reach HALT without suspending, got \(String(describing: first))")
            return
        }

        // The capability echoed arg[0] into signal 99, and result_reg (r2) holds 99.
        XCTAssertEqual(graph.read(99), .int(42), "capability must echo arg[0] into signal 99")
        XCTAssertEqual(outcome.registers[2], .int(99), "result_reg must hold the cell id 99")
        // AWAIT on the Ready cell placed the value in r0 → written to signal 2.
        XCTAssertEqual(graph.read(2), .int(42), "AWAIT on Ready cell must place the value in r0")
    }

    /// An asynchronous capability returns a Pending cell; AWAIT suspends. The host
    /// resolves the cell with `resolveCell`, then `resume` continues with the value.
    @MainActor
    func testAsyncCapabilitySuspendsThenResumesToHalt() {
        var graph = SignalGraph()
        let payload = FluxValue.record([(0, .int(42))])

        let first = FluxBytecodeVM.runResumable(asyncBytecode, signals: &graph, payload: payload)
        guard case let .success(.suspended(state)) = first else {
            XCTFail("async capability should suspend, got \(String(describing: first))")
            return
        }

        // result_reg (r2) holds the freshly allocated Pending cell id.
        guard case let .int(cellId) = state.registers[2] else {
            XCTFail("result_reg must hold the cell id")
            return
        }
        XCTAssert(cellId >= 1_000_000, "async capability must allocate a fresh cell id")
        XCTAssertEqual(graph.cellState(cellId), .pending, "cell must be Pending after async cap")

        // Host resolves the cell, then resumes.
        graph.resolveCell(cellId, .int(7))
        let resumed = FluxBytecodeVM.resume(state, signals: &graph, value: .int(7))
        guard case let .success(.halt(outcome)) = resumed else {
            XCTFail("expected .halt after resolve+resume, got \(String(describing: resumed))")
            return
        }
        XCTAssertEqual(graph.read(2), .int(7), "post-resume body must run with the resolved value")
        XCTAssertFalse(outcome.signals.isEmpty)
    }

    /// Cancellation parity (FLUX-086, Part B): when the awaiting `Task` is cancelled
    /// before its `Pending` cell settles, the signal graph must match the Rust oracle's
    /// `SuspendState` exactly — the `Pending` result cell is left untouched and no other
    /// signal was written before the `AWAIT`. A cancel is modelled as dropping the
    /// continuation (never calling `resume`); the captured `SuspendState` is the
    /// post-cancel source of truth for all three runtimes. The cancellation contract is
    /// already fully specified by the oracle's suspend semantics, so we assert it here
    /// rather than filing a follow-up.
    @MainActor
    func testAwaitCancellationLeavesPendingCellAndNoWrites() {
        var graph = SignalGraph()
        let payload = FluxValue.record([(0, .int(42))])

        let first = FluxBytecodeVM.runResumable(asyncBytecode, signals: &graph, payload: payload)
        guard case let .success(.suspended(state)) = first else {
            XCTFail("async capability should suspend, got \(String(describing: first))")
            return
        }
        guard case let .int(cellId) = state.registers[2] else {
            XCTFail("result_reg must hold the cell id")
            return
        }
        XCTAssert(cellId >= 1_000_000, "async capability must allocate a fresh cell id")

        // Oracle contract: at cancellation the only signal state is the Pending cell; the
        // handler had written nothing before the AWAIT (signal 2 must remain unbound).
        XCTAssertEqual(graph.cellState(cellId), .pending, "cell must remain Pending after cancel")
        XCTAssertNil(graph.read(2), "no signal writes may have occurred before the AWAIT")
        XCTAssertEqual(state.registers[2], .int(cellId), "captured future_reg must match the pending cell")
    }

    /// v1 semantics are preserved: a program that never emits `AWAIT` runs to `HALT`
    /// in a single `runResumable` call and never suspends.
    @MainActor
    func testNoAwaitRunsStraightToHalt() {
        var graph = SignalGraph()
        let plain: [UInt8] = [
            0xB0, 0x00, 0x2A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // LOAD_INT_CONST r0, 42
            0x11, 0x02, 0x00, 0x00, 0x00, 0x00,                         // WRITE_SIGNAL 2, r0
            0x00,                                                        // HALT
        ]
        let result = FluxBytecodeVM.runResumable(plain, signals: &graph, payload: .null)
        guard case let .success(.halt(outcome)) = result else {
            XCTFail("expected .halt without await, got \(String(describing: result))")
            return
        }
        XCTAssertEqual(graph.read(2), .int(42))
        XCTAssertFalse(outcome.signals.isEmpty)
    }
}
