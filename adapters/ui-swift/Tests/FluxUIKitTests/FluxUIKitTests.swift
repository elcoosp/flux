//  FluxUIKitTests.swift
//  FluxUIKitTests — adapter contract version check (Appendix F).

import XCTest
@testable import FluxUIKit

final class FluxUIKitTests: XCTestCase {
    func testAdapterContractVersionMatchesAppendixF() {
        XCTAssertEqual(FluxUIKitModule.adapterContractVersion, 1)
    }
}
