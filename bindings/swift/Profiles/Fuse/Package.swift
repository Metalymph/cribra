// swift-tools-version: 6.4
import PackageDescription

// Build this profile with `just swift-apple <core|flow|fuse>` before resolving.
let package = Package(
    name: "CribraFuse",
    platforms: [.macOS(.v13), .iOS(.v16)],
    products: [.library(name: "Cribra", targets: ["Cribra"])],
    targets: [
        .binaryTarget(name: "CribraFFI", path: "Generated/CribraFFI.xcframework"),
        .target(name: "Cribra", dependencies: ["CribraFFI"], path: "Generated/Sources/Cribra"),
    ]
)
