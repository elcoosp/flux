//  LayoutOverlayAdapterTests.swift
//  FluxUIKitTests — FLUX-037 layout + FLUX-038/042 overlay/motion adapters.

import XCTest
@testable import FluxUIKit

final class LayoutOverlayAdapterTests: XCTestCase {
    @MainActor func testStackSetsSpacingFromGap() {
        let adapter = StackAdapter()
        let stack = adapter.create()
        let props = Props([Props.propIndex(for: "gap"): .float(8)])
        adapter.update(stack, from: Props(), to: props)
        XCTAssertEqual(stack.spacing, 8)
    }

    @MainActor func testGridSetsSpacingFromGap() {
        let adapter = GridAdapter()
        let stack = adapter.create()
        let props = Props([Props.propIndex(for: "gap"): .float(4)])
        adapter.update(stack, from: Props(), to: props)
        XCTAssertEqual(stack.spacing, 4)
    }

    @MainActor func testSpacerCreatesView() {
        let adapter = SpacerAdapter()
        let stack = adapter.create()
        XCTAssertTrue(stack is UIStackView)
    }

    @MainActor func testSafeAreaHostsChildrenWithinInsets() {
        let adapter = SafeAreaAdapter()
        let view = adapter.create()
        let child = UIView()
        adapter.setChildren([child], on: view)
        XCTAssertTrue(view.subviews.contains(child))
    }

    @MainActor func testStackReconcilesChildrenByIdentity() {
        let adapter = StackAdapter()
        let stack = adapter.create()
        let a = UIView(), b = UIView()
        adapter.setChildren([a, b], on: stack)
        XCTAssertEqual(stack.arrangedSubviews, [a, b])
        let c = UIView()
        adapter.setChildren([a, c, b], on: stack)
        XCTAssertEqual(stack.arrangedSubviews, [a, c, b])
        XCTAssertTrue(a.superview === stack)
    }

    @MainActor func testModalHostsContentChildren() {
        let adapter = ModalAdapter()
        let view = adapter.create()
        let child = UIView()
        adapter.setChildren([child], on: view)
        XCTAssertTrue(view.subviews.contains(child))
    }

    @MainActor func testSheetHostsContentChildren() {
        let adapter = SheetAdapter()
        let view = adapter.create()
        let child = UIView()
        adapter.setChildren([child], on: view)
        XCTAssertTrue(view.subviews.contains(child))
    }

    @MainActor func testDialogHostsContentChildren() {
        let adapter = DialogAdapter()
        let view = adapter.create()
        let child = UIView()
        adapter.setChildren([child], on: view)
        XCTAssertTrue(view.subviews.contains(child))
    }

    @MainActor func testAnimateHostsContentChildren() {
        let adapter = AnimateAdapter()
        let view = adapter.create()
        let child = UIView()
        adapter.setChildren([child], on: view)
        XCTAssertTrue(view.subviews.contains(child))
    }

    @MainActor func testOverlayAdaptersExposeSurfaceName() {
        XCTAssertEqual(ModalAdapter().surface, "Modal")
        XCTAssertEqual(SheetAdapter().surface, "Sheet")
        XCTAssertEqual(DialogAdapter().surface, "Dialog")
        XCTAssertEqual(AnimateAdapter().surface, "Animate")
    }
}
