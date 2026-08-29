//  WebHostViewTests.swift
//  FluxUIKitTests — `WebHost` adapter (FLUX-048).

import XCTest
import WebKit
@testable import FluxUIKit

final class WebHostViewTests: XCTestCase {
    @MainActor func testCreateProducesWebView() {
        let adapter = WebHostView()
        let view = adapter.create()
        XCTAssertNotNil(view as WKWebView)
    }

    @MainActor func testUpdateWithHttpSrcLoadsWithoutThrowing() {
        let adapter = WebHostView()
        let view = adapter.create()
        // A valid http(s) src must navigate without throwing; the actual
        // network load is async and irrelevant to the adapter contract.
        adapter.update(view, from: Props(), to: Props([Props.propIndex(for: "src"): .str("https://example.com")]))
        adapter.destroy(view) // must not throw
    }

    @MainActor func testUpdateWithEmptySrcIsNoOp() {
        let adapter = WebHostView()
        let view = adapter.create()
        // An empty/missing src is a no-op (graceful degrade), not a crash.
        adapter.update(view, from: Props(), to: Props())
        adapter.destroy(view) // must not throw
    }
}
