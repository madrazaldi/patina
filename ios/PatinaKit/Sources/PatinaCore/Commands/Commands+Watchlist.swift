// Port of the "Watchlist" and "Playlists" sections of desktop/commands/index.ts.

import Foundation
import PlexKit

extension Commands {
    // MARK: Watchlist

    /// The plex.tv watchlist, up to 1000 items.
    public static func watchlist(_ app: AppState) async throws -> [Metadata] {
        try await Ops.Lists.watchlist(app)
    }

    public static func watchlisted(_ app: AppState, discoverId: String) async throws -> Bool {
        try await Ops.Lists.watchlisted(app, discoverId)
    }

    public static func setWatchlisted(_ app: AppState, discoverId: String, on: Bool) async throws {
        let tv = try app.plexTv()
        if on { try await tv.addToWatchlist(discoverId) } else { try await tv.removeFromWatchlist(discoverId) }
    }

    // MARK: Playlists

    /// All playlists, or the non-smart ones of one kind (`video`, `audio`, `photo`).
    public static func playlists(_ app: AppState, kind: String? = nil) async throws -> [Metadata] {
        try await app.server().playlists(kind, smart: kind != nil ? false : nil)
    }

    /// A playlist and its entries (up to `Ops.Lists.PLAYLIST_LIMIT`); `nil` when it is gone.
    public static func playlist(_ app: AppState, id: String) async throws -> Ops.Lists.PlaylistView? {
        try await Ops.Lists.playlist(app, id)
    }

    /// Add an item to an existing playlist, or to a new one called `title`.
    public static func addToPlaylist(
        _ app: AppState, ratingKey: String, playlist: String? = nil, title: String? = nil, kind: String = "video"
    ) async throws {
        try await Ops.Lists.addToPlaylist(app, ratingKey, playlist, title, kind)
    }

    /// Remove an entry (`itemId` is its `playlistItemID`); announces itself with a toast that offers Undo.
    public static func removeFromPlaylist(_ app: AppState, id: String, itemId: Int) async throws {
        try await Undo.removeFromPlaylist(app, id, itemId)
    }

    public static func deletePlaylist(_ app: AppState, id: String) async throws {
        try await app.server().deletePlaylist(id)
    }

    /// Move an entry after another (`after` is a `playlistItemID`), or to the top.
    public static func movePlaylistItem(_ app: AppState, id: String, itemId: Int, after: Int? = nil) async throws {
        try await app.server().movePlaylistItem(id, playlistItemId: itemId, after: after)
    }

    public static func updatePlaylist(_ app: AppState, id: String, title: String, summary: String) async throws {
        try await app.server().updatePlaylist(
            id, title: title.trimmingCharacters(in: .whitespacesAndNewlines),
            summary: summary.trimmingCharacters(in: .whitespacesAndNewlines))
    }

    public static func clearPlaylist(_ app: AppState, id: String) async throws {
        try await app.server().clearPlaylist(id)
    }
}
