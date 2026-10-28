// Port of desktop/plex/server/library.ts.
// Library sections, listings and maintenance (port of crates/plex/src/server/library.rs).

import Foundation

/// Query for browsing a library section. Every field is optional (Rust `..Default::default()`).
public struct LibraryQuery: Sendable, Equatable {
    /// Plex media type id (see `MediaType`); e.g. episodes of a show library.
    public var kind: Int?
    /// Sort key, e.g. `titleSort`, `addedAt:desc`, `year:desc`, `rating:desc`.
    public var sort: String?
    public var unwatched: Bool
    /// Free-form filters: `genre=12`, `year=1999`, `title=alien`, `resolution=4k`...
    public var filters: Params
    public var start: Int
    public var size: Int

    public init(
        kind: Int? = nil, sort: String? = nil, unwatched: Bool = false, filters: Params = Params(), start: Int = 0,
        size: Int = 0
    ) {
        self.kind = kind
        self.sort = sort
        self.unwatched = unwatched
        self.filters = filters
        self.start = start
        self.size = size
    }

    public static func page(_ start: Int, _ size: Int) -> LibraryQuery {
        LibraryQuery(start: start, size: size)
    }

    public func params() -> Params {
        var p = Params()
            .opt("type", kind)
            .opt("sort", sort)
            .flag("unwatched", unwatched)
            .extend(filters)
            .set("includeGuids", 1)
            .set("X-Plex-Container-Start", start)
        if size > 0 { p.push("X-Plex-Container-Size", size) }
        return p
    }
}

extension PlexServer {
    /// All library sections.
    public func sections() async throws -> [Directory] {
        try await get("/library/sections", Params()).directory
    }

    /// Section details, including the browse sub-directories (unwatched, genre, year, ...).
    public func section(_ id: String) async throws -> MediaContainer {
        try await get("/library/sections/\(id)", Params().set("includeDetails", 1))
    }

    /// Browse a section's items.
    public func sectionItems(_ id: String, query: LibraryQuery) async throws -> MediaContainer {
        try await get("/library/sections/\(id)/all", query.params())
    }

    /// Any section sub-listing: `unwatched`, `newest`, `recentlyAdded`, `onDeck`,
    /// `recentlyViewed`, `genre`, `year`, `decade`, `director`, `actor`,
    /// `contentRating`, `resolution`, `firstCharacter`, `collection`, `folder`, ...
    public func sectionListing(_ id: String, listing: String, params: Params = Params()) async throws
        -> MediaContainer
    {
        try await get("/library/sections/\(id)/\(listing)", params)
    }

    public func sectionFilters(_ id: String) async throws -> [Directory] {
        try await sectionListing(id, listing: "filters").directory
    }

    public func sectionSorts(_ id: String) async throws -> [Directory] {
        try await sectionListing(id, listing: "sorts").directory
    }

    /// Values of a filter (e.g. all genres) as `Directory` entries whose `key` is the filter value.
    public func sectionFilterValues(_ id: String, filter: String, kind: Int? = nil) async throws -> [Directory] {
        try await sectionListing(id, listing: filter, params: Params().opt("type", kind)).directory
    }

    /// Types, filters, sorts and editable fields for a section (`includeMeta=1`).
    public func sectionMeta(_ id: String) async throws -> LibraryMeta? {
        let params = Params().set("includeMeta", 1).set("X-Plex-Container-Size", 0)
        return try await get("/library/sections/\(id)/all", params).meta
    }

    public func sectionFirstCharacters(_ id: String, kind: Int? = nil) async throws -> [Directory] {
        try await sectionListing(id, listing: "firstCharacter", params: Params().opt("type", kind)).directory
    }

    public func sectionOnDeck(_ id: String) async throws -> [Metadata] {
        try await sectionListing(id, listing: "onDeck").metadata
    }

    public func sectionRecentlyAdded(_ id: String, size: Int) async throws -> [Metadata] {
        let p = Params().set("X-Plex-Container-Start", 0).set("X-Plex-Container-Size", size)
        return try await sectionListing(id, listing: "recentlyAdded", params: p).metadata
    }

    /// Browse a section by folder. `parent` is a key from a previous call.
    public func sectionFolders(_ id: String, parent: String? = nil) async throws -> MediaContainer {
        if let parent { return try await get(parent, Params()) }
        return try await sectionListing(id, listing: "folder")
    }

    // ----- Library-wide ---------------------------------------------------

    public func onDeck() async throws -> [Metadata] {
        try await get("/library/onDeck", Params()).metadata
    }

    public func recentlyAdded(_ size: Int) async throws -> [Metadata] {
        let p = Params().set("X-Plex-Container-Start", 0).set("X-Plex-Container-Size", size)
        return try await get("/library/recentlyAdded", p).metadata
    }

    /// Search all libraries with arbitrary filters (`/library/all`).
    public func libraryAll(_ params: Params) async throws -> MediaContainer {
        try await get("/library/all", params)
    }

    /// Find the local copy of a plex:// (or imdb://, tmdb://, tvdb://) guid.
    public func findByGuid(_ guid: String) async throws -> [Metadata] {
        try await libraryAll(Params().set("guid", guid)).metadata
    }

    /// Every tag of a given kind (`genre`, `actor`, ...), optionally for one type.
    public func tags(_ tagType: Int) async throws -> [Directory] {
        try await get("/library/tags", Params().set("type", tagType)).directory
    }

    // ----- Maintenance ----------------------------------------------------

    /// Scan a section for new files. `force` re-reads metadata; `path` limits to a subfolder.
    public func refreshSection(_ id: String, force: Bool, path: String? = nil) async throws {
        try await empty("GET", "/library/sections/\(id)/refresh", Params().flag("force", force).opt("path", path))
    }

    public func cancelRefreshSection(_ id: String) async throws {
        try await delete("/library/sections/\(id)/refresh")
    }

    public func refreshAllSections(_ force: Bool) async throws {
        try await empty("GET", "/library/sections/all/refresh", Params().flag("force", force))
    }

    public func emptyTrash(_ id: String) async throws {
        try await put("/library/sections/\(id)/emptyTrash")
    }

    public func analyzeSection(_ id: String) async throws {
        try await put("/library/sections/\(id)/analyze")
    }

    public func cleanBundles() async throws {
        try await put("/library/clean/bundles")
    }

    public func optimizeDatabase() async throws {
        try await put("/library/optimize", Params().set("async", 1))
    }

    /// Create a library. Params: `name`, `type` (movie/show/artist/photo), `agent`,
    /// `scanner`, `language`, `location` (repeatable), plus `prefs[...]`.
    public func createSection(_ params: Params) async throws -> MediaContainer {
        try await container("POST", "/library/sections", params)
    }

    public func editSection(_ id: String, params: Params) async throws {
        try await put("/library/sections/\(id)", params)
    }

    public func deleteSection(_ id: String) async throws {
        try await delete("/library/sections/\(id)")
    }

    /// Per-section preferences.
    public func sectionPrefs(_ id: String) async throws -> [Setting] {
        try await get("/library/sections/\(id)/prefs").setting
    }

    public func setSectionPrefs(_ id: String, params: Params) async throws {
        try await put("/library/sections/\(id)/prefs", params)
    }

    /// Available metadata agents for a media type.
    public func agents(_ mediaType: Int? = nil) async throws -> MediaContainer {
        try await get("/system/agents", Params().opt("mediaType", mediaType))
    }

    /// Available scanners for a media type.
    public func scanners(_ mediaType: Int) async throws -> MediaContainer {
        try await get("/system/scanners", Params().set("type", mediaType))
    }
}
