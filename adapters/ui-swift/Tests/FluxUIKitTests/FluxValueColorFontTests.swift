//  FluxValueColorFontTests.swift
//  FluxUIKitTests — `FluxValue` / `FluxColor` / `FluxFount` decoding.

import XCTest
@testable import FluxUIKit

final class FluxValueColorFontTests: XCTestCase {
    @MainActor func testColorDecodesFromRecord() {
        let record = Props([0: .float(1), 1: .float(0.5), 2: .float(0), 3: .float(0.5)])
        let color = FluxColor(record: record)
        XCTAssertNotNil(color)
        XCTAssertEqual(color?.red, 1)
        XCTAssertEqual(color?.green, 0.5)
        XCTAssertEqual(color?.blue, 0)
        XCTAssertEqual(color?.alpha, 0.5)
    }

    @MainActor func testColorNilWhenChannelsMissing() {
        let record = Props([0: .float(1)])
        XCTAssertNil(FluxColor(record: record))
    }

    @MainActor func testColorClampsOutOfRangeChannels() {
        let record = Props([0: .float(2), 1: .float(-1), 2: .float(0.5)])
        let color = FluxColor(record: record)!
        XCTAssertEqual(color.red, 1)
        XCTAssertEqual(color.green, 0)
        XCTAssertEqual(color.alpha, 1)
    }

    @MainActor func testPropsGetColorHelper() {
        let props = Props([7: .record(Props([0: .float(0), 1: .float(0), 2: .float(0)]))])
        XCTAssertNotNil(props.getColor(7))
    }

    @MainActor func testFontDefaultsSizeTo14() {
        let font = FluxFount(record: Props([:]))
        XCTAssertEqual(font?.size, 14)
        XCTAssertEqual(font?.weight, .regular)
    }

    @MainActor func testFontDecodesWeightByName() {
        let record = Props([0: .float(20), 1: .str("bold")])
        let font = FluxFount(record: record)
        XCTAssertEqual(font?.size, 20)
        XCTAssertEqual(font?.weight, .bold)
    }
}
