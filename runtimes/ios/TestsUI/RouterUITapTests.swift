//  RouterUITapTests.swift
//  Definitive real-tap verification for Router navigation on iOS.
//
//  Unlike the programmatic mount test, this drives the *actual* app through the
//  real touch system (XCUIApplication.tap), so it exercises exactly what a user
//  tap does: hit-testing, the UIButton action, handler dispatch, signal 97
//  write, and the router re-filter. If this passes, tapping "Go to Settings"
//  really swaps the visible screen.

import XCTest

final class RouterUITapTests: XCTestCase {
    /// Taps "Go to Settings" and asserts the screen swaps to Settings (the
    /// "Go to Home" button becomes visible). Then taps back and asserts Home
    /// returns. This is the user's exact gesture, driven by the real touch path.
    @MainActor
    func testRealTapSwapsRouterScreen() throws {
        continueAfterFailure = false
        let app = XCUIApplication()
        app.launch()

        // The app connects to the dev server (ws://...:7331) at launch and
        // renders the `examples/router` tree. Wait for the Home title.
        let homeTitle = app.staticTexts["Home"]
        XCTAssertTrue(homeTitle.waitForExistence(timeout: 15), "Home screen did not appear — dev server / frame load failed")

        let goToSettings = app.buttons["Go to Settings"]
        XCTAssertTrue(goToSettings.waitForExistence(timeout: 5), "Go to Settings button not found")

        goToSettings.tap()

        // After the tap, the router should present Settings.
        let settingsTitle = app.staticTexts["Settings"]
        let goToHome = app.buttons["Go to Home"]
        let swapped = settingsTitle.waitForExistence(timeout: 5) || goToHome.waitForExistence(timeout: 5)
        XCTAssertTrue(swapped, "Tapping 'Go to Settings' did NOT swap to the Settings screen — router navigation is broken on a real tap")

        // And tapping back should return Home.
        if goToHome.exists {
            goToHome.tap()
            let homeBack = app.staticTexts["Home"].waitForExistence(timeout: 5)
            XCTAssertTrue(homeBack, "Tapping 'Go to Home' did NOT return to Home")
        }
    }
}
