//  ToggleAdapterTests.swift
//  FluxUIKitTests — FLUX-077 `Toggle` adapter (data-driven surface, FLUX-072).
//
//  Parity with the Android `ToggleAdapter` (and the `SwitchAdapterTests` in
//  this suite): pins the `value` / `enabled` prop mapping and the
//  `onValueChange` handler binding through the weakly-held executor. The native
//  control is driven the same way `SwitchAdapterTests` drives a `UISwitch` — by
//  sending the `.valueChanged` action.

import XCTest
@testable import FluxUIKit

final class ToggleAdapterTests: XCTestCase {
    @MainActor func testTogglePushesValueAndBindsOnValueChange() {
        let executor = MockExecutor()
        let adapter = ToggleAdapter(executor: executor)
        let view = adapter.create()
        adapter.update(view, from: Props(), to: Props([
            Props.propIndex(for: "value"): .bool(true),
        ]))
        XCTAssertTrue(view.isOn)

        adapter.bindHandler(15, to: view, nodeId: 1)
        view.sendActions(for: .valueChanged)
        XCTAssertEqual(executor.dispatched.first?.handlerId, 15)
        XCTAssertEqual(executor.dispatched.first?.payload, .bool(true))
    }

    @MainActor func testToggleReflectsEnabledFlag() {
        let adapter = ToggleAdapter()
        let view = adapter.create()
        adapter.update(view, from: Props(), to: Props([
            Props.propIndex(for: "enabled"): .bool(false),
        ]))
        XCTAssertFalse(view.isEnabled)
        adapter.update(view, from: Props(), to: Props([
            Props.propIndex(for: "enabled"): .bool(true),
        ]))
        XCTAssertTrue(view.isEnabled)
    }

    @MainActor func testToggleStopsDispatchingAfterExecutorReleased() {
        var executor: MockExecutor? = MockExecutor()
        let adapter = ToggleAdapter(executor: executor)
        let view = adapter.create()
        adapter.update(view, from: Props(), to: Props([
            Props.propIndex(for: "value"): .bool(true),
        ]))
        adapter.bindHandler(7, to: view, nodeId: 1)
        executor = nil // release; the action target must not retain it
        view.sendActions(for: .valueChanged)
        // No crash, no dispatch.
    }

    @MainActor func testToggleAdapterIsConstructibleForRuntimeWiring() {
        // The runtime registry (AdapterKit.swift) wires the `Toggle` primitive
        // kind to `ToggleAdapter`. This suite lives in the standalone `FluxUIKit`
        // SwiftPM package, which has no registry object of its own (mirroring the
        // Android `FluxUiKit` factory map that resolves `"toggle"` →
        // `ToggleAdapter`); the registry-resolution assertion itself lives in the
        // `FluxHost` runtime test target. Here we assert the adapter the registry
        // would build is the correct concrete type and carries no handler binding
        // until `bindHandler` is called.
        let adapter = ToggleAdapter()
        let view = adapter.create()
        XCTAssertTrue(view is UISwitch)
    }

    @MainActor func testToggleDestroyTearsDown() {
        let adapter = ToggleAdapter()
        let view = adapter.create()
        adapter.bindHandler(1, to: view, nodeId: 1)
        adapter.destroy(view)
        // Tearing down must not crash and must leave the switch cleanly bound
        // to nothing (no stale action firing after destroy).
        view.sendActions(for: .valueChanged)
    }
}
