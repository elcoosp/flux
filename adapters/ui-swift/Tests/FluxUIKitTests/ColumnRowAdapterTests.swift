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

    @MainActor func testColumnPacksChildrenTopLeadingWithIntrinsicSizes() {
        let adapter = ColumnAdapter()
        let stack = adapter.create()
        // Defaults must not leave the stack ambiguous when its parent stretches
        // it full-screen: distribution must be `.fill` (top-pack) and the default
        // alignment (no `alignment` prop) must resolve to `.leading` (left).
        XCTAssertEqual(stack.distribution, .fill)
        XCTAssertEqual(stack.alignment, .leading)
        let a = UILabel(), b = UIButton(type: .system)
        adapter.setChildren([a, b], on: stack)
        // Each child must keep its intrinsic size on the main (vertical) axis so
        // the stack packs them at the top instead of distributing the extra
        // height (the iOS-only "big gap + centered Home + bottom button" bug).
        XCTAssertEqual(a.contentHuggingPriority(for: .vertical), .required)
        XCTAssertEqual(b.contentHuggingPriority(for: .vertical), .required)
        // On the cross (horizontal) axis children must not be stretched either,
        // matching Android's `fillMaxWidth` + left alignment.
        XCTAssertEqual(a.contentHuggingPriority(for: .horizontal), .required)
        XCTAssertEqual(b.contentHuggingPriority(for: .horizontal), .required)
    }

    @MainActor func testRowPacksChildrenLeadingWithIntrinsicSizes() {
        let adapter = RowAdapter()
        let stack = adapter.create()
        XCTAssertEqual(stack.distribution, .fill)
        XCTAssertEqual(stack.alignment, .center)
        let a = UILabel(), b = UIButton(type: .system)
        adapter.setChildren([a, b], on: stack)
        // Cross axis is vertical for a Row: children keep intrinsic height and
        // are centered (mirrors Android's `Row(fillMaxWidth())` default).
        XCTAssertEqual(a.contentHuggingPriority(for: .vertical), .required)
        XCTAssertEqual(b.contentHuggingPriority(for: .vertical), .required)
        XCTAssertEqual(a.contentHuggingPriority(for: .horizontal), .required)
        XCTAssertEqual(b.contentHuggingPriority(for: .horizontal), .required)
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
