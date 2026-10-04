//  ScrollViewAdapterTests.swift
//  FluxUIKitTests — FLUX-056 `ScrollView` adapter (unified tier, AGENTS.md §3.5).
//
//  Parity with the Android `LayoutOverlayAdapterTest`'s `scrollview records
//  orientation axis` test: pins the `orientation` prop mapping to the
//  native view's recorded property, and the `setChildren` reconciliation-by-
//  identity contract (existing child views are reused, not recreated).

import XCTest
@testable import FluxUIKit

final class ScrollViewAdapterTests: XCTestCase {
    @MainActor func testScrollViewRecordsOrientationAxis() {
        let adapter = ScrollViewAdapter()
        let view = adapter.create()
        adapter.update(view, from: Props(), to: Props([
            Props.propIndex(for: "orientation"): .str("horizontal"),
        ]))
        XCTAssertEqual(view.fluxRecordedProps[FluxRecordedProp.orientation] as? String, "horizontal")
    }

    @MainActor func testScrollViewDefaultsVerticalWhenOrientationAbsent() {
        let adapter = ScrollViewAdapter()
        let view = adapter.create()
        adapter.update(view, from: Props(), to: Props())
        XCTAssertEqual(view.fluxRecordedProps[FluxRecordedProp.orientation] as? String, "vertical")
    }

    @MainActor func testScrollViewReconcilesChildrenByIdentity() {
        // After the multi-child overdraw fix, `setChildren` wraps two or more
        // children in a `UIStackView` (so they flow instead of stacking on
        // the same four edges). The child instances are still reused — the
        // stack wraps them by reference — so "no recreation on reorder"
        // remains the invariant under test.
        let adapter = ScrollViewAdapter()
        let view = adapter.create()
        let content = view.subviews.first!
        let childA = UIView()
        let childB = UIView()

        adapter.setChildren([childA, childB], on: view)
        let stack = content.subviews.first as? UIStackView
        XCTAssertEqual(stack?.arrangedSubviews, [childA, childB])
        XCTAssertTrue(childA.superview === stack)
        XCTAssertTrue(childB.superview === stack)

        // Reorder: the SAME instances must be reused (no recreation).
        adapter.setChildren([childB, childA], on: view)
        let stack2 = content.subviews.first as? UIStackView
        XCTAssertEqual(stack2?.arrangedSubviews, [childB, childA], "reorder must not recreate child views")
        XCTAssertTrue(stack2?.arrangedSubviews.contains(childA) == true, "existing instance reused")
        XCTAssertTrue(stack2?.arrangedSubviews.contains(childB) == true, "existing instance reused")
    }

    @MainActor func testScrollViewDestroyClearsChildren() {
        let adapter = ScrollViewAdapter()
        let view = adapter.create()
        let child = UIView()
        adapter.setChildren([child], on: view)
        adapter.destroy(view)
        XCTAssertTrue(view.subviews.isEmpty, "destroy must remove all subviews")
    }
}
