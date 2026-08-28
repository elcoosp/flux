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

@testable import FluxHost
@testable import FluxApp

/// Builds a primitive `ShadowNode` for the mount tests.
@MainActor
private func mountNode(
    _ id: UInt32,
    componentId: UInt32,
    kind: NodeKind = .primitive,
    props: [Prop] = [],
    children: [Child] = [],
    handlers: [UInt32] = []
) -> ShadowNode {
    ShadowNode(
        id: id,
        kind: kind,
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
private func counterExecutor() async -> FluxRuntime {
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
        state: [], files: [], componentNames: [
            StringEntry(stringId: 0, value: "Text"),
            StringEntry(stringId: 1, value: "Button"),
            StringEntry(stringId: 2, value: "Column"),
        ], signalMeta: [:]
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
    func testCounterMountsRealViewHierarchy() async {
        let executor = await counterExecutor()

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
    func testMountSurvivesDispatch() async {
        let executor = await counterExecutor()
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

    /// Builds a router-shaped full `FluxFrame`: a `Router` with two `Screen`
    /// children (`home` and `settings`), each carrying a `route` prop. When
    /// [initialRoute] is non-nil, signal 97 (the `Router.navigate` target,
    /// ADR-0045) is pre-seeded so the router presents that screen from the start.
    @MainActor
    private func routerExecutor(initialRoute: String? = nil) -> FluxRuntime {
        let routeIndex: UInt16 = fnv1aRouteIndex()
        let home = mountNode(
            10, componentId: 6, kind: .screen,
            props: [Prop(index: routeIndex, value: .str(7))]) // route: "home"
        let settings = mountNode(
            11, componentId: 6, kind: .screen,
            props: [Prop(index: routeIndex, value: .str(8))]) // route: "settings"
        let router = mountNode(20, componentId: 5, kind: .router, children: [.node(10), .node(11)])

        var table = StringTable()
        table.intern(5, "Router")
        table.intern(6, "Screen")
        table.intern(7, "home")
        table.intern(8, "settings")

        var graph = SignalGraph()
        if let route = initialRoute {
            let routeId: UInt32 = route == "settings" ? 8 : 7
            graph.write(97, .record([(0, .str(routeId))]))
        }

        let frame = FluxFrame(
            version: 1, seq: 0, flags: 0x01,
            root: router,
            nodes: [20: router, 10: home, 11: settings],
            patches: [], handlers: [],
            strings: [
                StringEntry(stringId: 7, value: "home"),
                StringEntry(stringId: 8, value: "settings"),
            ],
            state: [], files: [], componentNames: [
                StringEntry(stringId: 5, value: "Router"),
                StringEntry(stringId: 6, value: "Screen"),
            ], signalMeta: [:]
        )

        let executor = FluxRuntime(graph: graph, registry: AdapterRegistry(table: table))
        executor.apply(frame)
        return executor
    }

    /// FNV-1a (32-bit) of "route", truncated to `UInt16` — must match the value
    /// `ShadowTreeReconciler.routePropIndex` uses to locate a Screen's `route` prop.
    private func fnv1aRouteIndex() -> UInt16 {
        var h: UInt32 = 0x811c_9dc5
        for b in "route".utf8 {
            h = (h ^ UInt32(b)) &* 0x0100_0193
        }
        return UInt16(truncatingIfNeeded: h)
    }

    /// ADR-0045: a `Router` presents only the active-route `Screen`. With signal
    /// 97 unset it shows the first screen (`home`); pre-seeding signal 97 with the
    /// `Router.navigate` target record makes it present the matching `settings`
    /// screen instead (the same signal the live `navigate` handler writes).
    @MainActor
    func testRouterPresentsActiveRouteFromSignal97() async {
        let homeExecutor = routerExecutor(initialRoute: nil)
        guard let homeHost = homeExecutor.view(for: 20) as? RouterHostView else {
            XCTFail("router root view must be a RouterHostView (Appendix F.6: a UIView wrapping UINavigationController)")
            return
        }
        XCTAssertEqual(
            homeHost.nav.viewControllers.count, 1,
            "router must show exactly one screen when signal 97 is unset")
        let homeScreen = homeHost.nav.viewControllers.first

        let settingsExecutor = routerExecutor(initialRoute: "settings")
        guard let settingsHost = settingsExecutor.view(for: 20) as? RouterHostView else {
            XCTFail("router root view must be a RouterHostView (Appendix F.6: a UIView wrapping UINavigationController)")
            return
        }
        XCTAssertEqual(
            settingsHost.nav.viewControllers.count, 1,
            "router must show exactly one screen when signal 97 targets 'settings'")
        let settingsScreen = settingsHost.nav.viewControllers.first

        XCTAssertNotNil(homeScreen, "home screen must be present")
        XCTAssertNotNil(settingsScreen, "settings screen must be present")
        XCTAssertFalse(
            settingsScreen === homeScreen,
            "seeding signal 97 with a different route must swap the visible screen")
    }

    /// ADR-0045 / parity with Android: a real `Router.navigate` tap goes through
    /// `CALL_CAP(3,1)`, which writes the target `RecordVal` to signal 97. The
    /// renderer must swap to the matching Screen. This test runs a REAL navigate
    /// closure through the VM (the same bytecode the compiler emits) and then
    /// asserts the router presents exactly ONE screen — proving `reconcileDirty`
    /// re-applies `routerActiveChildId` (filtering to the active Screen) instead
    /// of re-attaching every built child and stacking both screens. It fails
    /// against the pre-fix `reconcileDirty` (which re-attached all children).
    @MainActor
    func testRouterSwapsScreenOnRealNavigateDispatch() async {
        let routeIndex: UInt16 = fnv1aRouteIndex()
        let home = mountNode(
            10, componentId: 6, kind: .screen,
            props: [Prop(index: routeIndex, value: .str(7))]) // route: "home"
        let settings = mountNode(
            11, componentId: 6, kind: .screen,
            props: [Prop(index: routeIndex, value: .str(8))]) // route: "settings"
        let router = mountNode(20, componentId: 5, kind: .router, children: [.node(10), .node(11)])

        var table = StringTable()
        table.intern(5, "Router")
        table.intern(6, "Screen")
        table.intern(7, "home")
        table.intern(8, "settings")

        let graph = SignalGraph()
        let frame = FluxFrame(
            version: 1, seq: 0, flags: 0x01,
            root: router,
            nodes: [20: router, 10: home, 11: settings],
            patches: [], handlers: [],
            strings: [
                StringEntry(stringId: 7, value: "home"),
                StringEntry(stringId: 8, value: "settings"),
            ],
            state: [], files: [], componentNames: [
                StringEntry(stringId: 5, value: "Router"),
                StringEntry(stringId: 6, value: "Screen"),
            ], signalMeta: [20: NodeSignalMeta(deps: [97], thunk: nil, layout: [])]
        )

        let executor = FluxRuntime(graph: graph, registry: AdapterRegistry(table: table))
        executor.apply(frame)

        // Before navigation: exactly one screen (home). Capture its view instance.
        guard let beforeHost = executor.view(for: 20) as? RouterHostView else {
            XCTFail("router root view must be a RouterHostView")
            return
        }
        XCTAssertEqual(beforeHost.nav.viewControllers.count, 1, "router must show one screen before navigation")
        let beforeScreen = beforeHost.nav.viewControllers.first

        // A real navigate closure: LOAD_STR_CONST r0, 8 ; CALL_CAP r1, cap=3, method=1, args=r0 ; HALT.
        let navigateBytecode: [UInt8] = [
            0xB3, // LOAD_STR_CONST
            0, // result reg r0
            8, 0, 0, 0, // string id = 8 ("settings"), u32 LE
            0x90, // CALL_CAP
            1, // result reg r1
            3, 0, 0, 0, // capId = 3
            1, 0, // methodId = 1
            0, // args reg r0
            0x00, // HALT
        ]
        let closure = ClosureRef(
            hash: [], bytecodeOffset: 0,
            bytecodeLen: UInt16(navigateBytecode.count), signalCount: 0,
            signals: [], span: FluxSpan(fileId: 0, start: 0, end: 0))
        executor.registerHandler(100, closure: closure, bytecode: navigateBytecode)

        // Dispatch exactly like a button tap (async, MainActor); the navigate
        // capability writes the target to signal 97 and reconcileDirty re-filters,
        // building and showing the settings screen.
        executor.dispatch(FluxEvent(handlerId: 100, nodeId: 20))
        try? await Task.sleep(nanoseconds: 300_000_000)

        guard let afterHost = executor.view(for: 20) as? RouterHostView else {
            XCTFail("router root view must still be a RouterHostView after navigation")
            return
        }
        XCTAssertEqual(
            afterHost.nav.viewControllers.count, 1,
            "a real navigate dispatch must leave exactly one (active) screen — not stack both")
        let afterScreen = afterHost.nav.viewControllers.first
        // The active screen view instance must change: without the reconcileDirty
        // re-filter, the router keeps showing the originally-built home view and
        // navigation does nothing.
        XCTAssertFalse(
            afterScreen === beforeScreen,
            "a real navigate dispatch must swap the visible screen (build+show settings)")
    }
}
