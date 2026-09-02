//  RuntimeGapTests.swift
//  Spec-gap fixes for the iOS runtime (P1): G1–G6.
//
//  One RED test per gap, written before the implementation it exercises. Each
//  test drives the real `FluxExecutor` / `FluxBytecodeVM` (no mocks) and asserts
//  observable behavior: a registered handler firing, a memory-cap fault, real
//  string resolution, a data-driven capability dispatch, lifecycle hooks, and
//  `@pure` subtree skipping.

import XCTest
import UIKit
import FluxUIKit

@testable import FluxHost

// MARK: - Test support

/// Waits for an async `dispatch(_:)` Task to complete by polling for a
/// condition. `dispatch(_ event:)` is non-async (fires-and-forgets via Task);
/// tests must `await` before asserting signal writes.
@MainActor
func waitForDispatch(executor: FluxHost.FluxExecutor, timeoutMs: Int = 500, where condition: () -> Bool) async {
    let deadline = DispatchTime.now() + .milliseconds(timeoutMs)
    while !condition() && DispatchTime.now() < deadline {
        try? await Task.sleep(nanoseconds: 1_000_000)
    }
}

/// Builds a primitive `ShadowNode` (mirrors the helper in RuntimeE2ETests).
@MainActor
private func gapNode(
    _ id: UInt32,
    componentId: UInt32,
    props: [Prop] = [],
    children: [Child] = [],
    handlers: [UInt32] = [],
    mountHandler: UInt32? = nil,
    cleanupHandler: UInt32? = nil,
    isPure: Bool = false
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
        span: FluxSpan(fileId: 0, start: 0, end: 0),
        mountHandler: mountHandler,
        cleanupHandler: cleanupHandler,
        isPure: isPure
    )
}

/// Builds a full `FluxFrame` from a root node plus descendants and string/state.
@MainActor
private func gapFrame(
    root: ShadowNode,
    descendantNodes: [ShadowNode] = [],
    handlers: [HandlerDef] = [],
    strings: [StringEntry] = [],
    state: [StateCell] = []
) -> FluxFrame {
    let table = MaterializationStringTable()
    table.store(id: 0, value: "Text")
    table.store(id: 1, value: "Button")
    table.store(id: 2, value: "Column")
    table.store(id: 3, value: "Row")
    table.store(id: 4, value: "TextField")
    table.store(id: 5, value: "Router")
    table.store(id: 6, value: "Screen")
    for s in strings { table.store(id: s.stringId, value: s.value) }

    var nodes: [UInt32: ShadowNode] = [root.id: root]
    for n in descendantNodes { nodes[n.id] = n }

    return FluxFrame(
        version: 1, seq: 0, flags: 0x01,
        root: root, nodes: nodes,
        patches: [], handlers: handlers,
        strings: strings, state: state,
        files: [], componentNames: [], signalMeta: [:]
    )
}

/// A registry seeded with the stdlib primitive names.
@MainActor
private func gapRegistry() -> AdapterRegistry {
    let table = MaterializationStringTable()
    table.store(id: 0, value: "Text")
    table.store(id: 1, value: "Button")
    table.store(id: 2, value: "Column")
    table.store(id: 3, value: "Row")
    table.store(id: 4, value: "TextField")
    table.store(id: 5, value: "Router")
    table.store(id: 6, value: "Screen")
    return AdapterRegistry(table: table)
}

// MARK: - G1: register decoded handlers

