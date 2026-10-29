// Port of the "Playback" and "Bulk audio & subtitles" sections of desktop/commands/index.ts.
// `skip_marker` and `select_track` are iOS additions: the screen draws the skip button and the
// track menus that mpv drew on the desktop. `play_downloads` and `livetv_watch` belong to the
// Downloads and Live TV sections; they call `Ops.Playback.playDownloads` and `.livetvWatch`.

import Foundation
import PlexKit

extension Commands {
    public static func play(_ app: AppState, request: Player.PlayRequest) {
        Player.play(app, request)
    }

    public static func control(_ app: AppState, command: Player.Control) {
        Player.control(app, command)
    }

    /// Volume, speed, chapters, repeat and the sleep timer.
    public static func tune(_ app: AppState, command: Controls.Tune) {
        Player.tune(app, command)
    }

    /// The playing item's controls and diagnostics; also pushed as `CoreEvent.playbackState`.
    public static func playbackState(_ app: AppState) -> PlaybackState? {
        Controls.playbackState(app)
    }

    /// The playback failure waiting for a decision; also pushed as `CoreEvent.playbackFailure`.
    public static func playbackFailure(_ app: AppState) -> PlaybackFailure? {
        Failure.current(app)
    }

    public static func recoverPlayback(_ app: AppState, action: Failure.Recover) throws {
        try Failure.recover(app, action)
    }

    public static func dismissFailure(_ app: AppState) {
        Failure.clear(app)
    }

    /// The queue saved by the last playback, to offer a Resume.
    public static func savedQueue(_ app: AppState) -> SavedQueueView? {
        Ops.Playback.savedQueue(app)
    }

    public static func resumeSaved(_ app: AppState) throws {
        try Ops.Playback.resumeSaved(app)
    }

    public static func forgetSaved(_ app: AppState) {
        Ops.Playback.forgetSaved(app)
    }

    public static func queue(_ app: AppState, command: Player.QueueCommand) {
        Player.queue(app, command)
    }

    public static func queueItems(_ app: AppState, items: [Metadata]) throws {
        try Player.queueItems(app, items)
    }

    public static func lyrics(_ app: AppState, ratingKey: String) async throws -> Lyrics.Lyrics? {
        try await Lyrics.load(app.server(), ratingKey)
    }

    /// Skip the intro or credits the screen offers (`NowPlaying.marker`).
    public static func skipMarker(_ app: AppState) {
        Player.skipMarker(app)
    }

    /// Switch the audio or subtitle track in the player; the choice is saved to Plex when the
    /// player settings sync streams.
    public static func selectTrack(_ app: AppState, choice: Player.TrackChoice) {
        Player.selectTrack(app, choice)
    }

    // MARK: Bulk audio & subtitles

    public static func tracksLoad(_ app: AppState, show: String) async throws {
        try await Tracks.load(app, show)
    }

    public static func tracksView(_ app: AppState, query: Tracks.Query) throws -> Tracks.View {
        try Tracks.view(app, query)
    }

    /// Save the planned changes, reporting `(done, total)` as they land.
    public static func tracksApply(
        _ app: AppState, changes: [Tracks.PartChange], progress: (Int, Int) -> Void
    ) async throws {
        try await Tracks.apply(app, changes, progress)
    }
}
