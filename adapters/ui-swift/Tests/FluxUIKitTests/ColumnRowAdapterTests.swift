//  ColumnRowAdapterTests.swift
//  FluxUIKitTests — `Column`/`Row` keyed child diff (Appendix F.3/F.4).

import XCTest
@testable import FluxUIKit

final class ColumnRowAdapterTests: XCTestCase {
    @MainActor func testColumnSetsSpacingFromGap() {
        let adapter = ColumnAdapter()
        let stack = adapter.create()
        let props = Props([0: .float(12)])
        adapter.update(stack, from: Props(), to: props)
        XCTAssertEqual(stack.spacing, 12)
    }

    @MainActor func testSetChildrenInsertsAndPreservesOrder() {
        let adapter = ColumnAdapter()
        let stack = adapter.create()
        let a = UIView(), b = UIView(), c = UIView()
        adapter.setChildren([a, b], on: stack)
        XCTAssertEqual(stack.arrangedSubviews, [a, b])
        // Adding `c` must not recreate `a`/`b` (their state survives).
        adapter.setChildren([a, c, b], on: stack)
        XCTAssertEqual(stack.arrangedSubviews, [a, c, b])
        XCTAssertTrue(a.superview === stack)
    }

    @MainActor func testSetChildrenRemovesStaleViews() {
        let adapter = ColumnAdapter()
        let stack = adapter.create()
        let a = UIView(), b = UIView()
        adapter.setChildren([a, b], on: stack)
        adapter.setChildren([a], on: stack)
        XCTAssertEqual(stack.arrangedSubviews, [a])
        XCTAssertNil(b.superview)
    }

    @MainActor func testRowUsesHorizontalAxis() {
        let adapter = RowAdapter()
        let stack = adapter.create()
        XCTAssertEqual(stack.axis, .horizontal)
    }

    @MainActor func testRowAndColumnShareReconcileBehavior() {
        let col = ColumnAdapter()
        let row = RowAdapter()
        let colStack = col.create()
        let rowStack = row.create()
        // Each container needs its own child view; a UIView can only have one
        // superview, so sharing one across two stacks would move it.
        let colChild = UIView()
        let rowChild = UIView()
        col.setChildren([colChild], on: colStack)
        row.setChildren([rowChild], on: rowStack)
        XCTAssertTrue(colStack.arrangedSubviews.contains(colChild))
        XCTAssertTrue(rowStack.arrangedSubviews.contains(rowChild))
    }
}