/// G1 — the executor must register handlers carried by a frame and dispatch
/// the decoded bytecode when the bound id fires.
final class GapG1RegisterHandlersTests: XCTestCase {
    @MainActor
    func testInitHandlerFiresAfterRegister() async {
        // READ_SIGNAL r0, 1 ; LOAD_INT_CONST r1, 1 ; ADD_I64 r0,r0,r1 ;
        // WRITE_SIGNAL 1, r0 ; HALT
        let bytecode: [UInt8] = [
            0x10, 0x00, 0x01, 0x00, 0x00, 0x00,
            0xB0, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x20, 0x00, 0x00, 0x01,
            0x11, 0x01, 0x00, 0x00, 0x00, 0x00,
            0x00,
        ]
        let closure = ClosureRef(
            hash: Array(repeating: 0, count: 8),
            bytecodeOffset: 0, bytecodeLen: UInt16(bytecode.count),
            signalCount: 0, signals: [], span: FluxSpan(fileId: 0, start: 0, end: 0), excerpt: nil
        )

        let button = gapNode(11, componentId: 1, handlers: [1])
        let column = gapNode(20, componentId: 2, children: [.node(11)])
        let frame = gapFrame(
            root: column, descendantNodes: [button],
            handlers: [HandlerDef(handlerId: 1, closure: closure, bytecode: bytecode)],
            strings: [StringEntry(stringId: 8, value: "+1")],
            state: [StateCell(signalId: 1, value: .int(0))]
        )

        let executor = FluxExecutor(graph: SignalGraph(), registry: gapRegistry())
        executor.apply(frame)
        // The handler must be registered purely from the frame, with no explicit
        // `registerHandler` call from the test.
        XCTAssertEqual(executor.graph.read(1), .int(0))

        executor.dispatch(FluxEvent(handlerId: 1, nodeId: 11))
        await waitForDispatch(executor: executor) { executor.graph.read(1) == .int(1) }
        XCTAssertEqual(executor.graph.read(1), .int(1))
    }
}

// MARK: - G2: 16 MiB memory cap

/// G2 — allocations past the per-dispatch budget must raise `MemoryExhausted`.
final class GapG2MemoryCapTests: XCTestCase {
    @MainActor
    func testLargeAllocationErrors() async {
        // Loop: ALLOC_RECORD r0, 65535 (~512 KiB each) then JUMP back. The
        // running allocation counter crosses 16 MiB after ~31 iterations, which
        // is well within the 100k gas budget, so `MemoryExhausted` (not gas)
        // must fire.
        let big: [UInt8] = [
            0x70, 0x00, 0xFF, 0xFF,             // ALLOC_RECORD r0, 0xFFFF
            0x60, 0xF7, 0xFF, 0xFF, 0xFF,       // JUMP -9 (back to the ALLOC_RECORD)
            0x01,                               // NOP (keeps the jump's nextIP valid)
        ]
        let closure = ClosureRef(
            hash: Array(repeating: 0, count: 8),
            bytecodeOffset: 0, bytecodeLen: UInt16(big.count),
            signalCount: 0, signals: [], span: FluxSpan(fileId: 0, start: 0, end: 0), excerpt: nil
        )
        let executor = FluxExecutor(graph: SignalGraph(), registry: gapRegistry())
        let result = executor.dispatch(bytecode: big, closure: closure, payload: .null)
        XCTAssertNotNil(result.error)
        XCTAssertEqual(result.error?.kind, .memoryExhausted)
    }

    @MainActor
    func testSmallAllocationSucceeds() async {
        // ALLOC_RECORD r0, count=10  → 160 bytes, well under the budget.
        let small: [UInt8] = [
            0x70, 0x00, 0x0A, 0x00, 0x00, // ALLOC_RECORD r0, 10
            0x00,                        // HALT
        ]
        let closure = ClosureRef(
            hash: Array(repeating: 0, count: 8),
            bytecodeOffset: 0, bytecodeLen: UInt16(small.count),
            signalCount: 0, signals: [], span: FluxSpan(fileId: 0, start: 0, end: 0), excerpt: nil
        )
        let executor = FluxExecutor(graph: SignalGraph(), registry: gapRegistry())
        let result = executor.dispatch(bytecode: small, closure: closure, payload: .null)
        XCTAssertNil(result.error)
    }
}

// MARK: - G3: real STR_LEN / STR_CONCAT

/// G3 — `STR_LEN`/`STR_CONCAT` must resolve real strings via the table.
final class GapG3StringOpsTests: XCTestCase {
    @MainActor
    func testStrLenResolvesRealString() async throws {
        let table = MaterializationStringTable()
        table.store(id: 5, value: "hello") // 5 bytes
        var signals: any SignalStore = InMemorySignals()
        // LOAD_STR_CONST r1, 5 ; STR_LEN r0, r1 ; HALT
        let bc: [UInt8] = [
            0xB3, 0x01, 0x05, 0x00, 0x00, 0x00, // LOAD_STR_CONST r1, str(5)
            0x53, 0x00, 0x01,                   // STR_LEN r0, r1
            0x00,                               // HALT
        ]
        let out = try FluxBytecodeVM.run(bc, signals: &signals, payload: .null, stringTable: table)
        XCTAssertEqual(out.registers[0], .int(5))
    }

