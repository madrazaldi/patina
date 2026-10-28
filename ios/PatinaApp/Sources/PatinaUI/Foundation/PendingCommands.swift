// The commands of desktop/commands/index.ts the screens need before their core modules are
// merged (no TypeScript twin of its own). Each function keeps the command's name and
// arguments, so the lead can swap a call `PendingCommands.x(...)` for `Commands.x(...)`
// once that lands. Where the handler is a direct server call it is done here, as
// index.ts does it; where it needs a module that isn't ported yet it is a no-op marked
// `TODO(merge)`, naming the command it waits for.

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

    // MARK: Downloads (the downloads agent)

    /// Queue an item (or every episode/track below it); returns how many were added.
    public static func download(_ app: AppState, ratingKey: String) async throws -> Int {
        // TODO(merge): Commands.download (downloads agent).
        throw CoreError("Downloads arrive in a later update.")
    }

    public static func retryDownload(_ app: AppState, id: String) async throws {
        // TODO(merge): Commands.retryDownload (downloads agent).
        throw CoreError("Downloads arrive in a later update.")
    }

    // MARK: Library browsing (the admin agent's Commands+Library)

    /// A filter of a section (`FilterView` of desktop/core/ops/library.ts).
    public struct FilterView: Sendable, Hashable {
        public var filter: String
        public var title: String
        /// `string`, `integer`, `tag` or `boolean`.
        public var kind: String
    }

    /// Filters for a section and media type (genre, year, resolution, unwatched, ...).
    public static func sectionFilters(_ app: AppState, id: String, kind: Int? = nil) async throws -> [FilterView] {
        // TODO(merge): Commands.sectionFilters (admin agent).
        let server = try app.server()
        func views(_ list: [Directory]) -> [FilterView] {
            list.compactMap { d in
                d.filter.map { FilterView(filter: $0, title: d.title ?? "", kind: d.filterType ?? "string") }
            }
        }
        if let meta = try await server.sectionMeta(id) {
            let found = meta.type.first { t in t.type.flatMap(Ops.Util.mediaTypeId) == kind } ?? meta.type.first
            if let found { return views(found.filter) }
        }
        return views(try await server.sectionFilters(id))
    }

    public static func firstCharacters(_ app: AppState, id: String, kind: Int? = nil) async throws -> [Directory] {
        // TODO(merge): Commands.firstCharacters (admin agent).
        try await app.server().sectionFirstCharacters(id, kind: kind)
    }

    /// A folder of a section (`FolderView`).
    public struct FolderView: Sendable, Equatable {
        public var title: String?
        public var directories: [Directory]
        public var items: [Metadata]
    }

    /// A section's folder tree: the root, or the folder at `key`.
    public static func sectionFolders(_ app: AppState, id: String, key: String? = nil) async throws -> FolderView {
        // TODO(merge): Commands.sectionFolders (admin agent).
        let mc = try await app.server().sectionFolders(id, parent: key)
        return FolderView(title: mc.title2 ?? mc.title1, directories: mc.directory, items: mc.metadata)
    }

    /// Extra hubs for a detail page, plus sonically similar music.
    public static func itemHubs(_ app: AppState, ratingKey: String, kind: String, server: String? = nil) async throws
        -> [Hub]
    {
        // TODO(merge): Commands.itemHubs (admin agent).
        let s = try await Servers.serverFor(app, server)
        var hubs = (try? await s.metadataHubs(ratingKey, count: 12)) ?? []
        var similar: (String, [Metadata])?
        if kind == "track" || kind == "album" {
            if let m = try? await s.nearest(ratingKey) { similar = ("Sonically similar", m) }
        } else if kind == "artist" {
            if let m = try? await s.similar(ratingKey) { similar = ("Similar artists", m) }
        }
        if let (title, items) = similar, !items.isEmpty {
            var hub = Hub()
            hub.title = title
            hub.hubIdentifier = "patina.\(kind).similar"
            hub.metadata = items.filter { $0.ratingKey != ratingKey }
            hubs.append(hub)
        }
        if !Servers.isCurrent(app, server) {
            for i in hubs.indices { Servers.tag(app, &hubs[i].metadata, server) }
        }
        return hubs
    }

    /// Scan a library's files (`section_action` with `scan`).
    public static func scanSection(_ app: AppState, id: String) async throws {
        // TODO(merge): Commands.sectionAction(id:, action: .scan) (admin agent).
        try await app.server().refreshSection(id, force: false)
    }

    // MARK: Watchlist (the admin agent's Commands+Watchlist)

    public static func watchlisted(_ app: AppState, discoverId: String) async throws -> Bool {
        // TODO(merge): Commands.watchlisted (admin agent).
        try await app.plexTv().userState(discoverId)?.watchlistedAt != nil
    }

    public static func setWatchlisted(_ app: AppState, discoverId: String, on: Bool) async throws {
        // TODO(merge): Commands.setWatchlisted (admin agent).
        let tv = try app.plexTv()
        if on { try await tv.addToWatchlist(discoverId) } else { try await tv.removeFromWatchlist(discoverId) }
    }

    // MARK: Playlists (the admin agent's Commands+Watchlist)

    /// A playlist and its entries (`PlaylistView` of desktop/core/ops/lists.ts).
    public struct PlaylistView: Sendable, Equatable {
        public var playlist: Metadata
        public var items: [Metadata]
        /// How many entries the server says the playlist has; more than `items.count` only
        /// when the safety limit cut loading short.
        public var total: Int
    }

    static let PLAYLIST_PAGE = 1000
    static let PLAYLIST_LIMIT = 100_000

    /// All playlists, or the non-smart ones of one kind (`video`, `audio`, `photo`).
    public static func playlists(_ app: AppState, kind: String? = nil) async throws -> [Metadata] {
        // TODO(merge): Commands.playlists (admin agent).
        try await app.server().playlists(kind, smart: kind != nil ? false : nil)
    }

    public static func playlist(_ app: AppState, id: String) async throws -> PlaylistView? {
        // TODO(merge): Commands.playlist (admin agent).
        let server = try app.server()
        guard let found = try await server.playlist(id) else { return nil }
        // `collectPages`: page until the reported total, a short page, or the limit.
        var items: [Metadata] = []
        var total: Int?
        while true {
            let mc = try await server.playlistItems(id, start: items.count, size: PLAYLIST_PAGE)
            if let t = mc.totalSize ?? mc.size { total = max(t, 0) }
            let got = mc.metadata.count
            items.append(contentsOf: mc.metadata)
            let done =
                got == 0 || (total.map { items.count >= $0 } ?? (got < PLAYLIST_PAGE)) || items.count >= PLAYLIST_LIMIT
            if done { break }
        }
        if items.count > PLAYLIST_LIMIT { items.removeLast(items.count - PLAYLIST_LIMIT) }
        return PlaylistView(playlist: found, items: items, total: total ?? items.count)
    }

    /// Add items to an existing playlist, or to a new one called `title` of `kind`.
    /// `ratingKey` may list several, comma-separated.
    public static func addToPlaylist(
        _ app: AppState, ratingKey: String, kind: String, playlist: String? = nil, title: String? = nil
    ) async throws {
        // TODO(merge): Commands.addToPlaylist (admin agent).
        let server = try app.server()
        let uri = server.itemUri([ratingKey])
        if let playlist {
            try await server.addToPlaylist(playlist, uri: uri)
        } else if let title {
            _ = try await server.createPlaylist(title, kind: kind, uri: uri, smart: false)
        } else {
            throw CoreError("No playlist given.")
        }
    }

    /// Remove an entry; announces itself with a toast that offers Undo.
    public static func removeFromPlaylist(_ app: AppState, id: String, itemId: Int) async throws {
        // TODO(merge): Commands.removeFromPlaylist (admin agent); Undo already has it.
        try await Undo.removeFromPlaylist(app, id, itemId)
    }

    public static func deletePlaylist(_ app: AppState, id: String) async throws {
        // TODO(merge): Commands.deletePlaylist (admin agent).
        try await app.server().deletePlaylist(id)
    }

    /// `after` nil moves to the top.
    public static func movePlaylistItem(_ app: AppState, id: String, itemId: Int, after: Int?) async throws {
        // TODO(merge): Commands.movePlaylistItem (admin agent).
        try await app.server().movePlaylistItem(id, playlistItemId: itemId, after: after)
    }

    public static func updatePlaylist(_ app: AppState, id: String, title: String, summary: String) async throws {
        // TODO(merge): Commands.updatePlaylist (admin agent).
        try await app.server().updatePlaylist(
            id, title: title.trimmingCharacters(in: .whitespacesAndNewlines),
            summary: summary.trimmingCharacters(in: .whitespacesAndNewlines))
    }

    public static func clearPlaylist(_ app: AppState, id: String) async throws {
        // TODO(merge): Commands.clearPlaylist (admin agent).
        try await app.server().clearPlaylist(id)
    }

    // MARK: Collections (the admin agent's Commands+Library)

    /// Add an item to a collection, or to a new one called `title`.
    public static func addToCollection(
        _ app: AppState, ratingKey: String, collection: String? = nil, title: String? = nil
    ) async throws {
        // TODO(merge): Commands.addToCollection (admin agent).
        let server = try app.server()
        let uri = server.itemUri([ratingKey])
        if let collection {
            try await server.addToCollection(collection, uri: uri)
        } else if let title {
            guard let item = try await server.metadata(ratingKey) else { throw CoreError("Item not found.") }
            guard let section = item.librarySectionID else { throw CoreError("The item is not in a library.") }
            let kind = Ops.Util.mediaTypeId(item.type ?? "") ?? 1
            _ = try await server.createCollection(String(section), title: title, kind: kind, uri: uri, smart: false)
        } else {
            throw CoreError("No collection given.")
        }
    }

    public static func removeFromCollection(_ app: AppState, id: String, ratingKey: String) async throws {
        // TODO(merge): Commands.removeFromCollection (admin agent).
        try await app.server().removeFromCollection(id, ratingKey: ratingKey)
    }

    public static func deleteCollection(_ app: AppState, id: String) async throws {
        // TODO(merge): Commands.deleteCollection (admin agent).
        try await app.server().deleteCollection(id)
    }

    /// `mode`: -1 library default, 0 hide collection, 1 hide items, 2 show both.
    /// `sort`: 0 release date, 1 title, 2 custom.
    public static func collectionSettings(_ app: AppState, id: String, mode: Int? = nil, sort: Int? = nil) async throws {
        // TODO(merge): Commands.collectionSettings (admin agent).
        let s = try app.server()
        if let mode { try await s.setCollectionMode(id, mode: mode) }
        if let sort { try await s.setCollectionSort(id, sort: sort) }
    }

    // MARK: Item actions

    /// Edit fields (locking them so refreshes keep the change) and tags.
    public static func editMetadata(
        _ app: AppState, ratingKey: String, fields: [(String, String)], tags: [Ops.Edit.TagEdit]
    ) async throws {
        // TODO(merge): Commands.editMetadata. It exists, but `Ops.Edit.editMetadata` still throws
        // "not ported", so this is desktop/core/ops/edit.ts done here.
        let server = try app.server()
        guard let item = try await server.metadata(ratingKey) else { throw CoreError("Item not found.") }
        guard let kind = Ops.Util.mediaTypeId(item.type ?? "") else { throw CoreError("This item cannot be edited.") }
        if !fields.isEmpty {
            try await server.editMetadata(ratingKey, sectionId: item.librarySectionID, kind: kind, fields: fields, lock: true)
        }
        for t in tags where !t.add.isEmpty || !t.remove.isEmpty {
            try await server.editTags(ratingKey, kind: kind, tag: t.tag, add: t.add, remove: t.remove)
        }
    }

    // MARK: Live TV and Seerr (the admin agent)

    /// Whether the server has a DVR (the Live TV page).
    public static func livetvAvailable(_ app: AppState) async -> Bool {
        // TODO(merge): Commands.livetvAvailable (admin agent).
        guard let dvrs = try? await app.server().dvrs() else { return false }
        return !(dvrs["MediaContainer"]?["Dvr"]?.arrayValue ?? []).isEmpty
    }

    /// Whether a Seerr instance is set up (`seerr_config` without contacting it).
    public static func seerrConfigured(_ app: AppState) -> Bool {
        // TODO(merge): Commands.seerrConfig (admin agent).
        !app.settings().seerr.url.isEmpty
    }
}
