// swift-tools-version: 6.0
// The native macOS front end. It links Patina's Rust backend as a static
// library (crates/ffi); build it with `macos/build.sh`, which builds the
// library first and bundles Patina.app.

import PackageDescription

let package = Package(
    name: "Patina",
    platforms: [.macOS(.v14)],
    targets: [
        .systemLibrary(name: "CPatina", path: "Sources/CPatina"),
        .executableTarget(
            name: "Patina",
            dependencies: ["CPatina"],
            path: "Sources/Patina",
            swiftSettings: [.swiftLanguageMode(.v5)],
            linkerSettings: [
                .linkedFramework("Security"),
                .linkedFramework("SystemConfiguration"),
                .linkedFramework("CoreFoundation"),
                .linkedLibrary("iconv"),
            ]
        ),
    ]
)
