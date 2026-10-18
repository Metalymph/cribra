// swift-tools-version: 6.4
import PackageDescription

// The driver copies the selected standalone distribution next to this consumer.
let package = Package(
    name: "CribraExternalConsumer",
    platforms: [.macOS(.v13), .iOS(.v16)],
    dependencies: [.package(path: "../CribraDistribution")],
    targets: [
        .executableTarget(
            name: "Consumer",
            dependencies: [.product(name: "Cribra", package: "CribraDistribution")],
            swiftSettings: [
                // PROFILE_DEFINES
            ]
        ),
    ]
)
