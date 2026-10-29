// Port of desktop/plex/server/metadata.ts.
// Item metadata, watch state, editing, matching, artwork and streams
// (port of crates/plex/src/server/metadata.rs).

import Foundation

/// Optional sections to include when fetching an item. Every field defaults to `false`.
public struct MetadataOptions: Sendable, Hashable {
    public var children: Bool
    public var extras: Bool
    public var related: Bool
    public var markers: Bool
    public var chapters: Bool
    public var onDeck: Bool
    public var reviews: Bool
    public var concerts: Bool
    /// Radio stations of an artist (or album / track).
    public var stations: Bool

    public init(
        children: Bool = false, extras: Bool = false, related: Bool = false, markers: Bool = false,
        chapters: Bool = false, onDeck: Bool = false, reviews: Bool = false, concerts: Bool = false,
        stations: Bool = false
    ) {
        self.children = children
        self.extras = extras
        self.related = related
        self.markers = markers
        self.chapters = chapters
        self.onDeck = onDeck
        self.reviews = reviews
        self.concerts = concerts
        self.stations = stations
    }

    /// Everything a detail page needs.
    public static func full() -> MetadataOptions {
        MetadataOptions(
            children: false, extras: true, related: true, markers: true, chapters: true, onDeck: true, reviews: true,
            concerts: false, stations: false)
    }

    /// What the player needs: markers and chapters.
    public static func playback() -> MetadataOptions {
        MetadataOptions(markers: true, chapters: true)
    }

    public func params() -> Params {
        Params()
            .flag("includeChildren", children)
            .flag("includeExtras", extras)
            .flag("includeRelated", related)
            .flag("includeMarkers", markers)
            .flag("includeChapters", chapters)
            .flag("includeOnDeck", onDeck)
            .flag("includeReviews", reviews)
            .flag("includeConcerts", concerts)
            .flag("includeStations", stations)
            .set("includeGuids", 1)
            .set("asyncAugmentMetadata", 0)
    }
}

/// `futures::stream::iter(items).map(f).buffered(limit).try_collect()`: at most `limit`
/// calls in flight, results in input order, the first error (in order) wins.
func buffered<T: Sendable, R: Sendable>(_ items: [T], _ limit: Int, _ f: @escaping @Sendable (T) async throws -> R)
    async throws -> [R]
{
    var results = [Result<R, Error>?](repeating: nil, count: items.count)
    await withTaskGroup(of: (Int, Result<R, Error>).self) { group in
        var next = 0
        func launch() {
            let i = next
            let item = items[i]
            next += 1
            group.addTask {
                do { return (i, .success(try await f(item))) } catch { return (i, .failure(error)) }
            }
        }
        while next < items.count && next < limit { launch() }
        while let (i, r) = await group.next() {
            results[i] = r
            if next < items.count { launch() }
        }
    }
    return try results.map { try $0!.get() }
}

private func libKey(_ ratingKey: String) -> Params {
    Params().set("key", ratingKey).set("identifier", "com.plexapp.plugins.library")
}

extension PlexServer {
    public func metadata(_ ratingKey: String, opts: MetadataOptions = MetadataOptions()) async throws -> Metadata? {
        try await get("/library/metadata/\(ratingKey)", opts.params()).metadata.first
    }

    /// Several items in one request.
    public func metadataMany(_ ratingKeys: [String]) async throws -> [Metadata] {
        if ratingKeys.isEmpty { return [] }
        return try await get("/library/metadata/\(ratingKeys.joined(separator: ","))").metadata
    }

    /// Direct children: seasons of a show, episodes of a season, tracks of an album.
    public func children(_ ratingKey: String) async throws -> MediaContainer {
        try await get("/library/metadata/\(ratingKey)/children", Params().set("includeGuids", 1))
    }

    /// All leaves: every episode of a show, every track of an artist.
    public func allLeaves(_ ratingKey: String) async throws -> MediaContainer {
        try await get("/library/metadata/\(ratingKey)/allLeaves")
    }

