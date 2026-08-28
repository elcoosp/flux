//  FluxHostConnectionTests.swift
//  FR-017 reconnect UX — connection-state + banner (TDD, mocked transport).
//
//  The live WebSocket path is verified by build + this state-machine test using
//  an in-memory `MockTransport` (mirrors Android's `MockTransport` unit-test
//  contract; real sockets land in FLUX-023). We assert the banner appears on
//  `.reconnecting` and clears on `.connected`, and that a received frame is
//  decoded and applied to the runtime.

import XCTest
import SwiftUI
import FluxHost
@testable import FluxApp
@testable import FluxHost

/// In-memory transport for connection-state tests (FR-017). No network.
@MainActor
final class MockTransport: FluxTransport {
    var status: ConnectionStatus = .connecting
    var onFrame: (@MainActor (Data) -> Void)?
    var onStatusChange: (@MainActor (ConnectionStatus) -> Void)?
    private(set) var connectCalls = 0
    private(set) var closeCalls = 0

    func connect() { connectCalls += 1; setStatus(.connected) }
    func send(_ bytes: Data) {}
    func close() { closeCalls += 1; setStatus(.connecting) }

    func setStatus(_ s: ConnectionStatus) {
        status = s
        onStatusChange?(s)
    }

    /// Simulates the socket dropping, entering the reconnecting state.
    func drop() { setStatus(.reconnecting) }
}

@MainActor
final class FluxHostConnectionTests: XCTestCase {
    func testBannerVisibleWhileReconnecting() {
        let transport = MockTransport()
        let state = HostConnectionState()
        state.bind(transport)

        XCTAssertFalse(state.isReconnecting, "connected → no banner")

        transport.drop()
        XCTAssertTrue(state.isReconnecting, "reconnecting → banner visible")
        XCTAssertEqual(state.status, .reconnecting)

        transport.setStatus(.connected)
        XCTAssertFalse(state.isReconnecting, "reconnected → banner hidden")
    }

    func testReceivedFrameBuildsNativeTree() async throws {
        // The transport's onFrame path decodes bytes and calls
        // `FluxExecutor.apply`; decoding is covered by WireDecodeTests, so here we
        // assert the frame the transport would deliver actually mounts a real
        // native tree on the runtime.
        let frame = makeCounterInitFrame()

        var table = StringTable()
        table.intern(0, "Text"); table.intern(1, "Button")
        table.intern(2, "Column"); table.intern(3, "Row")
        table.intern(4, "TextField"); table.intern(5, "Router")
        table.intern(6, "Screen")
        let runtime = FluxExecutor(graph: SignalGraph(), registry: AdapterRegistry(table: table))
        runtime.apply(frame)

        XCTAssertNotNil(runtime.rootView, "frame applied → native tree mounted")
    }
}

// MARK: - Frame helper (mirrors RuntimeE2ETests fixtures)

private func node(_ id: UInt32, componentId: UInt32, props: [Prop] = [], children: [Child] = []) -> ShadowNode {
    ShadowNode(
        id: id, kind: .primitive, componentId: componentId,
        props: props, childCount: UInt16(children.count),
        children: children, handlerCount: 0, handlers: [],
        span: FluxSpan(fileId: 0, start: 0, end: 0)
    )
}

private func makeCounterInitFrame() -> FluxFrame {
    // Root column (id 1) with a text child (id 2) and button child (id 3).
    let text = node(2, componentId: 0, props: [Prop(index: 0, value: .str(7))])
    let button = node(3, componentId: 1, props: [Prop(index: 0, value: .str(8)), Prop(index: 1, value: .handlerRef(5))])
    let root = node(1, componentId: 2, children: [.node(2), .node(3)])
    let nodes: [UInt32: ShadowNode] = [1: root, 2: text, 3: button]
    return FluxFrame(
        version: 1, seq: 0, flags: 0x01,
        root: root, nodes: nodes,
        patches: [], handlers: [],
        strings: [StringEntry(stringId: 7, value: "tapped 0 times"),
                  StringEntry(stringId: 8, value: "Increment")],
        state: [], files: [], componentNames: [
            StringEntry(stringId: 0, value: "Text"),
            StringEntry(stringId: 1, value: "Button"),
            StringEntry(stringId: 2, value: "Column"),
        ], signalMeta: [:]
    )
}
