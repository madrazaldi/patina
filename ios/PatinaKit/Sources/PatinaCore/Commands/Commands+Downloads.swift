// Port of the "Downloads" section of desktop/commands/index.ts.

import Foundation
import PlexKit

extension Commands {
    public static func downloads(_ app: AppState) -> [Download] {
        Downloads.list(app.paths)
    }

    /// Queue an item, or every playable item of a show, season, album or playlist. Returns how many were added.
    public static func download(_ app: AppState, ratingKey: String) async throws -> Int {
        try await Downloads.enqueue(app, ratingKey: ratingKey)
    }

    public static func removeDownload(_ app: AppState, id: String) {
        Downloads.remove(app, id)
    }

    public static func pauseDownload(_ app: AppState, id: String) {
        Downloads.pause(app, id)
    }

    public static func retryDownload(_ app: AppState, id: String) {
        Downloads.retry(app, id)
    }

    /// Play downloads offline, in the order given.
    public static func playDownloads(_ app: AppState, ids: [String]) throws {
        try Ops.Playback.playDownloads(app, ids)
    }

    /// Show the downloads folder (`Documents/Downloads`, in the Files app).
    public static func openDownloadsFolder(_ app: AppState, _ host: any Host) async throws {
        let dir = app.paths.downloads
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        try await host.openPath(dir.path)
    }
}
