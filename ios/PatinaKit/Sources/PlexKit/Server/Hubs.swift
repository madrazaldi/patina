// Port of desktop/plex/server/hubs.ts.
// Hubs and search (port of crates/plex/src/server/hubs.rs).

import Foundation

extension PlexServer {
    /// Home screen hubs across all libraries.
    public func homeHubs(_ count: Int) async throws -> [Hub] {
        let p = Params().set("count", count).set("includeLibraryPlaylists", 1).set("includeStations", 0)
        return try await get("/hubs", p).hub
    }

    /// Promoted hubs (what Plex apps show on the home screen), optionally per section.
    public func promotedHubs(_ sectionIds: [String], count: Int) async throws -> [Hub] {
        let ids = sectionIds.isEmpty ? nil : sectionIds.joined(separator: ",")
        return try await get("/hubs/promoted", Params().set("count", count).opt("contentDirectoryID", ids)).hub
    }

    /// "Continue Watching" (partially watched items plus next episodes).
    public func continueWatching(_ count: Int) async throws -> [Metadata] {
        try await get("/hubs/home/continueWatching", Params().set("count", count)).hub.flatMap(\.metadata)
    }

    public func homeOnDeck(_ count: Int) async throws -> [Metadata] {
        try await get("/hubs/home/onDeck", Params().set("count", count)).hub.flatMap(\.metadata)
    }

    /// Recently added across libraries; `kind` filters by media type.
    public func homeRecentlyAdded(_ kind: Int?, count: Int) async throws -> [Hub] {
        try await get("/hubs/home/recentlyAdded", Params().opt("type", kind).set("count", count)).hub
    }

    /// Hubs for one library section (recommended view).
    public func sectionHubs(_ sectionId: String, count: Int) async throws -> [Hub] {
        let p = Params().set("count", count).set("includeLibraryPlaylists", 1)
        return try await get("/hubs/sections/\(sectionId)", p).hub
    }

    /// Related hubs for an item.
    public func metadataHubs(_ ratingKey: String, count: Int) async throws -> [Hub] {
        try await get("/hubs/metadata/\(ratingKey)", Params().set("count", count)).hub
    }

    /// Items of a hub (via its `key`), paginated.
    public func hubItems(_ hubKey: String, start: Int, size: Int) async throws -> MediaContainer {
        try await get(hubKey, Params().set("X-Plex-Container-Start", start).set("X-Plex-Container-Size", size))
    }

    /// Search grouped into hubs (movies, shows, episodes, actors, genres, ...).
    public func searchHubs(_ query: String, limit: Int, sectionId: String? = nil) async throws -> [Hub] {
        let p = Params()
            .set("query", query)
            .set("limit", limit)
            .set("includeCollections", 1)
            .set("includeExternalMedia", 0)
            .opt("sectionId", sectionId)
        return try await get("/hubs/search", p).hub
    }

    /// Fuzzier search tuned for spoken queries.
    public func searchVoice(_ query: String, limit: Int, kind: Int? = nil) async throws -> [Hub] {
        try await get("/hubs/search/voice", Params().set("query", query).set("limit", limit).opt("type", kind)).hub
    }

    /// Legacy flat search.
    public func search(_ query: String, kind: Int? = nil) async throws -> [Metadata] {
        try await get("/search", Params().set("query", query).opt("type", kind)).metadata
    }
}
