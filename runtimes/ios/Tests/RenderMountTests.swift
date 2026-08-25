//  RenderMountTests.swift
//  FA-RENDER Phase A — the reconciled tree must reach real on-screen UIKit
//  views, not `Color.clear`.
//
//  Drives the real `FluxRuntime` + `FluxUIKit` adapters with a counter-shaped
//  `Init` frame (Column → Text + Button), then asserts the host mount presents
//  a real view hierarchy: the executor's `rootView` is the Column's
//  `UIStackView` and it contains the Text `UILabel` and Button `UIButton`.

import XCTest
import UIKit
import FluxUIKit

@testable import FluxApp

/// Builds a primitive `ShadowNode` for the mount tests.
@MainActor
private func mountNode(
    _ id: UInt32,
    componentId: UInt32,
    props: [Prop] = [],
    children: [Child] = [],
    handlers: [UInt32] = []
) -> ShadowNode {
    ShadowNode(
        id: id,
        kind: .primitive,
        componentId: componentId,
        props: props,
        childCount: UInt16(children.count),
        children: children,
        handlerCount: UInt16(handlers.count),
        handlers: handlers,
        span: FluxSpan(fileId: 0, start: 0, end: 0)
    )
}

/// Builds a counter-shaped full `FluxFrame` and the executor it feeds.
@MainActor
private func counterExecutor() -> FluxRuntime {
    let text = mountNode(10, componentId: 0, props: [Prop(index: 0, value: .str(7))])
    let button = mountNode(11, componentId: 1, props: [Prop(index: 0, value: .str(8))])
    let column = mountNode(20, componentId: 2, children: [.node(10), .node(11)])

    var table = StringTable()
    table.intern(0, "Text")
    table.intern(1, "Button")
    table.intern(2, "Column")
    table.intern(7, "tapped 0 times")
    table.intern(8, "Increment")

    let frame = FluxFrame(
        version: 1, seq: 0, flags: 0x01,
        root: column,
        nodes: [20: column, 10: text, 11: button],
        patches: [], handlers: [],
        strings: [
            StringEntry(stringId: 7, value: "tapped 0 times"),
            StringEntry(stringId: 8, value: "Increment"),
        ],
        state: [], files: []
    )

    let executor = FluxRuntime(graph: SignalGraph(), registry: AdapterRegistry(table: table))
    executor.apply(frame)
    return executor
}

/// The host mount presents a real view hierarchy for the counter example.
final class RenderMountTests: XCTestCase {
    /// After applying the counter Init frame, the host's root view is the real
    /// Column `UIStackView` and it hosts the Text `UILabel` + Button `UIButton`.
    @MainActor
    func testCounterMountsRealViewHierarchy() {
        let executor = counterExecutor()

        guard let root = executor.rootView else {
            XCTFail("rootView must be non-nil after applying the counter frame")
            return
        }
        XCTAssertTrue(root is UIStackView, "root view must be the Column's UIStackView, got \(type(of: root))")

        let stack = root as! UIStackView
        XCTAssertEqual(stack.arrangedSubviews.count, 2, "Column must host Text + Button")
        XCTAssertTrue(stack.arrangedSubviews[0] is UILabel, "first child must be the Text UILabel")
        XCTAssertTrue(stack.arrangedSubviews[1] is UIButton, "second child must be the Button UIButton")
    }

    /// The mount survives a per-dispatch reconcile: after one dispatch the same
    /// root view (identity preserved) still presents the counter's children.
    @MainActor
    func testMountSurvivesDispatch() {
        let executor = counterExecutor()
        let rootBefore = executor.rootView
        XCTAssertNotNil(rootBefore, "root view must exist before dispatch")

        // A no-op dispatch (handler 0 unregistered) still runs the reconcile
        // path and must not detach or recreate the mounted root.
        executor.dispatch(FluxEvent(handlerId: 0, nodeId: 20))
        let rootAfter = executor.rootView
        XCTAssertTrue(rootAfter === rootBefore, "mounted root view identity must survive dispatch")
        guard let stack = rootAfter as? UIStackView else {
            XCTFail("root view is no longer the Column stack after dispatch")
            return
        }
        XCTAssertEqual(stack.arrangedSubviews.count, 2, "children must remain after dispatch")
    }
}
