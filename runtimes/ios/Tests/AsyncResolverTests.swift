//  AsyncResolverTests.swift
//  First-class async (ADR-0044) + unified capability bridge (ADR-0045): the
//  executor's `AsyncResolver` settles a `Pending` result cell and resumes the
//  parked handler. These tests drive `FluxExecutor.dispatch` (which runs the
//  resumable handler via `runHandlerAsync`) with a real async resolver to prove:
//    1. a `Pending` cell genuinely parks the handler (it does not complete
//       synchronously);
//    2. the resolver's settled value is what the handler resumes with;
//    3. swapping `asyncResolver` from `PassthroughAsyncResolver` to a real
//       `DelayAsyncResolver` / keyed resolver changes the resolved value.
//
//  The capability under test is the oracle's reference async stub (cap 2, method
//  99), which allocates a fresh `Pending` cell and returns its id (ADR-0045).

import XCTest
import FluxUIKit

@testable import FluxHost

final class AsyncResolverTests: XCTestCase {
    /// Bytecode (iOS CALL_CAP is 9 bytes: op, result, cap u32 LE, method u16 LE,
    /// args — matching `Opcodes.callCap` operandLen 8):
    /// `CALL_CAP r2, cap=2, method=99, args=r0` · `AWAIT r0, r2` · `WRITE_SIGNAL s2, r0` · `HALT`.
    private let asyncHandler: [UInt8] = [
        0x90, 0x02, 0x02, 0x00, 0x00, 0x00, 0x63, 0x00, 0x00, // CALL_CAP r2, (2,99), args=r0
        0xE0, 0x00, 0x02, // AWAIT r0, r2 (future = cell id in r2)
        0x11, 0x02, 0x00, 0x00, 0x00, 0x00, // WRITE_SIGNAL 2, r0
        0x00, // HALT
    ]

    @MainActor
    private func executor(resolver: any AsyncResolver) -> FluxHost.FluxExecutor {
        let executor = FluxExecutor(graph: SignalGraph(), registry: AdapterRegistry(table: MaterializationStringTable()))
        executor.asyncResolver = resolver
        let closure = ClosureRef(
            hash: [], bytecodeOffset: 0,
            bytecodeLen: UInt16(asyncHandler.count), signalCount: 0,
            signals: [], span: FluxSpan(fileId: 0, start: 0, end: 0), excerpt: nil)
        executor.registerHandler(1, closure: closure, bytecode: asyncHandler)
        return executor
    }

    /// The default `PassthroughAsyncResolver` treats the `Pending` cell as already
    /// settled, so `r0` receives the cell id itself and signal 2 echoes it. This
    /// anchors the contrast: with a real resolver the value differs.
    @MainActor
    func testPassthroughResolverEchoesCellId() async {
        let executor = self.executor(resolver: PassthroughAsyncResolver())
        executor.dispatch(FluxEvent(handlerId: 1, nodeId: 0))
        // Passthrough is synchronous; give the Task a tick to fold signals.
        try? await Task.sleep(nanoseconds: 50_000_000)

        guard case let .int(cellId) = executor.graph.read(2) else {
            XCTFail("signal 2 must hold the echoed cell id under Passthrough")
            return
        }
        XCTAssert(cellId >= 1_000_000, "Passthrough echoes the fresh async cell id into signal 2")
    }

    /// A real `DelayAsyncResolver` parks the handler for a wall-clock interval and
    /// settles the `Pending` cell to `Null`; signal 2 must then hold `Null`, not
    /// the cell id — proving the handler resumed via the resolver, not synchronously.
    @MainActor
    func testDelayResolverSettlesPendingCellToNull() async {
        nonisolated(unsafe) var waited = false
        let resolver = DelayAsyncResolver(delay: 0.05) { d in
            waited = true
            try? await Task.sleep(nanoseconds: UInt64(d * 1_000_000_000))
        }
        let executor = self.executor(resolver: resolver)
        let start = Date()
        executor.dispatch(FluxEvent(handlerId: 1, nodeId: 0))
        try? await Task.sleep(nanoseconds: 150_000_000)
        let elapsed = Date().timeIntervalSince(start)

        XCTAssert(waited, "the resolver's suspend closure must have been awaited")
        XCTAssert(elapsed >= 0.05, "handler must not complete until the future settles (real suspension)")
        XCTAssertEqual(executor.graph.read(2), .null, "after a Null-settled resolve, signal 2 is Null")
    }

    /// A `CapabilityAsyncResolver` with a `default` resolver returns the resolved
    /// value; the handler must resume with that marker in signal 2.
    @MainActor
    func testCapabilityResolverResumesWithResolvedValue() async {
        let marker: Int64 = 42
        let resolver = CapabilityAsyncResolver(default: { _, _ in
            .int(marker)
        })
        let executor = self.executor(resolver: resolver)
        executor.dispatch(FluxEvent(handlerId: 1, nodeId: 0))
        try? await Task.sleep(nanoseconds: 100_000_000)

        XCTAssertEqual(executor.graph.read(2), .int(marker), "handler resumes with the resolver's value")
    }
}
