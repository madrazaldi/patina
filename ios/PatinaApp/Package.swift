// swift-tools-version: 6.2

import PackageDescription

// The iOS-only half of the app: the player (over AetherEngine) and every screen.
// The backend it drives is `ios/PatinaKit`.

let mainActor: [SwiftSetting] = [
    .defaultIsolation(MainActor.self),
    .enableUpcomingFeature("NonisolatedNonsendingByDefault"),
    .enableUpcomingFeature("InferIsolatedConformances"),
]

let package = Package(
    name: "PatinaApp",
    platforms: [.iOS(.v18)],
    products: [
        .library(name: "PatinaUI", targets: ["PatinaUI"]),
    ],
    dependencies: [
        .package(path: "../PatinaKit"),
        // Playback: FFmpeg demux over AVPlayer / VideoToolbox. Pinned, as its README asks.
        .package(url: "https://github.com/superuser404notfound/AetherEngine", exact: "7.50.0"),
    ],
    targets: [
        .target(
            name: "PatinaPlayer",
            dependencies: [
                .product(name: "PatinaCore", package: "PatinaKit"),
                .product(name: "AetherEngine", package: "AetherEngine"),
            ],
            swiftSettings: mainActor
        ),
        .target(
            name: "PatinaUI",
            dependencies: [
                "PatinaPlayer",
                .product(name: "PatinaCore", package: "PatinaKit"),
                .product(name: "PlexKit", package: "PatinaKit"),
                .product(name: "SeerrKit", package: "PatinaKit"),
            ],
            swiftSettings: mainActor
        ),
    ]
)
