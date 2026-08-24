// swift-tools-version:6.0
// Frozen build manifest — created once by the foundation pass (FLUX-001).
// Agents may not modify this file (boundary contract R2).

import PackageDescription

let package = Package(
    name: "FluxUIKit",
    platforms: [
        // Spec constraint C-002: iOS 16 is the minimum deployment target.
        .iOS(.v16)
    ],
    products: [
        .library(name: "FluxUIKit", targets: ["FluxUIKit"])
    ],
    targets: [
        .target(
            name: "FluxUIKit",
            path: "Sources/FluxUIKit",
            swiftSettings: [
                // Swift 6 language mode: strict concurrency checking is on, so
                // adapter code must be explicit about actor isolation.
                .swiftLanguageMode(.v6)
            ]
        ),
        .testTarget(
            name: "FluxUIKitTests",
            dependencies: ["FluxUIKit"],
            path: "Tests/FluxUIKitTests",
            swiftSettings: [.swiftLanguageMode(.v6)]
        )
    ]
)
