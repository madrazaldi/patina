// swift-tools-version: 6.2

import PackageDescription

// The iOS app's backend, as a package with no dependencies so it builds and tests
// on macOS with `swift test` (no simulator needed). The screens and the player are
// in `ios/PatinaApp`, which depends on this.
//
// PlexKit and SeerrKit are ports of `desktop/plex` and `desktop/seerr` (Sendable,
// nonisolated). PatinaCore is the port of `desktop/core`; like the JavaScript it
// replaces it runs on one thread, so it defaults to the main actor.

let mainActor: [SwiftSetting] = [
    .defaultIsolation(MainActor.self),
    .enableUpcomingFeature("NonisolatedNonsendingByDefault"),
    .enableUpcomingFeature("InferIsolatedConformances"),
]

let package = Package(
    name: "PatinaKit",
    platforms: [.iOS(.v18), .macOS(.v15)],
    products: [
        .library(name: "PlexKit", targets: ["PlexKit"]),
        .library(name: "SeerrKit", targets: ["SeerrKit"]),
        .library(name: "PatinaCore", targets: ["PatinaCore"]),
    ],
    targets: [
        .target(name: "PlexKit"),
        .target(name: "SeerrKit"),
        .target(name: "PatinaCore", dependencies: ["PlexKit", "SeerrKit"], swiftSettings: mainActor),
        .testTarget(name: "PlexKitTests", dependencies: ["PlexKit"]),
        .testTarget(name: "SeerrKitTests", dependencies: ["SeerrKit"]),
        .testTarget(name: "PatinaCoreTests", dependencies: ["PatinaCore"], swiftSettings: mainActor),
    ]
)
