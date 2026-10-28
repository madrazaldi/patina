// swift-tools-version: 6.2

import PackageDescription

// The iOS app's code, as a package so the parts below the UI build and test on
// macOS with `swift test` (no simulator needed). The app target in
// `ios/Patina.xcodeproj` only holds the entry point and the assets.
//
// PlexKit and SeerrKit are ports of `desktop/plex` and `desktop/seerr`
// (Sendable, nonisolated). PatinaCore is the port of `desktop/core`; like the
// JavaScript it replaces it runs on one thread, so it defaults to the main actor.

let mainActor: [SwiftSetting] = [
    .defaultIsolation(MainActor.self),
    .enableUpcomingFeature("NonisolatedNonsendingByDefault"),
    .enableUpcomingFeature("InferIsolatedConformances"),
]

let package = Package(
    name: "PatinaKit",
    platforms: [.iOS(.v18), .macOS(.v15)],
    products: [
        .library(name: "PatinaUI", targets: ["PatinaUI"]),
    ],
    dependencies: [
        // Playback: FFmpeg demux over AVPlayer / VideoToolbox. Pinned, as its README asks.
        .package(url: "https://github.com/superuser404notfound/AetherEngine", exact: "7.50.0"),
    ],
    targets: [
        .target(name: "PlexKit"),
        .target(name: "SeerrKit"),
        .target(name: "PatinaCore", dependencies: ["PlexKit", "SeerrKit"], swiftSettings: mainActor),
        .target(
            name: "PatinaPlayer",
            dependencies: ["PatinaCore", .product(name: "AetherEngine", package: "AetherEngine")],
            swiftSettings: mainActor
        ),
        .target(name: "PatinaUI", dependencies: ["PatinaCore", "PatinaPlayer"], swiftSettings: mainActor),
        .testTarget(name: "PlexKitTests", dependencies: ["PlexKit"]),
        .testTarget(name: "SeerrKitTests", dependencies: ["SeerrKit"]),
        .testTarget(name: "PatinaCoreTests", dependencies: ["PatinaCore"], swiftSettings: mainActor),
    ]
)
