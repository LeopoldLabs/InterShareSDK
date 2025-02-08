// swift-tools-version: 5.7
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
    name: "InterShareKit",
    platforms: [
        .iOS(.v15),
        .macOS(.v12)
    ],
    products: [
        .library(
            name: "InterShareKit",
            targets: ["InterShareKit"]),
    ],
    dependencies: [],
    targets: [
        .target(
            name: "InterShareKit",
            dependencies: ["InterShareSDKFFI"],
            path: "./bindings/swift/Sources"
        ),

        .binaryTarget(
            name: "InterShareSDKFFI",
            path: "./bindings/swift/InterShareSDKFFI.xcframework"
        ),

        .testTarget(
            name: "InterShareSDKTests",
            dependencies: ["InterShareKit"],
            path: "./bindings/swift/Tests"
        ),
    ]
)