    /// Every leaf under an item, with full stream details. `allLeaves` leaves
    /// streams out, so the items are fetched again in batches; any the batch
    /// endpoint still returns without streams are fetched one by one.
    public func allLeavesWithStreams(_ ratingKey: String) async throws -> [Metadata] {
        let keys = try await allLeaves(ratingKey).metadata.map(Metadata.ratingKey)
        let chunks = stride(from: 0, to: keys.count, by: 40).map { Array(keys[$0..<min($0 + 40, keys.count)]) }
        let batches = try await buffered(chunks, 4) { chunk in try await self.metadataMany(chunk) }
        let items = try await buffered(batches.flatMap { $0 }, 8) { item in
            let missing = item.media.flatMap(\.part).contains { $0.stream.isEmpty }
            if !missing { return item }
            return try await self.metadata(Metadata.ratingKey(item)) ?? item
        }
        // Keep allLeaves order even if the batch endpoint reorders.
        let position = { (m: Metadata) in keys.firstIndex(of: Metadata.ratingKey(m)) ?? -1 }  // -1 (None) sorts first, like Option
        return items.enumerated()
            .sorted { (position($0.element), $0.offset) < (position($1.element), $1.offset) }
            .map(\.element)
    }

    public func grandchildren(_ ratingKey: String) async throws -> MediaContainer {
        try await get("/library/metadata/\(ratingKey)/grandchildren")
    }

    /// "More like this" and related hubs.
    public func related(_ ratingKey: String) async throws -> [Hub] {
        try await get("/library/metadata/\(ratingKey)/related").hub
    }

    public func similar(_ ratingKey: String) async throws -> [Metadata] {
        try await get("/library/metadata/\(ratingKey)/similar").metadata
    }

    /// Trailers, featurettes, deleted scenes, ...
    public func extras(_ ratingKey: String) async throws -> [Metadata] {
        try await get("/library/metadata/\(ratingKey)/extras").metadata
    }

    /// Items surrounding an item in its parent (e.g. neighbouring episodes).
    public func nearest(_ ratingKey: String) async throws -> [Metadata] {
        try await get("/library/metadata/\(ratingKey)/nearest").metadata
    }

    // ----- Watch state & ratings -----------------------------------------

    public func scrobble(_ ratingKey: String) async throws {
        try await empty("GET", "/:/scrobble", libKey(ratingKey))
    }

    public func unscrobble(_ ratingKey: String) async throws {
        try await empty("GET", "/:/unscrobble", libKey(ratingKey))
    }

    /// Rate on a 0-10 scale; `nil` clears the rating.
    public func rate(_ ratingKey: String, rating: Double? = nil) async throws {
        try await put("/:/rate", libKey(ratingKey).set("rating", rating ?? -1))
    }

    /// Hide an item from Continue Watching / On Deck until it is played again.
    public func removeFromContinueWatching(_ ratingKey: String) async throws {
        try await put("/actions/removeFromContinueWatching", Params().set("ratingKey", ratingKey))
    }

    /// The first radio station of an artist, album or track (sonic analysis required).
    public func station(_ ratingKey: String) async throws -> Metadata? {
        try await metadata(ratingKey, opts: MetadataOptions(stations: true))?.stations.first
    }

    /// Store a resume position without a playback session.
    public func setProgress(_ ratingKey: String, timeMs: Int, state: String) async throws {
        try await empty("GET", "/:/progress", libKey(ratingKey).set("time", timeMs).set("state", state))
    }

    // ----- Maintenance & editing -----------------------------------------

    public func refreshMetadata(_ ratingKey: String, force: Bool) async throws {
        try await put("/library/metadata/\(ratingKey)/refresh", Params().flag("force", force))
    }

    public func analyzeMetadata(_ ratingKey: String) async throws {
        try await put("/library/metadata/\(ratingKey)/analyze")
    }

    /// Delete an item and its files from disk (requires the server setting to allow it).
    public func deleteMetadata(_ ratingKey: String) async throws {
        try await delete("/library/metadata/\(ratingKey)")
    }

    /// Edit fields: pairs like `("title", "New")`, `("summary", "...")`.
    /// Each field is written as `<field>.value` and optionally locked.
    /// Uses the section endpoint when `sectionId` is known (works on all server versions).
    public func editMetadata(
        _ ratingKey: String, sectionId: Int?, kind: Int, fields: [(String, String)], lock: Bool
    ) async throws {
        var p = Params().set("type", kind).set("id", ratingKey)
        for (field, value) in fields {
            p.push("\(field).value", value)
            if lock { p.push("\(field).locked", 1) }
        }
        if let sectionId { return try await put("/library/sections/\(sectionId)/all", p) }
        try await put("/library/metadata/\(ratingKey)", p)
    }

