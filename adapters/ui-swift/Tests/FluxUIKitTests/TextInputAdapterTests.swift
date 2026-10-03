//  TextInputAdapterTests.swift
//  FluxUIKitTests — `TextInput` adapter (Appendix F.5).

import XCTest
@testable import FluxUIKit

final class TextInputAdapterTests: XCTestCase {
    @MainActor func testUpdateSetsControlledTextAndPlaceholder() {
        let adapter = TextInputAdapter()
        let field = adapter.create()
        let props = Props([
            Props.propIndex(for: "text"): .str("value"),
            Props.propIndex(for: "placeholder"): .str("Type…"),
            Props.propIndex(for: "secureTextEntry"): .bool(true),
        ])
        adapter.update(field, from: Props(), to: props)
        XCTAssertEqual(field.text, "value")
        XCTAssertEqual(field.placeholder, "Type…")
        XCTAssertTrue(field.isSecureTextEntry)
    }

    @MainActor func testEditingDispatchesOnChangeTextWithNewText() {
        let executor = MockExecutor()
        let adapter = TextInputAdapter(executor: executor)
        let field = adapter.create()
        adapter.bindHandler(4, to: field, nodeId: 9)
        field.text = "abc"
        // Simulate the real UIKit user-edit signal (fires on every keystroke);
        // the adapter registers an `.editingChanged` action that dispatches the
        // field's current text as the `onChangeText` payload. This replaces the
        // old test's call into a delegate callback removed by Audit D19 — the
        // suite previously couldn't compile, which is exactly why the dead
        // controlled-input loop went unnoticed on iOS.
        field.sendActions(for: .editingChanged)
        XCTAssertEqual(executor.dispatched.first?.handlerId, 4)
        XCTAssertEqual(executor.dispatched.first?.payload, .str("abc"))
    }
}
