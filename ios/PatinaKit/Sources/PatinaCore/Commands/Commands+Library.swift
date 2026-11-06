// Port of the "Library browsing", "Pick something to watch", "Search across servers" and
// "Collections" sections of desktop/commands/index.ts, plus saved library views (the desktop's
// webview edits `ui.saved_views` itself; here the UI goes through `Ops.Views`).

import Foundation
import PlexKit

extension Commands {
    // MARK: Library browsing: filters, jump bar, folders, related

    /// Filters for a section and media type (genre, year, resolution, unwatched, ...).
    public static func sectionFilters(_ app: AppState, id: String, kind: Int? = nil) async throws -> [Ops.Library.FilterView] {
        try await Ops.Library.sectionFilters(app, id, kind)
    }

    /// The jump bar: first characters of the titles, with how many start with each.
    public static func firstCharacters(_ app: AppState, id: String, kind: Int? = nil) async throws -> [Directory] {
        try await app.server().sectionFirstCharacters(id, kind: kind)
    }

    /// A section's folder tree: the root, or the folder at `key`.
    public static func sectionFolders(_ app: AppState, id: String, key: String? = nil) async throws -> Ops.Library.FolderView {
        try await Ops.Library.sectionFolders(app, id, key)
    }

    /// Extra hubs for a detail page, plus sonically similar music. `server` is the machine id
    /// of another server of the account.
    public static func itemHubs(_ app: AppState, server: String? = nil, ratingKey: String, kind: String) async throws -> [Hub] {
        if Servers.isCurrent(app, server) { return try await Ops.Library.itemHubs(app, ratingKey, kind) }
        var hubs = await Ops.Library.itemHubsOn(try await Servers.serverFor(app, server), ratingKey, kind)
        for i in hubs.indices { Servers.tag(app, &hubs[i].metadata, server) }
        return hubs
    }

    // MARK: Saved views

    /// The saved views of a library on the connected server, in the order saved.
    public static func savedViews(_ app: AppState, library: String) -> [Ops.Views.SavedView] {
        Ops.Views.list(app, library)
    }

    /// A new view of `library` (not saved yet), in the connected account and server's scope.
    public static func newSavedView(_ app: AppState, library: String, name: String) throws -> Ops.Views.SavedView {
        guard let scope = Ops.Views.scope(app) else { throw CoreError("Not connected to a server.") }
        return Ops.Views.newSavedView(scope, library, name)
    }

    /// Save a view; one of the same library and name is replaced.
    public static func saveView(_ app: AppState, view: Ops.Views.SavedView) throws {
        try Ops.Views.save(app, view)
    }

    public static func deleteView(_ app: AppState, id: String) throws {
        try Ops.Views.remove(app, id)
    }

    // MARK: Pick something to watch

    /// Libraries, genres and content ratings the form offers.
    public static func pickOptions(_ app: AppState) async throws -> Ops.Pick.Options {
        try await Ops.Pick.options(app)
    }

    /// A few things to watch. Pass the rating keys offered so far in `query.exclude` to reroll.
    public static func pickSomething(_ app: AppState, query: Ops.Pick.Query = Ops.Pick.Query()) async throws -> Ops.Pick.Result {
        try await Ops.Pick.pick(app, query)
    }

    // MARK: Search across servers

    /// Search every server of the account; `progress` gets the results so far once at the
    /// start and after each server answers (on the main actor).
    @discardableResult
    public static func searchAllServers(
        _ app: AppState, query: String, progress: (Ops.XSearch.Results) -> Void = { _ in }
    ) async -> Ops.XSearch.Results {
        await Ops.XSearch.searchAll(app, query, progress)
    }

    // MARK: Collections

    /// Add an item to a collection, or to a new one called `title`.
    public static func addToCollection(_ app: AppState, ratingKey: String, collection: String? = nil, title: String? = nil)
        async throws
    {
        try await Ops.Edit.addToCollection(app, ratingKey, collection, title)
    }

    public static func removeFromCollection(_ app: AppState, id: String, ratingKey: String) async throws {
        try await app.server().removeFromCollection(id, ratingKey: ratingKey)
    }

    /// Move an item after another (`after` is a rating key), or to the top.
    public static func moveCollectionItem(_ app: AppState, id: String, ratingKey: String, after: String? = nil) async throws {
        try await app.server().moveCollectionItem(id, ratingKey: ratingKey, after: after)
    }

    public static func deleteCollection(_ app: AppState, id: String) async throws {
        try await app.server().deleteCollection(id)
    }

    /// Collection display settings: `mode` (-1..2) and/or `sort` (0..2).
    public static func collectionSettings(_ app: AppState, id: String, mode: Int? = nil, sort: Int? = nil) async throws {
        let s = try app.server()
        if let mode { try await s.setCollectionMode(id, mode: mode) }
        if let sort { try await s.setCollectionSort(id, sort: sort) }
    }
}
