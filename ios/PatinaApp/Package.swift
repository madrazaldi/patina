// swift-tools-version: 6.2

import PackageDescription

// The iOS-only half of the app: the player (over AetherEngine, with libass for ASS subtitles) and every screen.
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
        // ASS and SSA subtitles: libass (with FreeType, HarfBuzz and FriBidi) as prebuilt static libraries.
        .package(url: "https://github.com/mihai8804858/swift-libass", exact: "1.4.0"),
    ],
    targets: [
        .target(
            name: "PatinaPlayer",
            dependencies: [
                .product(name: "PatinaCore", package: "PatinaKit"),
                .product(name: "AetherEngine", package: "AetherEngine"),
                .product(name: "SwiftLibass", package: "swift-libass"),
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
