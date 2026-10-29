// The commands of desktop/commands/index.ts as the screens call them (no TypeScript twin of
// its own). The screens were written before the core modules were merged, so they call
// these; each keeps the command's name and arguments and forwards to `Commands`, adapting
// the shapes the screens use (`PlayOptions`, non-throwing availability checks).

import Foundation
import PatinaCore
import PlexKit

public enum PendingCommands {
    // MARK: Playback (merged: these forward to Commands)

    /// What `play` takes (`PlayOptions` in web/src/lib/api.ts).
    public struct PlayOptions: Sendable, Equatable {
        public var resume = true
        public var shuffle = false
        public var mediaIndex: Int?
        /// Play the item's radio station (music).
        public var radio = false

        public init(resume: Bool = true, shuffle: Bool = false, mediaIndex: Int? = nil, radio: Bool = false) {
            self.resume = resume
            self.shuffle = shuffle
            self.mediaIndex = mediaIndex
            self.radio = radio
        }
    }

    public typealias SavedQueueView = PatinaCore.SavedQueueView

    public static func play(_ app: AppState, item: Metadata, options: PlayOptions = PlayOptions()) async throws {
        PatinaCore.Commands.play(
            app,
            request: .init(
                item: item, resume: options.resume, shuffle: options.shuffle, mediaIndex: options.mediaIndex,
                radio: options.radio))
    }

    /// Queue an item (or everything in a container) next, or at the end (`queue` with `add`).
    public static func queueAdd(_ app: AppState, item: Metadata, next: Bool) async throws {
        PatinaCore.Commands.queue(app, command: .add(item: item, next: next))
    }

    /// Adding while nothing plays starts playback.
    public static func queueItems(_ app: AppState, items: [Metadata]) async throws {
        try PatinaCore.Commands.queueItems(app, items: items)
    }

    public static func savedQueue(_ app: AppState) async throws -> SavedQueueView? {
        PatinaCore.Commands.savedQueue(app)
    }

    public static func resumeSaved(_ app: AppState) async throws {
        try PatinaCore.Commands.resumeSaved(app)
    }

    public static func forgetSaved(_ app: AppState) async throws {
        PatinaCore.Commands.forgetSaved(app)
    }

    /// Index of the version (Media) the automatic playback preferences prefer.
    public static func preferredVersion(_ app: AppState, item: Metadata) async throws -> Int {
        PatinaCore.Commands.preferredVersion(app, item: item)
    }

    // MARK: Library browsing (merged: these forward to Commands+Library)

    /// A filter of a section (`FilterView` of desktop/core/ops/library.ts).
    public typealias FilterView = Ops.Library.FilterView

    /// Filters for a section and media type (genre, year, resolution, unwatched, ...).
    public static func sectionFilters(_ app: AppState, id: String, kind: Int? = nil) async throws -> [FilterView] {
        try await PatinaCore.Commands.sectionFilters(app, id: id, kind: kind)
    }

    public static func firstCharacters(_ app: AppState, id: String, kind: Int? = nil) async throws -> [Directory] {
        try await PatinaCore.Commands.firstCharacters(app, id: id, kind: kind)
    }

    /// A folder of a section (`FolderView`).
    public typealias FolderView = Ops.Library.FolderView

    /// A section's folder tree: the root, or the folder at `key`.
    public static func sectionFolders(_ app: AppState, id: String, key: String? = nil) async throws -> FolderView {
        try await PatinaCore.Commands.sectionFolders(app, id: id, key: key)
    }

    /// Extra hubs for a detail page, plus sonically similar music.
    public static func itemHubs(_ app: AppState, ratingKey: String, kind: String, server: String? = nil) async throws
        -> [Hub]
    {
        try await PatinaCore.Commands.itemHubs(app, server: server, ratingKey: ratingKey, kind: kind)
    }

    /// Scan a library's files (`section_action` with `scan`).
    public static func scanSection(_ app: AppState, id: String) async throws {
        try await PatinaCore.Commands.sectionAction(app, id: id, action: .scan)
    }

    // MARK: Watchlist (merged: these forward to Commands+Watchlist)

