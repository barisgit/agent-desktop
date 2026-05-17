// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "cursor-overlay",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(name: "cursor-overlay", targets: ["cursor-overlay"])
    ],
    targets: [
        .executableTarget(
            name: "cursor-overlay",
            path: "Sources/cursor-overlay"
        )
    ]
)