    @MainActor
    func testStrConcatInternsResult() async throws {
        let table = MaterializationStringTable()
        table.store(id: 5, value: "hello")
        table.store(id: 6, value: "world")
        // STR_CONCAT will intern "helloworld" into the shared table via
        // `intern(_:)`; a second `STR_LEN` against that id resolves it back to
        // text, proving the derived string was interned and is visible to the
        // same table instance the VM mutates.
        var signals: any SignalStore = InMemorySignals()
        // LOAD_STR_CONST r1, 5 ; LOAD_STR_CONST r2, 6 ;
        // STR_CONCAT r3, r1, r2 ; STR_LEN r0, r3 ; HALT
        let bc: [UInt8] = [
            0xB3, 0x01, 0x05, 0x00, 0x00, 0x00,
            0xB3, 0x02, 0x06, 0x00, 0x00, 0x00,
            0x50, 0x03, 0x01, 0x02,               // STR_CONCAT r3, r1, r2
            0x53, 0x00, 0x03,                     // STR_LEN r0, r3
            0x00,
        ]
        let out = try FluxBytecodeVM.run(
            bc, signals: &signals, payload: .null,
            stringTable: table
        )
        XCTAssertEqual(out.registers[0], .int(10))
    }
}

// MARK: - G4: data-driven CALL_CAP registry

/// G4 — `CALL_CAP` must route by `(capId, methodId)` through a registry, not a
/// hardcoded `== (1,1)` branch.
final class GapG4CapRegistryTests: XCTestCase {
    @MainActor
    func testNonTrivialCapRoutesToRegisteredImpl() async throws {
        // Registry wiring capId=7, methodId=9 to write signal 50 with the first
        // argument's value.
        let registry = CapabilityRegistry(entries: [
            (7, 9, { _, _, arg, signals in
                signals.write(50, arg)
                return 50
            }),
        ])
        var signals: any SignalStore = InMemorySignals()
        // CALL_CAP r0, capId=7, methodId=9, argsReg=r1 ; HALT
        // operand layout: [0]=resultReg, [1..4]=capId, [5..6]=methodId, [7]=args
        let bc: [UInt8] = [
            0x90, 0x00, 0x07, 0x00, 0x00, 0x00, 0x09, 0x00, 0x01,
            0x00,
        ]
        // Seed r1 with the argument via LOAD_INT_CONST r1, 42 first.
        var full: [UInt8] = [
            0xB0, 0x01, 0x2A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // r1 = 42
        ]
        full.append(contentsOf: bc)
        let out = try FluxBytecodeVM.run(full, signals: &signals, payload: .null, capRegistry: registry)
        XCTAssertEqual(signals.read(50), FluxHost.FluxValue.int(42))
        XCTAssertEqual(out.registers[0], FluxHost.FluxValue.int(42))
    }

    @MainActor
    func testUnregisteredCapErrors() async {
        // Only (1,1) is registered by `.dev`; (4,4) must raise a type error.
        var signals: any SignalStore = InMemorySignals()
        let bc: [UInt8] = [
            0x90, 0x00, 0x04, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, // capId=4, methodId=4
            0x00,
        ]
        XCTAssertThrowsError(try FluxBytecodeVM.run(bc, signals: &signals, payload: .null, capRegistry: .dev)) { error in
            XCTAssertEqual((error as? VmError)?.kind, .typeMismatch)
        }
    }
}

// MARK: - G5: lifecycle hooks