    public static func watchlisted(_ app: AppState, discoverId: String) async throws -> Bool {
        try await PatinaCore.Commands.watchlisted(app, discoverId: discoverId)
    }

    public static func setWatchlisted(_ app: AppState, discoverId: String, on: Bool) async throws {
        try await PatinaCore.Commands.setWatchlisted(app, discoverId: discoverId, on: on)
    }

    // MARK: Playlists (merged: these forward to Commands+Watchlist)

    /// A playlist and its entries (`PlaylistView` of desktop/core/ops/lists.ts).
    public typealias PlaylistView = Ops.Lists.PlaylistView

    /// All playlists, or the non-smart ones of one kind (`video`, `audio`, `photo`).
    public static func playlists(_ app: AppState, kind: String? = nil) async throws -> [Metadata] {
        try await PatinaCore.Commands.playlists(app, kind: kind)
    }

    public static func playlist(_ app: AppState, id: String) async throws -> PlaylistView? {
        try await PatinaCore.Commands.playlist(app, id: id)
    }

    /// Add items to an existing playlist, or to a new one called `title` of `kind`.
    /// `ratingKey` may list several, comma-separated.
    public static func addToPlaylist(
        _ app: AppState, ratingKey: String, kind: String, playlist: String? = nil, title: String? = nil
    ) async throws {
        try await PatinaCore.Commands.addToPlaylist(app, ratingKey: ratingKey, playlist: playlist, title: title, kind: kind)
    }

    /// Remove an entry; announces itself with a toast that offers Undo.
    public static func removeFromPlaylist(_ app: AppState, id: String, itemId: Int) async throws {
        try await PatinaCore.Commands.removeFromPlaylist(app, id: id, itemId: itemId)
    }

    public static func deletePlaylist(_ app: AppState, id: String) async throws {
        try await PatinaCore.Commands.deletePlaylist(app, id: id)
    }

    /// `after` nil moves to the top.
    public static func movePlaylistItem(_ app: AppState, id: String, itemId: Int, after: Int?) async throws {
        try await PatinaCore.Commands.movePlaylistItem(app, id: id, itemId: itemId, after: after)
    }

    public static func updatePlaylist(_ app: AppState, id: String, title: String, summary: String) async throws {
        try await PatinaCore.Commands.updatePlaylist(app, id: id, title: title, summary: summary)
    }

    public static func clearPlaylist(_ app: AppState, id: String) async throws {
        try await PatinaCore.Commands.clearPlaylist(app, id: id)
    }

    // MARK: Collections (merged: these forward to Commands+Library)

    /// Add an item to a collection, or to a new one called `title`.
    public static func addToCollection(
        _ app: AppState, ratingKey: String, collection: String? = nil, title: String? = nil
    ) async throws {
        try await PatinaCore.Commands.addToCollection(app, ratingKey: ratingKey, collection: collection, title: title)
    }

    public static func removeFromCollection(_ app: AppState, id: String, ratingKey: String) async throws {
        try await PatinaCore.Commands.removeFromCollection(app, id: id, ratingKey: ratingKey)
    }

    public static func deleteCollection(_ app: AppState, id: String) async throws {
        try await PatinaCore.Commands.deleteCollection(app, id: id)
    }

    /// `mode`: -1 library default, 0 hide collection, 1 hide items, 2 show both.
    /// `sort`: 0 release date, 1 title, 2 custom.
    public static func collectionSettings(_ app: AppState, id: String, mode: Int? = nil, sort: Int? = nil) async throws {
        try await PatinaCore.Commands.collectionSettings(app, id: id, mode: mode, sort: sort)
    }

    // MARK: Live TV and Seerr (merged: these forward to Commands)

    /// Whether the server has a DVR (the Live TV page).
    public static func livetvAvailable(_ app: AppState) async -> Bool {
        (try? await PatinaCore.Commands.livetvAvailable(app)) ?? false
    }

    /// Whether a Seerr instance is set up (`seerr_config` without contacting it).
    public static func seerrConfigured(_ app: AppState) -> Bool {
        PatinaCore.Commands.seerrConfig(app) != nil
    }
}
