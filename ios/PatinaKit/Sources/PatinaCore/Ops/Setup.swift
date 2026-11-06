// Port of desktop/core/ops/setup.ts.
// About the install: versions, paths and choices. The mpv check and its paths are not
// ported (there is no mpv on iOS); the app's own folders take their place.

import Foundation

extension Ops {
    public enum Setup {
        public nonisolated struct Choice: Sendable, Equatable {
            public var id: String
            public var label: String
            public var color: String?
        }

        public nonisolated struct Info: Sendable, Equatable {
            public var version: String
            public var configPath: String
            /// Checkpoints, saved queues, artwork of downloads.
            public var dataPath: String
            /// Downloaded media and saved diagnostics (Documents/Downloads in the Files app).
            public var downloadsPath: String
            public var qualities: [Choice]
            public var accents: [Choice]
            /// Debug builds: `PATINA_ROUTE=/item/100` opens a page on startup.
            public var debugRoute: String?
            /// Debug builds: `PATINA_PLAY=<ratingKey>` starts playback on startup.
            public var debugPlay: String?
        }

        /// Whether this is a development build (`cfg!(debug_assertions)`).
        #if DEBUG
            public nonisolated static let debugBuild = true
        #else
            public nonisolated static let debugBuild = false
        #endif

        /// About the install: versions, paths and choices.
        public nonisolated static func info(_ paths: Paths) -> Info {
            func debug(_ k: String) -> String? {
                debugBuild ? ProcessInfo.processInfo.environment[k] : nil
            }
            return Info(
                version: Config.VERSION, configPath: Config.configFile(paths).path, dataPath: paths.data.path,
                downloadsPath: paths.downloads.path,
                qualities: Config.QUALITIES.map { Choice(id: $0.id, label: $0.label, color: nil) },
                accents: Config.ACCENTS.map { Choice(id: $0.id, label: $0.label, color: $0.color) },
                debugRoute: debug("PATINA_ROUTE"), debugPlay: debug("PATINA_PLAY"))
        }
    }
}
