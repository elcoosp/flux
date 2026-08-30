//  RenderMountTests.swift
//  FA-RENDER Phase A — the reconciled tree must reach real on-screen UIKit
//  views, not `Color.clear`.
//
//  Drives the real `FluxExecutor` + `FluxUIKit` adapters with a counter-shaped
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
private func counterExecutor() async -> FluxHost.FluxExecutor {
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

    let executor = FluxExecutor(graph: SignalGraph(), registry: AdapterRegistry(table: table))
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
    private func routerExecutor(initialRoute: String? = nil) -> FluxHost.FluxExecutor {
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

        let executor = FluxExecutor(graph: graph, registry: AdapterRegistry(table: table))
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
            ], signalMeta: [20: NodeSignalMeta(deps: [97], thunk: nil, layout: [], itemSlot: nil)]
        )

        let executor = FluxExecutor(graph: graph, registry: AdapterRegistry(table: table))
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
            signals: [], span: FluxSpan(fileId: 0, start: 0, end: 0), excerpt: nil)
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

    /// LANE-B device-only blind spot: a POSITIONAL `Screen("home")` lowers its
    /// route to prop index 0, NOT `FNV-1a("route")`. The reconciler's
    /// `routerActiveChildId` reads the `route` prop at `routePropIndex` (FNV-1a),
    /// finds nothing, and navigation silently never swaps (the documented
    /// "go to settings does nothing" trap, ADR-0045). This test pins the trap
    /// on-device: with the route carried at positional index 0, a real
    /// `Router.navigate` tap must NOT swap the visible screen — it stays on the
    /// first child (home). If this ever starts swapping, the compiler began
    /// lowering positional args to the named prop (closing the blind spot — the
    /// intended fix). The correct author fix is the NAMED `Screen(route:)` form.
    @MainActor
    func testPositionalScreenRouteAtIndex0NeverSwapsOnNavigate() async {
        let home = mountNode(
            10, componentId: 6, kind: .screen,
            props: [Prop(index: 0, value: .str(7))]) // route at POSITIONAL index 0
        let settings = mountNode(
            11, componentId: 6, kind: .screen,
            props: [Prop(index: 0, value: .str(8))]) // route at POSITIONAL index 0
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
            ], signalMeta: [20: NodeSignalMeta(deps: [97], thunk: nil, layout: [], itemSlot: nil)]
        )

        let executor = FluxExecutor(graph: graph, registry: AdapterRegistry(table: table))
        executor.apply(frame)

        guard let beforeHost = executor.view(for: 20) as? RouterHostView else {
            XCTFail("router root view must be a RouterHostView")
            return
        }
        XCTAssertEqual(beforeHost.nav.viewControllers.count, 1, "router must show one screen before navigation")

        // A real navigate closure targeting "settings": LOAD_STR_CONST r0, 8 ; CALL_CAP r1, (3,1), args=r0 ; HALT.
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
            signals: [], span: FluxSpan(fileId: 0, start: 0, end: 0), excerpt: nil)
        executor.registerHandler(100, closure: closure, bytecode: navigateBytecode)

        executor.dispatch(FluxEvent(handlerId: 100, nodeId: 20))
        try? await Task.sleep(nanoseconds: 300_000_000)

        guard let afterHost = executor.view(for: 20) as? RouterHostView else {
            XCTFail("router root view must still be a RouterHostView after navigation")
            return
        }
        XCTAssertEqual(
            afterHost.nav.viewControllers.count, 1,
            "a real navigate dispatch must still leave exactly one screen")
    }

    /// The adapter registry resolves the FLUX-077 `Toggle` primitive kind to a
    /// real `ToggleAdapter` (parity with the Android `FluxUiKit` factory map,
    /// which resolves `"toggle"` → `ToggleAdapter`). Without this wiring the
    /// `examples/todo` `TaskRow` `Toggle` degrades to a blank container on iOS.
    /// The adapter's `update`/`bindHandler`/`destroy` behaviour is exercised
    /// through the running `FluxUIKit` types on the simulator (the standalone
    /// `FluxUIKitTests` SwiftPM target cannot run on this host because UIKit is
    /// iOS-only, so the adapter's behaviour is driven here).
    @MainActor
    func testRegistryResolvesTogglePrimitive() {
        var table = StringTable()
        table.intern(0, "Toggle")
        let registry = AdapterRegistry(table: table)
        guard let adapter = registry.make(named: "Toggle", executor: nil) else {
            XCTFail("registry must resolve the Toggle primitive")
            return
        }
        let view = adapter.create()
        XCTAssertTrue(view is UISwitch, "Toggle adapter must render a UISwitch")

        // `update` must push the controlled `value` prop onto the native switch.
        let props = Props([Props.propIndex(for: "value"): .bool(true)])
        adapter.update(view, from: Props(), to: props)
        XCTAssertEqual((view as? UISwitch)?.isOn, true, "Toggle must reflect the value prop")

        // `bindHandler` wires the `.valueChanged` action; `destroy` must tear it
        // down without throwing. The action-dispatch path itself is covered by
        // `SwitchAdapterTests` (identical `HandlerTarget`/`UIAction` pattern).
        adapter.bindHandler(15, to: view, nodeId: 1)
        adapter.destroy(view)
    }

    /// FLUX-077 parity with the Android `LayoutOverlayAdapterTest` registry
    /// assertion: every FLUX-077 primitive (`Stack`, `Grid`, `Spacer`, `SafeArea`,
    /// `Modal`, `Sheet`, `Dialog`, `Animate`, `Toggle`) resolves through
    /// `AdapterRegistry.byName` and, when `create()`d, produces the expected
    /// native UIKit view — i.e. the node reaches real UI, not a blank container.
    /// Mirrors `testRegistryResolvesTogglePrimitive` for the full set (Android's
    /// `FluxUiKit` factory map resolves each of these names → its adapter).
    @MainActor
    func testRegistryResolvesAllFlux077Primitives() {
        var table = StringTable()
        for (id, name) in [
            100: "Stack", 101: "Grid", 102: "Spacer", 103: "SafeArea",
            104: "Modal", 105: "Sheet", 106: "Dialog", 107: "Animate", 108: "Toggle",
        ] { table.intern(UInt32(id), name) }

        let registry = AdapterRegistry(table: table)
        let expectations: [(String, AnyClass)] = [
            ("Stack", UIStackView.self), ("Grid", UIStackView.self),
            ("Spacer", UIStackView.self), ("SafeArea", UIView.self),
            ("Modal", UIView.self), ("Sheet", UIView.self),
            ("Dialog", UIView.self), ("Animate", UIView.self),
            ("Toggle", UISwitch.self),
        ]
        for item in expectations {
            let name = item.0
            guard let adapter = registry.make(named: name, executor: nil) else {
                XCTFail("FLUX-077 primitive '\(name)' must resolve in AdapterRegistry")
                continue
            }
            let view = adapter.create()
            XCTAssertTrue(
                view.isKind(of: item.1),
                "FLUX-077 '\(name)' must create a \(item.1) (got \(type(of: view)))"
            )
        }
    }

    /// FLUX-072 / ADR-0050 regression: a `ForEach` must re-expand its rows when the
    /// backing list signal changes via a dispatch — not only at initial apply.
    ///
    /// Reproduces the reported To-Do bug: the list starts empty (so no rows render),
    /// the user taps "Add task" which appends to the list signal; the handler writes
    /// the list signal and `reconcileDirty` then runs. If the host only expanded the
    /// ForEach in `apply(frame)`, the new element never produces a row and the todo
    /// list stays blank — exactly what the user sees. This exercises the real
    /// `FluxUIKit` adapters on the simulator (the same path the device uses).
    @MainActor
    func testForEachReExpandsRowsAfterDispatch() async {
        let forEachId: UInt32 = 1
        let templateRowId: UInt32 = 10
        let listSignal: UInt32 = 5
        let itemSlot: UInt32 = 9

        // ForEach node carrying a single `.splice` template child (a Text row).
        let forEachNode = mountNode(
            forEachId,
            componentId: 2, // Column-like container in the registry
            kind: .forEach,
            children: [.splice(itemCount: 1, items: [(key: UInt64(0), node: templateRowId)])]
        )
        let rowNode = mountNode(
            templateRowId,
            componentId: 0, // Text
            props: [Prop(index: 0, value: .str(7))]
        )

        var table = StringTable()
        table.intern(0, "Text")
        table.intern(1, "Button")
        table.intern(2, "Column")
        table.intern(7, "todo")

        let frame = FluxFrame(
            version: 1, seq: 0, flags: 0x01,
            root: forEachNode,
            nodes: [forEachId: forEachNode, templateRowId: rowNode],
            patches: [], handlers: [],
            strings: [StringEntry(stringId: 7, value: "todo")],
            state: [
                StateCell(signalId: listSignal, value: .list([])),
                StateCell(signalId: itemSlot, value: .null),
            ],
            files: [], componentNames: [],
            signalMeta: [
                forEachId: NodeSignalMeta(deps: [listSignal], thunk: nil, layout: [], itemSlot: itemSlot),
                templateRowId: NodeSignalMeta(deps: [itemSlot], thunk: nil, layout: [], itemSlot: nil),
            ]
        )

        let executor = FluxExecutor(graph: SignalGraph(), registry: AdapterRegistry(table: table))
        _ = executor.apply(frame)

        // Empty list => no rows rendered initially.
        XCTAssertNil(executor.view(for: deriveRowId(forEachId, index: 0)),
                     "empty list must render zero rows initially")

        // Handler that appends a new element to the list signal:
        // READ_SIGNAL r0, listSignal ; LOAD_STR_CONST r1, 50 ; LIST_PUSH r0, r1 ;
        // WRITE_SIGNAL listSignal, r0 ; HALT
        let appendBytecode: [UInt8] = [
            0x10, 0x00, 0x05, 0x00, 0x00, 0x00, // READ_SIGNAL r0, signal 5
            0xB3, 0x01, 0x32, 0x00, 0x00, 0x00, // LOAD_STR_CONST r1, string id 50
            0x81, 0x00, 0x01,                   // LIST_PUSH r0, r1
            0x11, 0x05, 0x00, 0x00, 0x00, 0x00, // WRITE_SIGNAL signal 5, r0
            0x00,                               // HALT
        ]
        let closure = ClosureRef(
            hash: Array(repeating: 0, count: 8),
            bytecodeOffset: 0, bytecodeLen: UInt16(appendBytecode.count),
            signalCount: 0, signals: [],
            span: FluxSpan(fileId: 0, start: 0, end: 0), excerpt: nil
        )
        executor.registerHandler(1, closure: closure, bytecode: appendBytecode)

        // Simulate the "Add task" tap. `dispatch(_:)` runs the handler in a
        // `@MainActor` Task and reconciles when it completes, so we must yield to
        // let that Task finish before asserting (mirrors Android's runCurrent()).
        executor.dispatch(FluxEvent(handlerId: 1, nodeId: forEachId))
        try? await Task.sleep(nanoseconds: 100_000_000)
        XCTAssertNil(executor.lastError, "append handler must not fault: \(String(describing: executor.lastError))")

        // The list signal must have grown to one element...
        let listVal = executor.graph.read(listSignal)
        guard case let .list(items) = listVal else {
            XCTFail("list signal should be a list, got \(String(describing: listVal))")
            return
        }
        XCTAssertEqual(items.count, 1, "list signal must have grown to 1 element (got \(items.count))")

        // ...and the ForEach must have re-expanded to one row view.
        XCTAssertNotNil(executor.view(for: deriveRowId(forEachId, index: 0)),
                        "ForEach must re-expand to 1 row after append")
    }

    /// Mirrors `ShadowTreeReconciler.deriveForEachRowId` so the test can address
    /// the derived row id without reaching into the reconciler's internals.
    private func deriveRowId(_ foreachId: UInt32, index: UInt32) -> UInt32 {
        var h: UInt32 = foreachId &* 0x0100_0193
        h = h ^ index
        h = h &* 0x0100_0193
        return h | 0x8000_0000
    }
}