/// G5 — `onMount` runs on node creation, `onCleanup` runs on removal.
final class GapG5LifecycleTests: XCTestCase {
    @MainActor
    func testOnMountRunsOnBuild() async {
        // Handler 1 writes signal 5 = 1.
        let mountBc: [UInt8] = [
            0xB0, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // r0 = 1
            0x11, 0x05, 0x00, 0x00, 0x00, 0x00,                       // WRITE_SIGNAL 5, r0
            0x00,
        ]
        let closure = ClosureRef(
            hash: Array(repeating: 0, count: 8),
            bytecodeOffset: 0, bytecodeLen: UInt16(mountBc.count),
            signalCount: 0, signals: [], span: FluxSpan(fileId: 0, start: 0, end: 0), excerpt: nil
        )
        let node = gapNode(10, componentId: 0, props: [Prop(index: 0, value: .str(7))], mountHandler: 1)
        let frame = gapFrame(
            root: node,
            handlers: [HandlerDef(handlerId: 1, closure: closure, bytecode: mountBc)],
            strings: [StringEntry(stringId: 7, value: "hi")]
        )
        let executor = FluxExecutor(graph: SignalGraph(), registry: gapRegistry())
        executor.apply(frame)
        // onMount must have fired during apply, mutating signal 5.
        XCTAssertEqual(executor.graph.read(5), .int(1))
    }

    @MainActor
    func testOnCleanupRunsOnRemove() async {
        // Handler 2 writes signal 6 = 1.
        let cleanupBc: [UInt8] = [
            0xB0, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x11, 0x06, 0x00, 0x00, 0x00, 0x00,
            0x00,
        ]
        let closure = ClosureRef(
            hash: Array(repeating: 0, count: 8),
            bytecodeOffset: 0, bytecodeLen: UInt16(cleanupBc.count),
            signalCount: 0, signals: [], span: FluxSpan(fileId: 0, start: 0, end: 0), excerpt: nil
        )
        let node = gapNode(10, componentId: 0, props: [Prop(index: 0, value: .str(7))], cleanupHandler: 2)
        let frame = gapFrame(
            root: node,
            handlers: [HandlerDef(handlerId: 2, closure: closure, bytecode: cleanupBc)],
            strings: [StringEntry(stringId: 7, value: "hi")]
        )
        let executor = FluxExecutor(graph: SignalGraph(), registry: gapRegistry())
        executor.apply(frame)
        XCTAssertNil(executor.graph.read(6))

        executor.apply(FluxFrame(
            version: 1, seq: 1, flags: 0x00,
            root: nil, nodes: [:],
            patches: [.remove(id: 10)], handlers: [], strings: [], state: [], files: [], componentNames: [], signalMeta: [:]
        ))
        // onCleanup must have fired on removal.
        XCTAssertEqual(executor.graph.read(6), .int(1))
    }
}

// MARK: - G6: @pure subtree skip

/// G6 — a `@pure` node with unchanged props skips re-reconciling its subtree.
final class GapG6PureSkipTests: XCTestCase {
    @MainActor
    func testPureSubtreeSkippedOnStableProps() async {
        // A @pure parent wrapping a child Text. Both are stable across a
        // re-applied identical frame, so neither should be re-reconciled.
        let child = gapNode(11, componentId: 0, props: [Prop(index: 0, value: .str(7))])
        let pure = gapNode(10, componentId: 2, children: [.node(11)], isPure: true)
        let frame = gapFrame(
            root: pure, descendantNodes: [child],
            strings: [StringEntry(stringId: 7, value: "stable")]
        )
        let executor = FluxExecutor(graph: SignalGraph(), registry: gapRegistry())
        let first = executor.apply(frame)
        XCTAssertEqual(Set(first), Set([10, 11]))

        // Re-apply the SAME frame: the @pure subtree's props are unchanged, so
        // the reconciler must skip it — it must not appear in `updated`.
        let second = executor.apply(frame)
        XCTAssertFalse(second.contains(10), "@pure parent must be skipped on stable props")
        XCTAssertFalse(second.contains(11), "@pure child must be skipped on stable props")

        // A non-@pure control node IS re-reconciled when its props change (R2
        // only skips the update when the props are unchanged).
        let plain = gapNode(20, componentId: 0, props: [Prop(index: 0, value: .str(8))])
        let plainFrame = gapFrame(root: plain, strings: [StringEntry(stringId: 8, value: "x")])
        _ = executor.apply(plainFrame)
        let plainChanged = gapNode(20, componentId: 0, props: [Prop(index: 0, value: .str(99))])
        let plainFrameChanged = gapFrame(root: plainChanged, strings: [StringEntry(stringId: 99, value: "y")])
        let third = executor.apply(plainFrameChanged)
        XCTAssertTrue(third.contains(20), "non-@pure node must be re-reconciled when its props change")
    }
}

// MARK: - R1: dirty-set reconciliation