    /// Lock or unlock fields (`title`, `summary`, `label`, ...) without changing
    /// their values; locked fields are kept when agents refresh the item.
    public func lockFields(_ ratingKey: String, sectionId: Int?, kind: Int, fields: [(String, Bool)]) async throws {
        var p = Params().set("type", kind).set("id", ratingKey)
        for (field, locked) in fields { p.push("\(field).locked", locked ? 1 : 0) }
        if let sectionId { return try await put("/library/sections/\(sectionId)/all", p) }
        try await put("/library/metadata/\(ratingKey)", p)
    }

    /// Add or remove tags (genre, collection, label, ...).
    public func editTags(_ ratingKey: String, kind: Int, tag: String, add: [String], remove: [String]) async throws {
        var p = Params().set("type", kind).set("id", ratingKey)
        for (i, v) in add.enumerated() { p.push("\(tag)[\(i)].tag.tag", v) }
        if !remove.isEmpty { p.push("\(tag)[].tag.tag-", remove.joined(separator: ",")) }
        try await put("/library/metadata/\(ratingKey)", p)
    }

    /// Item-level preferences (e.g. show episode ordering, collection mode).
    public func setMetadataPrefs(_ ratingKey: String, params: Params) async throws {
        try await put("/library/metadata/\(ratingKey)/prefs", params)
    }

    // ----- Matching --------------------------------------------------------

    public func matches(_ ratingKey: String, title: String? = nil, year: Int? = nil, agent: String? = nil)
        async throws -> [MatchResult]
    {
        let p = Params().set("manual", 1).opt("title", title).opt("year", year).opt("agent", agent)
        return try await get("/library/metadata/\(ratingKey)/matches", p).searchResult
    }

    public func applyMatch(_ ratingKey: String, guid: String, name: String? = nil) async throws {
        try await put("/library/metadata/\(ratingKey)/match", Params().set("guid", guid).opt("name", name))
    }

    public func unmatch(_ ratingKey: String) async throws {
        try await put("/library/metadata/\(ratingKey)/unmatch")
    }

    // ----- Artwork ---------------------------------------------------------

    public func posters(_ ratingKey: String) async throws -> [Metadata] {
        try await get("/library/metadata/\(ratingKey)/posters").metadata
    }

    public func arts(_ ratingKey: String) async throws -> [Metadata] {
        try await get("/library/metadata/\(ratingKey)/arts").metadata
    }

    /// Select a poster by its URL/key from `posters`.
    public func setPoster(_ ratingKey: String, url: String) async throws {
        try await put("/library/metadata/\(ratingKey)/poster", Params().set("url", url))
    }

    public func setArt(_ ratingKey: String, url: String) async throws {
        try await put("/library/metadata/\(ratingKey)/art", Params().set("url", url))
    }

    // ----- Streams & subtitles ---------------------------------------------

    /// Persist the preferred audio / subtitle stream for a part.
    /// `subtitle = 0` disables subtitles.
    public func selectStreams(_ partId: Int, audio: Int?, subtitle: Int?, allParts: Bool) async throws {
        let p = Params()
            .opt("audioStreamID", audio)
            .opt("subtitleStreamID", subtitle)
            .flag("allParts", allParts)
        try await put("/library/parts/\(partId)", p)
    }

    /// Search online subtitle providers for an item.
    public func searchSubtitles(_ ratingKey: String, language: String, hearingImpaired: Bool, forced: Bool)
        async throws -> [MediaStream]
    {
        let p = Params()
            .set("language", language)
            .flag("hearingImpaired", hearingImpaired)
            .flag("forced", forced)
        let v = try await raw("GET", "/library/metadata/\(ratingKey)/subtitles", p)
        // `from_value(list).unwrap_or_default()`: anything that does not decode gives an empty list.
        guard case .array(let list)? = De.pointer(v, "/MediaContainer/Stream") else { return [] }
        return (try? list.map(MediaStream.init(json:))) ?? []
    }

    /// Download an online subtitle found with `searchSubtitles`.
    public func downloadSubtitle(_ ratingKey: String, key: String) async throws {
        try await put("/library/metadata/\(ratingKey)/subtitles", Params().set("key", key))
    }

    public func deleteStream(_ streamId: Int) async throws {
        try await delete("/library/streams/\(streamId)")
    }

    /// Direct URL for a part (the original file), with the token embedded.
    public func partUrl(_ partKey: String, download: Bool) throws -> URL {
        try authedUrl(partKey, Params().flag("download", download))
    }
}
