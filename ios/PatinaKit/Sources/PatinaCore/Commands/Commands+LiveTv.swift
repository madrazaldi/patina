// Port of the "Live TV" section of desktop/commands/index.ts.

import Foundation
import PlexKit

extension Commands {
    /// Whether the server has a DVR set up.
    public static func livetvAvailable(_ app: AppState) async throws -> Bool {
        await LiveTv.available(try app.server())
    }

    /// Channels with what is on for the next `hours` (1 to 24).
    public static func livetvGuide(_ app: AppState, hours: Int) async throws -> [LiveTv.Channel] {
        try await LiveTv.guide(app.server(), min(24, max(1, hours)))
    }

    /// Tune a channel and play it; `title` names it if the tuner doesn't.
    public static func livetvWatch(_ app: AppState, dvr: String, channel: String, title: String) async throws {
        try await Ops.Playback.livetvWatch(app, dvr, channel, title)
    }

    public static func recordings(_ app: AppState) async throws -> LiveTv.Recordings {
        try await LiveTv.recordings(app.server())
    }

    /// Record a guide item with the server's default rule for it.
    public static func record(_ app: AppState, guid: String) async throws {
        try await LiveTv.record(app.server(), guid)
    }

    public static func deleteRecordingRule(_ app: AppState, id: String) async throws {
        try await LiveTv.deleteRule(app.server(), id)
    }
}
