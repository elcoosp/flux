// swift-tools-version:6.0
//
// FluxHost — the platform-independent Flux runtime engine (FA-RENDER Phase B).
//
// Holds the VM, signal graph, shadow tree, wire decoder and the executor that
// bridges them to the `FluxUIKit` adapter kit. It is a pure iOS-library package
// with no app-shell (SwiftUI/UIKit scene) code; the `FluxApp` application
// target depends on `FluxHost` and supplies the thin host shell.
//
// Type visibility: the engine is exercised by its own tests via
// `@testable import FluxHost`; the app shell reaches the public surface
// (`FluxRuntime`, `AdapterRegistry`, `SignalGraph`, …).

import PackageDescription

let package = Package(
    name: "FluxHost",
    platforms: [
        // Spec constraint C-002: iOS 16 is the minimum deployment target.
        .iOS(.v16)
    ],
    products: [
        .library(name: "FluxHost", targets: ["FluxHost"])
    ],
    dependencies: [
        .package(path: "../../../adapters/ui-swift")
    ],
    targets: [
        .target(
            name: "FluxHost",
            dependencies: [
                .product(name: "FluxUIKit", package: "ui-swift")
            ],
            path: "Sources/FluxHost",
            swiftSettings: [
                .swiftLanguageMode(.v6)
            ]
        ),
        .testTarget(
            name: "FluxHostTests",
            dependencies: ["FluxHost"],
            path: "Tests/FluxHostTests",
            swiftSettings: [.swiftLanguageMode(.v6)]
        )
    ]
)
