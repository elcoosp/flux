//  PropsTests.swift
//  FluxUIKitTests — `Props` accessors (Appendix C §C.1).

import XCTest
@testable import FluxUIKit

final class PropsTests: XCTestCase {
    @MainActor func testGetStringResolvesConcreteText() {
        let props = Props([0: .str("hello")])
        XCTAssertEqual(props.getString(0), "hello")
    }

    @MainActor func testGetStringReturnsNilForNonString() {
        let props = Props([0: .int(42)])
        XCTAssertNil(props.getString(0))
    }

    @MainActor func testGetIntFloatBoolHandlerRoundTrip() {
        let props = Props([0: .int(-7), 1: .float(2.5), 2: .bool(true), 3: .handlerRef(9)])
        XCTAssertEqual(props.getInt(0), -7)
        XCTAssertEqual(props.getFloat(1), 2.5)
        XCTAssertEqual(props.getBool(2), true)
        XCTAssertEqual(props.getHandler(3), 9)
    }

    @MainActor func testMissingFieldReturnsNil() {
        let props = Props([0: .null])
        XCTAssertNil(props.getString(99))
    }

    @MainActor func testContentHashStableAcrossEqualMaps() {
        let a = Props([0: .str("x"), 1: .int(1)])
        let b = Props([1: .int(1), 0: .str("x")])
        XCTAssertEqual(a.hash, b.hash, "prop order must not affect content hash")
    }
}
