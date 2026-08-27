//  AsyncSuspendResumeTests.swift
//  First-class async (ADR-0044): the reference VM captures a continuation at
//  `AWAIT` and resumes it after the awaited future settles. This test drives
//  `FluxBytecodeVM.runResumable` / `resume` directly — synchronously and
//  deterministically — to prove the suspend/resume round-trip without depending
//  on the executor's async event loop.
//
//  Handler bytecode under test:
//    LOAD_INT_CONST r0, 1      ; the "future" handle, also written to signal 1
//    WRITE_SIGNAL 1, r0
//    AWAIT r0, r0              ; park; executor reads future from r0, resumes
//    LOAD_INT_CONST r0, 42     ; post-resume body
//    WRITE_SIGNAL 2, r0
//    HALT

import XCTest

@testable import FluxHost

final class AsyncSuspendResumeTests: XCTestCase {
    /// Bytecode: see file header. `AWAIT` is `0xE0` with operands (result_reg, future_reg).
    private let awaitBytecode: [UInt8] = [
        0xB0, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // LOAD_INT_CONST r0, 1
        0x11, 0x01, 0x00, 0x00, 0x00, 0x00,                         // WRITE_SIGNAL 1, r0
        0xE0, 0x00, 0x00,                                           // AWAIT r0, r0
        0xB0, 0x00, 0x2A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // LOAD_INT_CONST r0, 42
        0x11, 0x02, 0x00, 0x00, 0x00, 0x00,                         // WRITE_SIGNAL 2, r0
        0x00,                                                        // HALT
    ]

    /// The VM parks at `AWAIT`, the executor resolves the future (here the future
    /// value is just `1`), and the post-resume body writes signal 2 = 42.
    @MainActor
    func testAwaitSuspendsThenResumesToHalt() {
        var graph = SignalGraph()
        graph.write(1, .int(0))

        let first = FluxBytecodeVM.runResumable(awaitBytecode, signals: &graph, payload: .null)
        guard case let .success(.suspended(state)) = first else {
            XCTFail("expected .suspended on first run, got \(String(describing: first))")
            return
        }

        // The pre-await write to signal 1 landed, and the future reg holds the handle.
        XCTAssertEqual(graph.read(1), .int(1), "pre-await signal write must persist")
        let future = state.registers[Int(state.futureReg)]
        XCTAssertEqual(future, .int(1), "futureReg must point at the awaited handle")

        // Resume with the resolved future value.
        let resumed = FluxBytecodeVM.resume(state, signals: &graph, value: future)
        guard case let .success(.halt(outcome)) = resumed else {
            XCTFail("expected .halt after resume, got \(String(describing: resumed))")
            return
        }

        // Post-resume body executed: signal 2 = 42, and the resumed VM saw the
        // delivered value in r0 (it overwrote it, so we assert the side effect).
        XCTAssertEqual(graph.read(2), .int(42), "post-resume body must run after AWAIT")
        XCTAssertFalse(outcome.signals.isEmpty, "resume must report signal writes")
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
