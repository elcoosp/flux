//  FormGestureAdapterTests.swift
//  FluxUIKitTests — FLUX-040 form + FLUX-041 gesture adapters (PRD-N family).
//
//  Parity with the Android `FormGestureAdapterTest` (the kit's JVM suite): each
//  test pins the adapter's `update` prop mapping, handler binding through the
//  weakly-held executor, executor disposal no-op, and `Gesture` keyed child
//  reconciliation. Controls are driven the same way `ButtonAdapterTests` drives
//  a `UIButton` — by sending the bound action / invoking the recognizer.

import XCTest
@testable import FluxUIKit

final class FormGestureAdapterTests: XCTestCase {
    // --- Switch (FLUX-040) ---

    @MainActor func testSwitchPushesValueAndBindsOnChange() {
        let executor = MockExecutor()
        let adapter = SwitchAdapter(executor: executor)
        let view = adapter.create()
        adapter.update(view, from: Props(), to: Props([Props.propIndex(for: "value"): .bool(true)]))
        XCTAssertTrue(view.isOn)

        adapter.bindHandler(11, to: view, nodeId: 1)
        view.sendActions(for: .valueChanged)
        XCTAssertEqual(executor.dispatched.first?.handlerId, 11)
        XCTAssertEqual(executor.dispatched.first?.payload, .bool(true))
    }

    @MainActor func testSwitchReflectsEnabledFlag() {
        let adapter = SwitchAdapter()
        let view = adapter.create()
        adapter.update(view, from: Props(), to: Props([Props.propIndex(for: "enabled"): .bool(false)]))
        XCTAssertFalse(view.isEnabled)
        adapter.update(view, from: Props(), to: Props([Props.propIndex(for: "enabled"): .bool(true)]))
        XCTAssertTrue(view.isEnabled)
    }

    // --- Checkbox (FLUX-040) ---

    @MainActor func testCheckboxPushesValueAndBindsOnChange() {
        let executor = MockExecutor()
        let adapter = CheckboxAdapter(executor: executor)
        let view = adapter.create()
        adapter.update(view, from: Props(), to: Props([
            Props.propIndex(for: "value"): .bool(true),
            Props.propIndex(for: "label"): .str("Accept"),
        ]))
        XCTAssertTrue(view.isSelected)

        adapter.bindHandler(5, to: view, nodeId: 1)
        view.sendActions(for: .touchUpInside)
        XCTAssertEqual(executor.dispatched.first?.handlerId, 5)
        XCTAssertEqual(executor.dispatched.first?.payload, .bool(true))
    }

    // --- Slider (FLUX-040) ---

    @MainActor func testSliderPushesBoundsAndBindsOnChange() {
        let executor = MockExecutor()
        let adapter = SliderAdapter(executor: executor)
        let view = adapter.create()
        adapter.update(view, from: Props(), to: Props([
            Props.propIndex(for: "value"): .float(0.5),
            Props.propIndex(for: "min"): .float(0.0),
            Props.propIndex(for: "max"): .float(1.0),
            Props.propIndex(for: "step"): .float(0.1),
        ]))
        XCTAssertEqual(view.minimumValue, 0.0)
        XCTAssertEqual(view.maximumValue, 1.0)
        XCTAssertEqual(Double(view.value), 0.5, accuracy: 0.0001)

        adapter.bindHandler(8, to: view, nodeId: 1)
        view.sendActions(for: .valueChanged)
        XCTAssertEqual(executor.dispatched.first?.handlerId, 8)
        XCTAssertEqual(executor.dispatched.first?.payload, .float(Double(view.value)))
    }

    // --- Picker (FLUX-040) ---

    @MainActor func testPickerPushesItemsAndBindsOnChange() {
        let executor = MockExecutor()
        let adapter = PickerAdapter(executor: executor)
        let view = adapter.create()
        adapter.update(view, from: Props(), to: Props([
            Props.propIndex(for: "value"): .int(1),
            Props.propIndex(for: "items"): .list([.str("a"), .str("b")]),
        ]))
        XCTAssertEqual(view.selectedRow(inComponent: 0), 1)

        adapter.bindHandler(9, to: view, nodeId: 1)
        // Simulate a user selection of row 0.
        (view.delegate as? PickerAdapter.Source)?.pickerView(view, didSelectRow: 0, inComponent: 0)
        XCTAssertEqual(executor.dispatched.first?.handlerId, 9)
        XCTAssertEqual(executor.dispatched.first?.payload, .int(0))
    }

    // --- DatePicker (FLUX-040) ---