/// R1 — `dispatch` reconciles ONLY nodes whose signal dependencies were just
/// written, not the whole tree.
final class GapR1DirtySetTests: XCTestCase {
    /// Bytecode: LOAD_INT_CONST r0,1 ; WRITE_SIGNAL 5, r0 ; HALT.
    private let writeSignal5: [UInt8] = [
        0xB0, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x11, 0x05, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ]

    @MainActor
    func testDispatchReconcilesOnlySignalDependentNode() async {
        // Node 10 reads signal 5 via an int prop; node 11 is static. Both live
        // under a Column (20).
        let dependent = gapNode(10, componentId: 0, props: [Prop(index: 0, value: .int(5))])
        let unrelated = gapNode(11, componentId: 0, props: [Prop(index: 0, value: .str(7))])
        let root = gapNode(20, componentId: 2, children: [.node(10), .node(11)])
        let frame = gapFrame(
            root: root,
            descendantNodes: [dependent, unrelated],
            strings: [StringEntry(stringId: 7, value: "unrelated")]
        )
        let executor = FluxExecutor(graph: SignalGraph(), registry: gapRegistry())
        _ = executor.apply(frame)

        executor.registerHandler(
            1,
            closure: ClosureRef(hash: [], bytecodeOffset: 0, bytecodeLen: UInt16(writeSignal5.count),
                                signalCount: 0, signals: [], span: FluxSpan(fileId: 0, start: 0, end: 0), excerpt: nil),
            bytecode: writeSignal5
        )

        executor.dispatch(FluxEvent(handlerId: 1, nodeId: 20))
        await waitForDispatch(executor: executor) { !executor.lastReconcile.built.isEmpty || !executor.lastReconcile.updated.isEmpty }

        // Only the signal-dependent node (10) must have been reconciled; the
        // unrelated node (11) must be untouched (R1).
        let reconciled = executor.lastReconcile.built + executor.lastReconcile.updated
        XCTAssertTrue(reconciled.contains(10), "dependent node must be reconciled")
        XCTAssertFalse(reconciled.contains(11), "unrelated node must NOT be reconciled (R1)")
    }
}

// MARK: - R3: cached decoded bytecode

/// R3 — handler bytecode is decoded once at registration and reused on every
/// dispatch; re-registering a handler invalidates the cache.
final class GapR3CacheTests: XCTestCase {
    /// Bytecode: LOAD_INT_CONST r0, `value` ; WRITE_SIGNAL `id`, r0 ; HALT.
    private func writeSignal(_ id: UInt32, _ value: Int64) -> [UInt8] {
        var v = value
        let vBytes = withUnsafeBytes(of: &v) { Array($0) }
        return [
            0xB0, 0x00, vBytes[0], vBytes[1], vBytes[2], vBytes[3], vBytes[4], vBytes[5], vBytes[6], vBytes[7],
            0x11, UInt8(id & 0xFF), 0x00, 0x00, 0x00,
            0x00,
        ]
    }

    @MainActor
    func testReRegistrationInvalidatesDecodeCache() async {
        let executor = FluxExecutor(graph: SignalGraph(), registry: gapRegistry())

        // First registration: handler 1 writes signal 5 = 2.
        executor.registerHandler(
            1,
            closure: ClosureRef(hash: [], bytecodeOffset: 0, bytecodeLen: 0, signalCount: 0, signals: [], span: FluxSpan(fileId: 0, start: 0, end: 0), excerpt: nil),
            bytecode: writeSignal(5, 2)
        )
        executor.dispatch(FluxEvent(handlerId: 1, nodeId: 0))
        await waitForDispatch(executor: executor) { executor.graph.read(5) == .int(2) }
        XCTAssertEqual(executor.graph.read(5), .int(2), "first handler must write signal 5 = 2")

        // Re-registering the same id with different bytecode must invalidate the
        // cached decode and run the new body (signal 5 = 3, not 2).
        executor.registerHandler(
            1,
            closure: ClosureRef(hash: [], bytecodeOffset: 0, bytecodeLen: 0, signalCount: 0, signals: [], span: FluxSpan(fileId: 0, start: 0, end: 0), excerpt: nil),
            bytecode: writeSignal(5, 3)
        )
        executor.dispatch(FluxEvent(handlerId: 1, nodeId: 0))
        await waitForDispatch(executor: executor) { executor.graph.read(5) == .int(3) }
        XCTAssertEqual(executor.graph.read(5), .int(3), "re-registered handler must run NEW bytecode (R3 cache invalidation)")

        // A different handler id keeps its own cached decode (signal 6 = 9).
        executor.registerHandler(
            2,
            closure: ClosureRef(hash: [], bytecodeOffset: 0, bytecodeLen: 0, signalCount: 0, signals: [], span: FluxSpan(fileId: 0, start: 0, end: 0), excerpt: nil),
            bytecode: writeSignal(6, 9)
        )
        executor.dispatch(FluxEvent(handlerId: 2, nodeId: 0))
        await waitForDispatch(executor: executor) { executor.graph.read(6) == .int(9) }
        XCTAssertEqual(executor.graph.read(6), .int(9), "independent handler keeps its own cached decode")
    }
}

