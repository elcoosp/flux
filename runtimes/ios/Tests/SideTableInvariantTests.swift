//  SideTableInvariantTests.swift
//  Regression guard for the reconciler's per-node side tables.
//
//  The iOS reconciler keeps several `[UInt32: ...]` maps keyed by node id
//  (`built`, `nodeTable`, `signalDeps`, `thunkHandlerToNode`,
//  `forEachRowContext`). Every one of them must be bounded by the *current*
//  tree size — a hot-reload churn that replaces the tree with fresh ids must
//  not grow them without bound. Rounds 3–5 of the codebase audit found several
//  of these accumulated across an editing session, slowing the app and
//  eventually pressuring memory. This test drives a long edit storm and
//  asserts the maps stay bounded.

import XCTest
import UIKit

@testable import FluxHost
@testable import FluxApp

@MainActor
private func makeLeaf(
    _ id: UInt32,
    componentId: UInt32,
    props: [Prop] = []
) -> ShadowNode {
    ShadowNode(
        id: id,
        kind: .primitive,
        componentId: componentId,
        props: props,
        childCount: 0,
        children: [],
        handlerCount: 0,
        handlers: [],
        span: FluxSpan(fileId: 0, start: 0, end: 0)
    )
}

@MainActor
private func makeRoot(
    _ id: UInt32,
    childIds: [UInt32]
) -> ShadowNode {
    ShadowNode(
        id: id,
        kind: .primitive,
        componentId: 2, // Column
        props: [],
        childCount: UInt32(childIds.count),
        children: childIds.map { Child.node($0) },
        handlerCount: 0,
        handlers: [],
        span: FluxSpan(fileId: 0, start: 0, end: 0)
    )
}

@MainActor
private func makeInitFrame(rootId: UInt32) -> FluxFrame {
    let textId = rootId + 1
    let buttonId = rootId + 2
    let text = makeLeaf(textId, componentId: 0, props: [Prop(index: 0, value: .str(7))])
    let button = makeLeaf(buttonId, componentId: 1, props: [Prop(index: 0, value: .str(8))])
    let root = makeRoot(rootId, childIds: [textId, buttonId])
    let table = MaterializationStringTable()
    table.store(id: 0, value: "Text")
    table.store(id: 1, value: "Button")
    table.store(id: 2, value: "Column")
    table.store(id: 7, value: "tapped 0 times")
    table.store(id: 8, value: "Increment")
    return FluxFrame(
        version: 1, seq: 0, flags: 0x01,
        root: root,
        nodes: [rootId: root, textId: text, buttonId: button],
        patches: [], handlers: [],
        strings: [
            StringEntry(stringId: 7, value: "tapped 0 times"),
            StringEntry(stringId: 8, value: "Increment"),
        ],
        state: [], files: [],
        componentNames: [
            StringEntry(stringId: 0, value: "Text"),
            StringEntry(stringId: 1, value: "Button"),
            StringEntry(stringId: 2, value: "Column"),
        ],
        signalMeta: [:]
    )
}

@MainActor
private func makeReplaceDeltaFrame(
    oldRootId: UInt32,
    newRootId: UInt32,
    seq: UInt32
) -> FluxFrame {
    let textId = newRootId + 1
    let buttonId = newRootId + 2
    let text = makeLeaf(textId, componentId: 0, props: [Prop(index: 0, value: .str(7))])
    let button = makeLeaf(buttonId, componentId: 1, props: [Prop(index: 0, value: .str(8))])
    let root = makeRoot(newRootId, childIds: [textId, buttonId])
    // The differ emits `Replace { id: new_node.id, node: new_node }` for every
    // node in the freshly-lowered subtree. A hot-reload of a small file ships
    // three Replace patches (root + two children) whose ids all differ from the
    // previous frame's ids.
    return FluxFrame(
        version: 1, seq: seq, flags: 0x00,
        root: nil,
        nodes: [:],
        patches: [
            .replace(id: newRootId, node: root),
            .replace(id: textId, node: text),
            .replace(id: buttonId, node: button),
        ],
        handlers: [], strings: [], state: [], files: [],
        componentNames: [], signalMeta: [:]
    )
}

final class SideTableInvariantTests: XCTestCase {
    /// A hot-reload storm must not grow the reconciler's per-node side tables
    /// beyond the current tree size. Before the reachability-prune fix,
    /// `nodeTable` accumulated every replaced subtree's entries — a 60-edit
    /// storm on a 3-node tree left ~180 stale entries.
    @MainActor
    func testSideTablesStayBoundedAcrossEditStorm() async {
        let table = MaterializationStringTable()
        table.store(id: 0, value: "Text")
        table.store(id: 1, value: "Button")
        table.store(id: 2, value: "Column")
        table.store(id: 7, value: "tapped 0 times")
        table.store(id: 8, value: "Increment")

        let registry = AdapterRegistry(table: table)
        let executor = FluxExecutor(graph: SignalGraph(), registry: registry)
        var reconciler = ShadowTreeReconciler(
            registry: registry,
            executor: executor,
            table: table
        )

        // Seed with a full Init tree (root id 100, children 101 + 102).
        var rootId: UInt32 = 100
        _ = reconciler.apply(makeInitFrame(rootId: rootId))

        // 60 hot-reload edits, each replacing the whole tree with fresh ids —
        // the shape the differ emits when a source-span-shifting edit lands.
        for i in 1...60 {
            let nextId = UInt32(100 + i * 10)
            let delta = makeReplaceDeltaFrame(
                oldRootId: rootId,
                newRootId: nextId,
                seq: UInt32(i)
            )
            _ = reconciler.apply(delta)
            rootId = nextId
        }

        // (1) The reconciler's authoritative tables must track the CURRENT
        // tree, not the first frame's ids. After 60 edits the root id is
        // `rootId`; if `nodeTable` still holds 100/101/102 it is pointing at
        // destroyed nodes, and a subsequent patch addressed to the current
        // root would fail to resolve. This caught a real regression: a
        // per-destroy `nodeTable` prune ran on the *new* id of each Replace,
        // deleting the entry the frame-boundary assignment had just added.
        XCTAssertEqual(
            reconciler.debugCurrentRootId, rootId,
            "currentRootId must track the latest replace's new root"
        )
        XCTAssertTrue(
            reconciler.debugNodeTableKeys.contains(rootId),
            "nodeTable must contain the current root id \(rootId)"
        )

        // (2) Counts must stay bounded — no accumulation across the storm.
        let counts = reconciler.debugSideTableCounts
        let budget = 10
        for (name, count) in counts {
            XCTAssertLessThanOrEqual(
                count, budget,
                "side table '\(name)' grew to \(count) entries for a 3-node tree — "
                + "the edit storm leaked. Full counts: \(counts)"
            )
        }
    }
}
