//  RenderPerfHarnessTests.swift
//  FLUX-066 on-device render-perf harness — iOS host side.
//
//  Builds a fixed warm fixture tree (column root -> N Text leaves), each leaf
//  subscribing to a distinct signal via the ADR-0027 `signalMeta` deps, then
//  drives the REAL `ShadowTreeReconciler.reconcileDirty` (the in-place
//  prop-observation path from AGENTS.md §3.10) and times it on a booted simulator
//  (UILabel/UIStackView are real `UIView`s here). The observed latencies are
//  emitted as a `MetricRecord`-shaped JSON document (the same schema
//  `flux-perf-harness` consumes) and the §3.10 `NodeMutation` budget (p95 <= 3 ms)
//  is asserted.
//
//  This is a genuine measurement of the production reconciler, closing the
//  "demonstration, not a measurement" gap left by PRD-J's `ci_run` example.

import XCTest
import UIKit
import FluxUIKit

@testable import FluxHost

/// Builds a shadow node for a stdlib primitive, given its component id + props.
@MainActor
private func mountNode(
    _ id: UInt32,
    componentId: UInt32,
    kind: NodeKind = .primitive,
    props: [Prop] = [],
    children: [Child] = []
) -> ShadowNode {
    ShadowNode(
        id: id,
        kind: kind,
        componentId: componentId,
        props: props,
        childCount: UInt16(children.count),
        children: children,
        handlerCount: 0,
        handlers: [],
        span: FluxSpan(fileId: 0, start: 0, end: 0)
    )
}

final class RenderPerfHarnessTests: XCTestCase {

    /// `tree_size` used in the emitted record (root + leaves).
    private let leafCount = 24
    /// Leaf signal ids start here; leaf `i` subscribes to `leafSignalBase + i`.
    private let leafSignalBase: UInt32 = 0x400

    /// Builds the warm fixture: a Column root with `leafCount` Text leaves, each
    /// Text subscribing to a distinct signal via the `signalMeta` deps. Returns the
    /// reconciler with the fixture already applied.
    @MainActor
    private func buildFixture() -> ShadowTreeReconciler {
        var nodes: [UInt32: ShadowNode] = [:]
        var meta: [UInt32: NodeSignalMeta] = [:]

        let root = mountNode(1, componentId: 2, children: (0 ..< leafCount).map { .node(UInt32(10 + $0)) })
        nodes[1] = root

        for i in 0 ..< leafCount {
            let id = UInt32(10 + i)
            nodes[id] = mountNode(id, componentId: 0, props: [Prop(index: 0, value: .str(7))])
            // Each leaf depends on a distinct signal so a write marks exactly it
            // dirty (R1 — `reconcileDirty` touches only `dependents[S]`).
            meta[id] = NodeSignalMeta(deps: [leafSignalBase + UInt32(i)], thunk: nil, layout: [])
        }

        let frame = FluxFrame(
            version: 1, seq: 0, flags: 0x01,
            root: root,
            nodes: nodes,
            patches: [], handlers: [],
            strings: [StringEntry(stringId: 7, value: "perf")],
            state: [], files: [],
            componentNames: [
                StringEntry(stringId: 0, value: "Text"),
                StringEntry(stringId: 2, value: "Column"),
            ],
            signalMeta: meta
        )

        var reconciler = ShadowTreeReconciler(registry: AdapterRegistry(table: StringTable()))
        _ = reconciler.apply(frame)
        return reconciler
    }

    @MainActor
    func testNodeMutationReconcileIsTimed() {
        var reconciler = buildFixture()

        // Warm up before timing so the first-call cost is excluded.
        for i in 0 ..< leafCount {
            let sig = leafSignalBase + UInt32(i)
            _ = reconciler.reconcileDirty(rootId: 1, signalIds: [sig])
        }

        let iterations = 200
        var latencies: [Double] = []
        latencies.reserveCapacity(iterations)
        for _ in 0 ..< iterations {
            // Touch a rotating leaf each iteration so the dirty set stays size 1.
            let sig = leafSignalBase + UInt32((latencies.count) % leafCount)
            let start = Date().timeIntervalSinceReferenceDate
            _ = reconciler.reconcileDirty(rootId: 1, signalIds: [sig])
            let elapsed = (Date().timeIntervalSinceReferenceDate - start) * 1000.0
            latencies.append(elapsed)
        }

        latencies.sort()
        let p95 = latencies[min(latencies.count - 1, Int(Double(latencies.count) * 0.95))]
        XCTAssertLessThan(p95, 3.0, "iOS node-mutation p95 \(p95)ms must stay under §3.10 3ms ceiling")

        // Emit a MetricRecord-shaped JSON for the Rust `ci_ondevice` gate.
        let samples = latencies.map { String(format: "{\"latency_ms\":%.4f,\"size\":1}", $0) }.joined(separator: ",")
        let json = """
        {"scenario":"IosImperativeDev","kind":"NodeMutation","tree_size":\(leafCount + 1),"samples":[\(samples)]}
        """
        print("RENDER_PERF \(json)")
    }
}