// MARK: - Capability round-trip (real MLP capability surface)

/// Exercises the full `CapabilityRegistry.dev` surface: `Storage` persistence,
/// `Router` navigation recording, and `Camera` capture — the on-device,
/// synchronous dev stand-ins for the real native backends (ADR-0045).
///
/// The `CapabilityImpl` signature returns a **result-cell signal id** (the
/// unified sync/async contract, ADR-0045); the VM stores that id in the result
/// register and the impl has already written the value into the cell. So these
/// tests assert the returned cell id AND the value written into the cell — never
/// the value as the bare return.
final class CapabilityRoundTripTests: XCTestCase {
    @MainActor
    func testStorageSetThenGetRoundTrips() async throws {
        var signals: any SignalStore = InMemorySignals()
        // Storage.set(key=Str(7), value=List[1,2,3]) → cap 2, method 1 returns cell id 95.
        let setArgs = FluxValue.record([(0, .str(7)), (1, .list([.int(1), .int(2), .int(3)]))])
        let written = try CapabilityRegistry.dev.lookup(2, 1)!(2, 1, setArgs, &signals)
        XCTAssertEqual(written, 95, "Storage.set returns its result-cell id")

        // Storage.get(key=Str(7)) → cap 2, method 2 exposes the persisted list via cell 95.
        let getArgs = FluxValue.record([(0, .str(7))])
        let gotCell = try CapabilityRegistry.dev.lookup(2, 2)!(2, 2, getArgs, &signals)
        XCTAssertEqual(gotCell, 95, "Storage.get returns its result-cell id")
        XCTAssertEqual(signals.read(95), .list([.int(1), .int(2), .int(3)]), "Storage.get returns the persisted value")
    }

    @MainActor
    func testRouterNavigateRecordsTarget() async throws {
        var signals: any SignalStore = InMemorySignals()
        let out = try CapabilityRegistry.dev.lookup(3, 1)!(3, 1, .str(42), &signals)
        XCTAssertEqual(out, 97, "Router.navigate returns its result-cell id")
        XCTAssertEqual(signals.read(97), .str(42), "Router.navigate records target string id in signal 97")
    }

    @MainActor
    func testCameraTakeEchoesForOracleParity() async throws {
        var signals: any SignalStore = InMemorySignals()
        let out = try CapabilityRegistry.dev.lookup(1, 1)!(1, 1, .record([(0, .int(7))]), &signals)
        XCTAssertEqual(out, 99, "Camera.take returns its result-cell id (99)")
        XCTAssertEqual(signals.read(99), .int(7), "Camera.take echoes into signal 99 (oracle parity)")
    }

    @MainActor
    func testStorageDeleteClearsValue() async throws {
        var signals: any SignalStore = InMemorySignals()
        let key = FluxValue.record([(0, .str(11))])
        let value = FluxValue.record([(0, .str(11)), (1, .list([.int(9)]))])
        _ = try CapabilityRegistry.dev.lookup(2, 1)!(2, 1, value, &signals)
        let beforeCell = try CapabilityRegistry.dev.lookup(2, 2)!(2, 2, key, &signals)
        XCTAssertEqual(beforeCell, 95, "Storage.get returns its result-cell id")
        XCTAssertEqual(signals.read(95), .list([.int(9)]), "value present before delete")
        _ = try CapabilityRegistry.dev.lookup(2, 3)!(2, 3, key, &signals)
        let afterCell = try CapabilityRegistry.dev.lookup(2, 2)!(2, 2, key, &signals)
        XCTAssertEqual(afterCell, 95, "Storage.get returns its result-cell id")
        XCTAssertEqual(signals.read(95), .null, "value cleared after delete")
    }

