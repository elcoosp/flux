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
        (field.delegate as? TextInputAdapter.Delegate)?.textFieldDidChangeSelection(field)
        XCTAssertEqual(executor.dispatched.first?.handlerId, 4)
        XCTAssertEqual(executor.dispatched.first?.payload, .str("abc"))
    }
}
