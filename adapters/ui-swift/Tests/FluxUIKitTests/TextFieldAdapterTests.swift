//  TextFieldAdapterTests.swift
//  FluxUIKitTests — `TextField` adapter (Appendix F.5).

import XCTest
@testable import FluxUIKit

final class TextFieldAdapterTests: XCTestCase {
    @MainActor func testUpdateSetsControlledTextAndPlaceholder() {
        let adapter = TextFieldAdapter()
        let field = adapter.create()
        let props = Props([0: .str("value"), 2: .str("Type…"), 5: .bool(true)])
        adapter.update(field, from: Props(), to: props)
        XCTAssertEqual(field.text, "value")
        XCTAssertEqual(field.placeholder, "Type…")
        XCTAssertTrue(field.isSecureTextEntry)
    }

    @MainActor func testEditingDispatchesOnChangeWithNewText() {
        let executor = MockExecutor()
        let adapter = TextFieldAdapter(executor: executor)
        let field = adapter.create()
        adapter.bindHandler(4, to: field, nodeId: 9)
        field.text = "abc"
        (field.delegate as? TextFieldAdapter.Delegate)?.textFieldDidChangeSelection(field)
        XCTAssertEqual(executor.dispatched.first?.handlerId, 4)
        XCTAssertEqual(executor.dispatched.first?.payload, .str("abc"))
    }
}