    /// LANE-C Task 1: `Storage` must persist across registry instances. We build
    /// a `UserDefaultsStorageBackend` over an isolated suite, write via one
    /// registry, drop it, recreate a registry over the SAME suite, and read the
    /// value back — proving it came from disk, not an in-memory cache.
    @MainActor
    func testStoragePersistsAcrossRegistryRecreation() async throws {
        let suite = "flux.lane-c.storage.\(UUID().uuidString)"
        defer { UserDefaults(suiteName: suite)?.removePersistentDomain(forName: suite) }

        let key = FluxValue.record([(0, .str(7))])
        let value = FluxValue.record([(0, .str(7)), (1, .list([.int(1), .int(2), .int(3)]))])

        // Write with the first registry (persistent backend).
        var firstSignals: any SignalStore = InMemorySignals()
        let first = CapabilityRegistry.makeDev(backend: UserDefaultsStorageBackend(suite: suite))
        _ = try first.lookup(2, 1)!(2, 1, value, &firstSignals)

        // Drop the registry instance entirely; only the disk suite survives.
        // A second registry over the same suite must observe the persisted value.
        var secondSignals: any SignalStore = InMemorySignals()
        let second = CapabilityRegistry.makeDev(backend: UserDefaultsStorageBackend(suite: suite))
        let gotCell = try second.lookup(2, 2)!(2, 2, key, &secondSignals)
        XCTAssertEqual(gotCell, 95, "Storage.get returns its result-cell id after recreation")
        XCTAssertEqual(
            secondSignals.read(95),
            .list([.int(1), .int(2), .int(3)]),
            "Storage value must survive registry recreation (real persistence)"
        )

        // Delete via the recreated registry; a fresh read must be null on disk.
        _ = try second.lookup(2, 3)!(2, 3, key, &secondSignals)
        var thirdSignals: any SignalStore = InMemorySignals()
        let third = CapabilityRegistry.makeDev(backend: UserDefaultsStorageBackend(suite: suite))
        _ = try third.lookup(2, 2)!(2, 2, key, &thirdSignals)
        XCTAssertEqual(thirdSignals.read(95), .null, "Storage.delete must clear the persisted value")
    }

    // MARK: - FLUX-050 / ADR-0056: fail-closed protocol version handshake

    /// A frame whose version byte the host does not implement must be rejected
    /// with `WireError.unsupportedVersion` — never mis-decoded into a tree.
    /// (Drives the real `FrameDeserializer`.)
    func testRejectsProtocolVersionMismatchFailClosed() {
        // magic(4) | version(1)=0x03 (unsupported) | kind(1)=0x02 (Init) | seq(4)=0
        // plus a minimal-but-valid-looking tail. The version gate fires before
        // any tree decoding, so the rest of the buffer need not be well-formed.
        var bytes: [UInt8] = []
        bytes += [0x58, 0x55, 0x5C, 0x46] // FLUX magic (LE)
        bytes += [0x03] // unsupported version (protocolVersion is 2)
        bytes += [0x02] // frame kind = Init
        bytes += [0x00, 0x00, 0x00, 0x00] // seq = 0
        bytes += [0x00, 0x00, 0x00, 0x00] // root node id
        bytes += [0x01] // root kind = Primitive
        bytes += [0x00, 0x00, 0x00, 0x00] // root component id
        bytes += [0x00, 0x00] // root prop count
        bytes += [0x00, 0x00] // root child count
        bytes += [0x00, 0x00] // root handler count
        bytes += [0x00, 0x00, 0x00, 0x00] // root span

        do {
            _ = try FrameDeserializer.decode(bytes)
            XCTFail("expected WireError.unsupportedVersion for version mismatch")
        } catch let err as WireError {
            guard case .unsupportedVersion = err else {
                XCTFail("version mismatch must surface as .unsupportedVersion, got \(err)"); return
            }
        } catch {
            XCTFail("version mismatch must surface as WireError, got \(error)")
        }
    }
}
