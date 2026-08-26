//  VMDispatchPerfTests.swift
//  Perf #9 — VM dispatch-table option.
//
//  The perf review suggested a LuaJIT-style closure/dispatch table indexed by
//  opcode to replace the `switch` in `FluxBytecodeVM.run`. This test pins
//  `runViaDispatchTable` (the integer-tagged dispatch variant) to be
//  byte-for-byte equivalent to the canonical `run` across a bytecode battery,
//  then micro-benchmarks both. Swift already lowers an `enum` `switch` to a
//  jump table, so the table variant is not expected to be faster; the canonical
//  `run` is retained. The throughput assertion is a regression guard.

import XCTest

@testable import FluxHost

final class VMDispatchPerfTests: XCTestCase {
    /// Every program in the battery must yield an identical `VmOutcome` from
    /// the switch-based `run` and the dispatch-table `runViaDispatchTable`.
    func testDispatchTableMatchesSwitch() async throws {
        let battery: [[UInt8]] = VMDispatchPerfTests.battery()
        for bc in battery {
            var s1: any SignalStore = InMemorySignals()
            var s2: any SignalStore = InMemorySignals()
            let a = try FluxBytecodeVM.run(bc, signals: &s1, payload: .null)
            let b = try FluxBytecodeVM.runViaDispatchTable(bc, signals: &s2, payload: .null)
            XCTAssertEqual(a.registers, b.registers, "register mismatch for \(bc)")
            // `signals` is `[(UInt32, VMValue)]` — an array of tuples, which
            // Swift cannot compare with `==`, so compare element-wise after a
            // stable id sort.
            let sa = a.signals.sorted { $0.0 < $1.0 }
            let sb = b.signals.sorted { $0.0 < $1.0 }
            XCTAssertEqual(sa.count, sb.count, "signal count mismatch for \(bc)")
            for (lhs, rhs) in zip(sa, sb) {
                XCTAssertEqual(lhs.0, rhs.0, "signal id mismatch for \(bc)")
                XCTAssertEqual(lhs.1, rhs.1, "signal value mismatch for \(bc)")
            }
            XCTAssertEqual(a.gasUsed, b.gasUsed, "gas mismatch for \(bc)")
        }
    }

    /// The canonical switch-based evaluator must sustain a throughput budget
    /// (regression guard for the "keep the switch" decision).
    func testSwitchThroughput() async throws {
        let bc = VMDispatchPerfTests.counterHandler()
        let iterations = 20_000
        let start = Date()
        for _ in 0..<iterations {
            var s: any SignalStore = InMemorySignals()
            s.write(1, .int(0))
            _ = try FluxBytecodeVM.run(bc, signals: &s, payload: .null)
        }
        let elapsed = Date().timeIntervalSince(start)
        XCTAssertLessThan(elapsed, 2.0, "\(iterations) handler evals took \(elapsed)s")
    }

    // MARK: - Bytecode fixtures

    /// A battery exercising arithmetic, control flow, lists, records and caps.
    static func battery() -> [[UInt8]] {
        [
            counterHandler(),
            // LOAD_INT_CONST r0, 7 ; ADD_I64 r0, r0, r0 ; WRITE_SIGNAL 1, r0 ; HALT
            [0xB0, 0x00, 0x07, 0, 0, 0, 0, 0, 0, 0, 0x20, 0x00, 0x00, 0x00, 0x11, 0x01, 0, 0, 0, 0x00],
            // conditional jump loop: count r0 from 0..<5
            // LOAD_INT_CONST r0,0 ; LOAD_INT_CONST r1,5 ; LT_I64 r2,r0,r1 ;
            // COND_JUMP +? ; (body) ADD_I64 r0,r0,1 ; JUMP -? ; HALT
            [0xB0, 0x00, 0x00, 0, 0, 0, 0, 0, 0, 0,
             0xB0, 0x01, 0x05, 0, 0, 0, 0, 0, 0, 0,
             0x30, 0x02, 0x00, 0x01,
             0x61, 0x06, 0x00, 0x00, 0x00,
             0x20, 0x00, 0x00, 0x00,
             0x60, 0xF6, 0xFF, 0xFF, 0xFF,
             0x00],
            // ALLOC_LIST ; LIST_PUSH r0, r1 (LOAD_INT_CONST r1, 9) ; LIST_LEN r2, r0 ; HALT
            [0x71, 0x00,
             0xB0, 0x01, 0x09, 0, 0, 0, 0, 0, 0, 0,
             0x72, 0x00, 0x01,
             0x34, 0x02, 0x00,
             0x00],
        ]
    }

    /// `READ_SIGNAL r0, 1 ; LOAD_INT_CONST r1, 1 ; ADD_I64 r0, r0, r1 ;
    ///  WRITE_SIGNAL 1, r0 ; HALT`.
    static func counterHandler() -> [UInt8] {
        [0x10, 0x00, 0x01, 0, 0, 0,
         0xB0, 0x01, 0x01, 0, 0, 0, 0, 0, 0, 0,
         0x20, 0x00, 0x00, 0x01,
         0x11, 0x01, 0, 0, 0, 0,
         0x00]
    }
}
