//  ButtonAdapterTests.swift
//  FluxUIKitTests — `Button` adapter (Appendix F.2).

import XCTest
@testable import FluxUIKit

final class ButtonAdapterTests: XCTestCase {
    @MainActor func testUpdateSetsTitleAndEnabled() {
        let adapter = ButtonAdapter()
        let button = adapter.create()
        let props = Props([
            Props.propIndex(for: "text"): .str("Tap"),
            Props.propIndex(for: "enabled"): .bool(false),
        ])
        adapter.update(button, from: Props(), to: props)
        XCTAssertEqual(button.title(for: .normal), "Tap")
        XCTAssertFalse(button.isEnabled)
    }

    @MainActor func testTapDispatchesBoundHandler() {
        let executor = MockExecutor()
        let adapter = ButtonAdapter(executor: executor)
        let button = adapter.create()
        adapter.bindHandler(7, to: button, nodeId: 3)
        button.sendActions(for: .touchUpInside)
        XCTAssertTrue(executor.didDispatch)
        XCTAssertEqual(executor.dispatched.first?.handlerId, 7)
        XCTAssertEqual(executor.dispatched.first?.nodeId, 3)
    }

    @MainActor func testExecutorReferencedWeakly() {
        var executor: MockExecutor? = MockExecutor()
        let adapter = ButtonAdapter(executor: executor)
        let button = adapter.create()
        adapter.bindHandler(1, to: button, nodeId: 1)
        executor = nil // release; the action target must not retain it
        button.sendActions(for: .touchUpInside) // should be a no-op, not a crash
    }
}
