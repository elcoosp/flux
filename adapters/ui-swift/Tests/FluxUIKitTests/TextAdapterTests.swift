//  TextAdapterTests.swift
//  FluxUIKitTests — `Text` adapter (Appendix F.1).

import XCTest
@testable import FluxUIKit

final class TextAdapterTests: XCTestCase {
    @MainActor func testCreateProducesLabel() {
        let adapter = TextAdapter()
        XCTAssertNotNil(adapter.create())
    }

    @MainActor func testUpdateSetsTextAndColorAndFont() {
        let adapter = TextAdapter()
        let label = adapter.create()
        let props = Props([
            0: .str("Hi"),
            3: .record(Props([0: .float(1), 1: .float(0), 2: .float(0)])), // red
            1: .record(Props([0: .float(18)])), // font size 18
        ])
        adapter.update(label, from: Props(), to: props)
        XCTAssertEqual(label.text, "Hi")
        XCTAssertEqual(label.textColor, UIColor.red)
        XCTAssertEqual(label.font.pointSize, 18)
    }

    @MainActor func testDestroyIsSafe() {
        let adapter = TextAdapter()
        let label = adapter.create()
        adapter.destroy(label) // must not throw
    }
}