    @MainActor func testDatePickerPushesValueAndBindsOnChange() {
        let executor = MockExecutor()
        let adapter = DatePickerAdapter(executor: executor)
        let view = adapter.create()
        adapter.update(view, from: Props(), to: Props([Props.propIndex(for: "value"): .int(1000)]))
        XCTAssertEqual(Int64(view.date.timeIntervalSince1970 * 1000), 1000)

        adapter.bindHandler(12, to: view, nodeId: 1)
        view.sendActions(for: .valueChanged)
        XCTAssertEqual(executor.dispatched.first?.handlerId, 12)
        XCTAssertEqual(executor.dispatched.first?.payload, .int(Int64(view.date.timeIntervalSince1970 * 1000)))
    }

    // --- TextArea (FLUX-040) ---

    @MainActor func testTextAreaPushesValueAndBindsOnChange() {
        let executor = MockExecutor()
        let adapter = TextAreaAdapter(executor: executor)
        let view = adapter.create()
        adapter.update(view, from: Props(), to: Props([
            Props.propIndex(for: "value"): .str("hello"),
            Props.propIndex(for: "placeholder"): .str("Notes"),
        ]))
        XCTAssertEqual(view.text, "hello")

        adapter.bindHandler(6, to: view, nodeId: 1)
        view.text = "updated"
        (view.delegate as? TextAreaAdapter.Delegate)?.textViewDidChange(view)
        XCTAssertEqual(executor.dispatched.first?.handlerId, 6)
        XCTAssertEqual(executor.dispatched.first?.payload, .str("updated"))
    }

    // --- Gesture (FLUX-041) ---

    @MainActor func testGestureDeclaresKindAndBindsOnGesture() {
        let executor = MockExecutor()
        let adapter = GestureAdapter(executor: executor)
        let view = adapter.create()
        adapter.update(view, from: Props(), to: Props([
            Props.propIndex(for: "kind"): .str("longPress"),
            Props.propIndex(for: "threshold"): .float(0.5),
        ]))
        XCTAssertTrue(view.gestureRecognizers?.contains { $0 is UILongPressGestureRecognizer } ?? false)

        adapter.bindHandler(21, to: view, nodeId: 1)
        // Fire the bound recognizer target directly (UIKit would do this on a real gesture).
        view.gestureEnvironment?.handlerTarget?.fire()
        XCTAssertEqual(executor.dispatched.first?.handlerId, 21)
    }

    @MainActor func testGestureReconcilesChildrenByIdentity() {
        let adapter = GestureAdapter()
        let view = adapter.create()
        let childA = UIView()
        let childB = UIView()
        adapter.setChildren([childA, childB], on: view)
        XCTAssertEqual(view.subviews, [childA, childB])

        // Reorder: children swapped; the SAME instances must be reused (no recreation).
        adapter.setChildren([childB, childA], on: view)
        XCTAssertEqual(view.subviews, [childB, childA], "reorder must not recreate child views")
        XCTAssertTrue(view.subviews.contains(childA), "existing instance reused")
        XCTAssertTrue(view.subviews.contains(childB), "existing instance reused")
    }

    @MainActor func testGestureStopsDispatchingAfterExecutorReleased() {
        var executor: MockExecutor? = MockExecutor()
        let adapter = GestureAdapter(executor: executor)
        let view = adapter.create()
        adapter.update(view, from: Props(), to: Props([Props.propIndex(for: "kind"): .str("longPress")]))
        adapter.bindHandler(3, to: view, nodeId: 1)
        executor = nil // release; the recognizer target must not retain it
        view.gestureEnvironment?.handlerTarget?.fire()
        // No crash, no dispatch.
    }

    @MainActor func testDestroyTearsDownAcrossAdapters() {
        // Each adapter is torn down on its own concrete instance (the `View`
        // associated type prevents a uniform existential here).
        do { let a = SwitchAdapter(); let v = a.create(); a.bindHandler(1, to: v, nodeId: 1); a.destroy(v) }
        do { let a = CheckboxAdapter(); let v = a.create(); a.bindHandler(1, to: v, nodeId: 1); a.destroy(v) }
        do { let a = SliderAdapter(); let v = a.create(); a.bindHandler(1, to: v, nodeId: 1); a.destroy(v) }
        do { let a = PickerAdapter(); let v = a.create(); a.bindHandler(1, to: v, nodeId: 1); a.destroy(v) }
        do { let a = DatePickerAdapter(); let v = a.create(); a.bindHandler(1, to: v, nodeId: 1); a.destroy(v) }
        do { let a = TextAreaAdapter(); let v = a.create(); a.bindHandler(1, to: v, nodeId: 1); a.destroy(v) }
        do { let a = GestureAdapter(); let v = a.create(); a.bindHandler(1, to: v, nodeId: 1); a.destroy(v) }
    }
}
