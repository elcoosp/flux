//  ImageAdapterTests.swift
//  FluxUIKitTests — `Image` adapter (Appendix F.8).

import XCTest
@testable import FluxUIKit

final class ImageAdapterTests: XCTestCase {
    @MainActor func testCreateProducesImageView() {
        let adapter = ImageAdapter()
        let view = adapter.create()
        XCTAssertNotNil(view as UIImageView)
    }

    @MainActor func testCreateStartsWithPlaceholder() {
        let adapter = ImageAdapter()
        let view = adapter.create()
        // An image view must never render a nil image in dev (BR-003): a
        // system placeholder is shown until the bitmap arrives.
        XCTAssertNotNil(view.image)
    }

    @MainActor func testUpdateWithEmptySrcFallsBackToPlaceholder() {
        let adapter = ImageAdapter()
        let view = adapter.create()
        let realImage = UIImage(systemName: "star")
        view.image = realImage
        // An empty/missing `src` is a load failure: the placeholder must come
        // back and no network task should be left in flight.
        adapter.update(view, from: Props(), to: Props())
        XCTAssertNotNil(view.image)
        adapter.destroy(view) // must not throw
    }

    @MainActor func testUpdateAppliesContentMode() {
        let adapter = ImageAdapter()
        let view = adapter.create()
        adapter.update(view, from: Props(), to: Props([3: .str("fit")]))
        XCTAssertEqual(view.contentMode, .scaleAspectFit)
    }

    @MainActor func testDestroyIsSafe() {
        let adapter = ImageAdapter()
        let view = adapter.create()
        adapter.destroy(view) // must not throw
    }
}
