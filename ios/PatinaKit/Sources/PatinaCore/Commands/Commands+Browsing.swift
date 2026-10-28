// Port of the "Browsing" section of desktop/commands/index.ts.

import Foundation
import PlexKit

extension Commands {
    public static func homeHubs(_ app: AppState) async throws -> [Hub] {
        try await app.server().homeHubs(20)
    }

    public static func sectionHubs(_ app: AppState, id: String) async throws -> [Hub] {
        try await app.server().sectionHubs(id, count: 18)
    }

    public static func sectionSorts(_ app: AppState, id: String) async throws -> [Directory] {
        try await app.server().sectionSorts(id)
    }

    public static func sectionFilterValues(_ app: AppState, id: String, filter: String, kind: Int? = nil) async throws
        -> [Directory]
    {
        try await app.server().sectionFilterValues(id, filter: filter, kind: kind)
    }

    /// One page of a library section; `filters` are `(key, value)` pairs such as `("genre", "12")`.
    public static func sectionItems(
        _ app: AppState, id: String, kind: Int? = nil, sort: String? = nil, unwatched: Bool = false,
        filters: [(String, String)] = [], start: Int, size: Int
    ) async throws -> MediaContainer {
        let query = LibraryQuery(
            kind: kind, sort: sort, unwatched: unwatched, filters: Params(filters.map { ($0.0, $0.1) }), start: start,
            size: size)
        return try await app.server().sectionItems(id, query: query)
    }

    public static func collections(_ app: AppState, id: String) async throws -> [Metadata] {
        try await app.server().collections(id)
    }

    /// Any server listing by key: "See all" for hubs, tag filters (genre, actor, ...).
    public static func listing(_ app: AppState, path: String, start: Int, size: Int) async throws -> MediaContainer {
        try await app.server().fetch(path, Params().set("X-Plex-Container-Start", start).set("X-Plex-Container-Size", size))
    }

    public static func search(_ app: AppState, query: String) async throws -> [Hub] {
        try await app.server().searchHubs(query, limit: 24)
    }

    /// `server` is the machine id of another server of the account (see `search_all_servers`).
    public static func metadata(_ app: AppState, server: String? = nil, ratingKey: String) async throws -> Metadata? {
        guard let found = try await Servers.serverFor(app, server).metadata(ratingKey, opts: .full()) else { return nil }
        var items = [found]
        Servers.tag(app, &items, server)
        return items[0]
    }

    /// Seasons, episodes, albums, tracks or collection items of an item.
    public static func children(_ app: AppState, server: String? = nil, ratingKey: String, collection: Bool = false)
        async throws -> [Metadata]
    {
        let s = try await Servers.serverFor(app, server)
        let mc = collection ? try await s.collectionItems(ratingKey) : try await s.children(ratingKey)
        var items = mc.metadata
        Servers.tag(app, &items, server)
        return items
    }

    /// Rating key of the local copy of a plex.tv item, if the server has one.
    public static func findByGuid(_ app: AppState, guid: String) async throws -> String? {
        try await app.server().findByGuid(guid).first?.ratingKey
    }
}
