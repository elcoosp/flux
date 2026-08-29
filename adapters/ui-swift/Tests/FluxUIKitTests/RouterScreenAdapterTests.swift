//  RouterScreenAdapterTests.swift
//  FluxUIKitTests — `Router`/`Screen` navigation (Appendix F.6/F.7).

import XCTest
@testable import FluxUIKit

final class RouterScreenAdapterTests: XCTestCase {
    @MainActor func testRouterCreateIsNavigationController() {
        let adapter = RouterAdapter()
        XCTAssertNotNil(adapter.create())
    }

    @MainActor func testRouterPushPreservesExistingScreenState() {
        let adapter = RouterAdapter()
        let nav = adapter.create()
        let home = UIViewController()
        home.title = "home"
        adapter.setChildren([home], on: nav)
        XCTAssertEqual(nav.nav.viewControllers, [home])

        let detail = UIViewController()
        adapter.setChildren([home, detail], on: nav)
        XCTAssertEqual(nav.nav.viewControllers, [home, detail])
        // `home` must be the same instance — its state is preserved.
        XCTAssertTrue(nav.nav.viewControllers.first === home)
    }

    @MainActor func testRouterPopRemovesLeavingScreen() {
        let adapter = RouterAdapter()
        let nav = adapter.create()
        let home = UIViewController()
        let detail = UIViewController()
        adapter.setChildren([home, detail], on: nav)
        adapter.setChildren([home], on: nav)
        XCTAssertEqual(nav.nav.viewControllers, [home])
        XCTAssertNil(detail.parent)
    }

    @MainActor func testScreenHostsChildContent() {
        let adapter = ScreenAdapter()
        let vc = adapter.create()
        let content = UIView()
        adapter.setChildren([content], on: vc)
        XCTAssertTrue(vc.view.subviews.contains(content))
    }
}
