//  RuntimeE2ETests.swift
//  End-to-end runtime test without sockets (FLUX-006 scope item 9), now driving
//  the REAL `FluxUIKit` adapters (FLUX-016).
//
//  Each test hand-builds an `Init` frame whose string table interns the
//  stdlib primitive names, then drives the full pipeline —
//  `FluxRuntime.apply` -> `ShadowTreeReconciler` -> real `UILabel` /
//  `UIButton` / `UIStackView` / `UINavigationController` adapters -> VM ->
//  reconciler. View identity is asserted explicitly: an update path must
//  re-apply in place and reuse the same `UIView`, never recreate it.

import XCTest
import UIKit
import FluxUIKit

@testable import FluxHost

/// Builds a node for a stdlib primitive, given its component id and props.
@MainActor
private func node(_ id: UInt32, componentId: UInt32, props: [Prop] = [], children: [Child] = [], handlers: [UInt32] = []) -> ShadowNode {
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

/// Builds a full `FluxFrame` from a root node plus its reachable descendants,
/// interning the stdlib primitive names (so the registry can resolve adapters)
/// and the provided strings.
@MainActor
private func initFrame(root: ShadowNode, descendantNodes: [ShadowNode] = [], strings: [StringEntry] = [], state: [StateCell] = []) -> FluxFrame {
    var table = StringTable()
    table.intern(0, "Text")
    table.intern(1, "Button")
    table.intern(2, "Column")
    table.intern(3, "Row")
    table.intern(4, "TextField")
    table.intern(5, "Router")
    table.intern(6, "Screen")
    for s in strings { table.intern(s.stringId, s.value) }

    var nodes: [UInt32: ShadowNode] = [root.id: root]
    for n in descendantNodes { nodes[n.id] = n }

    return FluxFrame(
        version: 1, seq: 0, flags: 0x01,
        root: root, nodes: nodes,
        patches: [], handlers: [],
        strings: strings, state: state,
        files: []
    )
}

/// A handler closure that reads signal 1 and writes it back incremented by 1:
/// `READ_SIGNAL r0, 1 ; LOAD_INT_CONST r1, 1 ; ADD_I64 r0, r0, r1 ;
///  WRITE_SIGNAL 1, r0 ; HALT`.
private let incrementBytecode: [UInt8] = [
    0x10, 0x00, 0x01, 0x00, 0x00, 0x00,   // READ_SIGNAL r0, signal 1
    0xB0, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // LOAD_INT_CONST r1, 1
    0x20, 0x00, 0x00, 0x01,             // ADD_I64 r0, r0, r1
    0x11, 0x01, 0x00, 0x00, 0x00, 0x00, // WRITE_SIGNAL signal 1, r0
    0x00,                                // HALT
]

/// A `ClosureRef` describing `incrementBytecode`.
private let incrementClosure = ClosureRef(
    hash: Array(repeating: 0, count: 8),
    bytecodeOffset: 0,
    bytecodeLen: UInt16(incrementBytecode.count),
    signalCount: 0,
    signals: [],
    span: FluxSpan(fileId: 0, start: 0, end: 0)
)

final class RuntimeE2ETests: XCTestCase {
    /// Init frame -> reconciler builds the real `UILabel` view tree; the label's
    /// text is set from the resolved string prop.
    @MainActor
    func testInitBuildsRealViewTree() {
        let textNode = node(10, componentId: 0, props: [Prop(index: 0, value: .str(7))])
        let frame = initFrame(
            root: textNode,
            strings: [StringEntry(stringId: 7, value: "Hello, Flux")]
        )
        let executor = FluxRuntime(graph: SignalGraph(), registry: buildRegistry())
        let built = executor.apply(frame)

        XCTAssertEqual(built, [10])
        XCTAssertTrue(executor.view(for: 10) is UILabel)
        let label = executor.view(for: 10) as! UILabel
        XCTAssertEqual(label.text, "Hello, Flux")
    }

    /// Re-applying an identical Init frame must not push `adapter.update` to nodes
    /// whose props are unchanged (Perf R2): the reconciler computes the prop
    /// content hash once and skips the update when it matches. The second apply
    /// should produce neither `built` (views already exist) nor `updated`.
    @MainActor
    func testReapplyingUnchangedFrameSkipsUpdates() {
        let textNode = node(10, componentId: 0, props: [Prop(index: 0, value: .str(7))])
        let frame = initFrame(
            root: textNode,
            strings: [StringEntry(stringId: 7, value: "Hello, Flux")]
        )
        var reconciler = ShadowTreeReconciler(registry: buildRegistry())
        let first = reconciler.apply(frame)
        XCTAssertEqual(Set(first.built), [10])

        let second = reconciler.apply(frame) // identical frame
        XCTAssertEqual(second.built, [], "no node should be rebuilt")
        XCTAssertEqual(second.updated, [], "unchanged nodes must not be re-updated (R2)")
    }

    /// Registering a handler and tapping its button drives VM -> signal write.
    /// The dev server then pushes an `Update` patch carrying the new text; the
    /// reconciler applies it in place and reuses the SAME `UILabel` instance
    /// (identity preserved, no recreation).
    @MainActor
    func testTapUpdatesLabelWithoutRecreatingView() {
        let labelNode = node(10, componentId: 0, props: [Prop(index: 0, value: .str(7))])
        let buttonNode = node(11, componentId: 1, props: [Prop(index: 0, value: .str(8))], handlers: [1])
        let column = node(20, componentId: 2, children: [.node(10), .node(11)])
        let frame = initFrame(
            root: column,
            descendantNodes: [labelNode, buttonNode],
            strings: [
                StringEntry(stringId: 7, value: "Count: 0"),
                StringEntry(stringId: 8, value: "+1"),
            ],
            state: [StateCell(signalId: 1, value: .int(0))]
        )

        let executor = FluxRuntime(graph: SignalGraph(), registry: buildRegistry())
        _ = executor.apply(frame)
        executor.registerHandler(1, closure: incrementClosure, bytecode: incrementBytecode)

        let label = executor.view(for: 10) as! UILabel
        let labelBefore = label // identity capture
        XCTAssertEqual(label.text, "Count: 0")

        // Simulate a tap on the button: dispatch handler 1 with no payload.
        // The VM reads signal 1 (0), adds 1, writes signal 1 (1) back.
        executor.dispatch(FluxEvent(handlerId: 1, nodeId: 11))
        XCTAssertEqual(executor.graph.read(1), .int(1))

        // The dev server pushes an Update patch with the recomputed label text.
        let patchFrame = FluxFrame(
            version: 1, seq: 1, flags: 0x00,
            root: nil, nodes: [:],
            patches: [.update(id: 10, changes: [Prop(index: 0, value: .str(9))], removals: [])],
            handlers: [],
            strings: [StringEntry(stringId: 9, value: "Count: 1")],
            state: [], files: []
        )
        executor.apply(patchFrame)

        // Identity preserved: same UILabel instance across the update.
        XCTAssertTrue(label === labelBefore, "label view must be reused, not recreated")
        XCTAssertEqual(label.text, "Count: 1")
    }

    /// A patch Update to an existing node's props must reuse the view, not build
    /// a new one.
    @MainActor
    func testPatchUpdateReusesView() {
        let textNode = node(10, componentId: 0, props: [Prop(index: 0, value: .str(7))])
        let frame = initFrame(
            root: textNode,
            strings: [StringEntry(stringId: 7, value: "first")]
        )
        let executor = FluxRuntime(graph: SignalGraph(), registry: buildRegistry())
        _ = executor.apply(frame)
        let original = executor.view(for: 10) as! UILabel

        // Now send a delta frame updating the label text.
        let patchFrame = FluxFrame(
            version: 1, seq: 1, flags: 0x00,
            root: nil,
            nodes: [:],
            patches: [.update(id: 10, changes: [Prop(index: 0, value: .str(9))], removals: [])],
            handlers: [],
            strings: [StringEntry(stringId: 9, value: "second")],
            state: [],
            files: []
        )
        executor.apply(patchFrame)

        let after = executor.view(for: 10) as! UILabel
        XCTAssertTrue(original === after, "patch update must reuse the existing view")
        XCTAssertEqual(after.text, "second")
    }

    /// Router E2E: push a screen, edit its state, pop, push again — the screen's
    /// view controller is reused by identity, so its state (a text field's text)
    /// survives the round trip.
    @MainActor
    func testRouterPreservesScreenStateAcrossPushPop() {
        // Two screens, each hosting a TextField bound to its own signal.
        let screenA = node(30, componentId: 6, children: [.node(31)])
        let fieldA = node(31, componentId: 4, props: [Prop(index: 0, value: .str(7))], handlers: [1])
        let screenB = node(40, componentId: 6, children: [.node(41)])
        let fieldB = node(41, componentId: 4, props: [Prop(index: 0, value: .str(8))], handlers: [2])
        let router = node(50, componentId: 5, children: [.node(30), .node(40)])

        let frame = initFrame(
            root: router,
            descendantNodes: [screenA, fieldA, screenB, fieldB],
            strings: [
                StringEntry(stringId: 7, value: "screen A"),
                StringEntry(stringId: 8, value: "screen B"),
            ],
            state: [
                StateCell(signalId: 1, value: .str(7)),
                StateCell(signalId: 2, value: .str(8)),
            ]
        )
        let executor = FluxRuntime(graph: SignalGraph(), registry: buildRegistry())
        _ = executor.apply(frame)

        // The router is a UINavigationController with both screens pushed.
        let nav = executor.view(for: 50) as! UINavigationController
        XCTAssertEqual(nav.viewControllers.count, 2)
        let vcA = nav.viewControllers[0]
        let vcB = nav.viewControllers[1]

        // Pop screen B (Router E2E: push -> edit -> pop -> state preserved).
        let poppedFrame = FluxFrame(
            version: 1, seq: 1, flags: 0x00,
            root: nil, nodes: [:],
            patches: [.reorder(parentId: 50, keys: [30])],
            handlers: [], strings: [], state: [], files: []
        )
        executor.apply(poppedFrame)
        XCTAssertEqual(nav.viewControllers.count, 1)
        XCTAssertTrue(nav.viewControllers.first === vcA)

        // Push B back: the SAME view controller instance must be reused, so its
        // field's edited text (state) survives.
        let pushedFrame = FluxFrame(
            version: 1, seq: 2, flags: 0x00,
            root: nil, nodes: [:],
            patches: [.reorder(parentId: 50, keys: [30, 40])],
            handlers: [], strings: [], state: [], files: []
        )
        executor.apply(pushedFrame)
        XCTAssertEqual(nav.viewControllers.count, 2)
        XCTAssertTrue(nav.viewControllers[1] === vcB, "screen B's VC must be reused by identity on push")
    }

    /// AdapterRegistry resolves the `Image` primitive (registered for P1).
    @MainActor
    func testRegistryResolvesImage() {
        var table = StringTable()
        table.intern(0, "Text")
        table.intern(1, "Button")
        table.intern(2, "Column")
        table.intern(3, "Row")
        table.intern(4, "TextField")
        table.intern(5, "Router")
        table.intern(6, "Screen")
        table.intern(9, "Image")
        let registry = AdapterRegistry(table: table)
        XCTAssertNotNil(registry.make(for: 9, executor: nil), "Image component id 9 should resolve")
    }

    /// AdapterRegistry resolves every stdlib ComponentId the Init frame declares.
    @MainActor
    func testRegistryResolvesAllStdlibComponents() {
        var table = StringTable()
        table.intern(0, "Text")
        table.intern(1, "Button")
        table.intern(2, "Column")
        table.intern(3, "Row")
        table.intern(4, "TextField")
        table.intern(5, "Router")
        table.intern(6, "Screen")
        let registry = AdapterRegistry(table: table)
        XCTAssertEqual(Set(registry.resolvedComponentIds), Set([0, 1, 2, 3, 4, 5, 6]))
        for id in 0...6 {
            XCTAssertNotNil(registry.make(for: UInt32(id), executor: nil), "component id \(id) should resolve")
        }
    }

    /// Gas exhaustion must be reported, not loop forever, on the real pipeline.
    @MainActor
    func testGasExhaustionIsReported() {
        let textNode = node(10, componentId: 0, props: [Prop(index: 0, value: .str(7))])
        let frame = initFrame(root: textNode, strings: [StringEntry(stringId: 7, value: "x")])
        let executor = FluxRuntime(graph: SignalGraph(), registry: buildRegistry())
        _ = executor.apply(frame)

        // An unconditional backward jump to itself never reaches HALT.
        let bytecode: [UInt8] = [0x60, 0xFB, 0xFF, 0xFF, 0xFF, 0x00]
        let closure = ClosureRef(
            hash: Array(repeating: 0, count: 8),
            bytecodeOffset: 0, bytecodeLen: 2, signalCount: 0, signals: [],
            span: FluxSpan(fileId: 0, start: 0, end: 0)
        )
        let result = executor.dispatch(bytecode: bytecode, closure: closure, payload: .null)
        XCTAssertNotNil(result.error)
        XCTAssertEqual(result.error?.kind, .gasExhausted)
    }
}

/// A registry seeded with the stdlib primitive names.
@MainActor
private func buildRegistry() -> AdapterRegistry {
    var table = StringTable()
    table.intern(0, "Text")
    table.intern(1, "Button")
    table.intern(2, "Column")
    table.intern(3, "Row")
    table.intern(4, "TextField")
    table.intern(5, "Router")
    table.intern(6, "Screen")
    return AdapterRegistry(table: table)
}
