// STUB: the admin agent replaces this file with the port of desktop/core/ops/setup.ts.
// Only what Commands+App calls.

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
            public var mpvConfigDir: String
            public var mpvConfExists: Bool
            public var qualities: [Choice]
            public var accents: [Choice]
            /// Debug builds: `PATINA_ROUTE=/item/100` opens a page on startup.
            public var debugRoute: String?
            /// Debug builds: `PATINA_PLAY=<ratingKey>` starts playback on startup.
            public var debugPlay: String?
        }

        /// About the install: versions, paths and choices.
        public static func info(_ paths: Paths) -> Info {
            Info(
                version: Config.VERSION, configPath: Config.configFile(paths).path, mpvConfigDir: "", mpvConfExists: false,
                qualities: [], accents: [], debugRoute: nil, debugPlay: nil)
        }
    }
}
