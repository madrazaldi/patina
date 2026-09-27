import Foundation

// Plex models as the backend returns them: Plex's own JSON field names (see
// crates/plex/src/models and web/src/lib/types.ts). Only fields the UI reads
// are decoded; `Metadata` also keeps its whole JSON to hand back.

/// A coding key from any string, so decoders can read Plex's field names directly.
struct K: CodingKey, ExpressibleByStringLiteral {
    let stringValue: String
    var intValue: Int? { nil }
    init(stringValue: String) { self.stringValue = stringValue }
    init?(intValue: Int) { nil }
    init(stringLiteral value: String) { stringValue = value }
}

typealias Keyed = KeyedDecodingContainer<K>

struct Tag: Decodable, Hashable, Identifiable {
    var id: Int?
    var tag: String?
    var filter: String?
    var role: String?
    var thumb: String?
    var count: Int?

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        id = c.lenientInt("id")
        tag = c.lenientString("tag")
        filter = c.lenientString("filter")
        role = c.lenientString("role")
        thumb = c.lenientString("thumb")
        count = c.lenientInt("count")
    }
}

struct Stream: Decodable, Hashable, Identifiable {
    var streamID: Int?
    var id: Int { streamID ?? key?.hashValue ?? 0 }
    var streamType: Int?
    var selected: Bool
    var forced: Bool
    var codec: String?
    var index: Int?
    var language: String?
    var languageCode: String?
    var title: String?
    var displayTitle: String?
    var extendedDisplayTitle: String?
    var key: String?
    var channels: Int?
    var colorTrc: String?
    var doviPresent: Bool
    var hearingImpaired: Bool
    var format: String?
    var providerTitle: String?
    var score: Double?
    var perfectMatch: Bool
    var downloaded: Bool

    var isVideo: Bool { streamType == 1 }
    var isAudio: Bool { streamType == 2 }
    var isSubtitle: Bool { streamType == 3 }
    var label: String { extendedDisplayTitle ?? displayTitle ?? title ?? language ?? "Track \(index ?? 0)" }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        streamID = c.lenientInt("id")
        streamType = c.lenientInt("streamType")
        selected = c.lenientBool("selected")
        forced = c.lenientBool("forced")
        codec = c.lenientString("codec")
        index = c.lenientInt("index")
        language = c.lenientString("language")
        languageCode = c.lenientString("languageCode")
        title = c.lenientString("title")
        displayTitle = c.lenientString("displayTitle")
        extendedDisplayTitle = c.lenientString("extendedDisplayTitle")
        key = c.lenientString("key")
        channels = c.lenientInt("channels")
        colorTrc = c.lenientString("colorTrc")
        doviPresent = c.lenientBool("DOVIPresent")
        hearingImpaired = c.lenientBool("hearingImpaired")
        format = c.lenientString("format")
        providerTitle = c.lenientString("providerTitle")
        score = c.lenientDouble("score")
        perfectMatch = c.lenientBool("perfectMatch")
        downloaded = c.lenientBool("downloaded")
    }
}

struct Part: Decodable, Hashable {
    var id: Int?
    var key: String?
    var file: String?
    var size: Int64?
    var streams: [Stream]

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        id = c.lenientInt("id")
        key = c.lenientString("key")
        file = c.lenientString("file")
        size = c.lenientInt("size").map(Int64.init)
        streams = c.list("Stream")
    }
}

struct Media: Decodable, Hashable {
    var id: Int?
    var bitrate: Int?
    var width: Int?
    var height: Int?
    var audioChannels: Int?
    var audioCodec: String?
    var videoCodec: String?
    var videoResolution: String?
    var container: String?
    var title: String?
    var editionTitle: String?
    var parts: [Part]

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        id = c.lenientInt("id")
        bitrate = c.lenientInt("bitrate")
        width = c.lenientInt("width")
        height = c.lenientInt("height")
        audioChannels = c.lenientInt("audioChannels")
        audioCodec = c.lenientString("audioCodec")
        videoCodec = c.lenientString("videoCodec")
        videoResolution = c.lenientString("videoResolution")
        container = c.lenientString("container")
        title = c.lenientString("title")
        editionTitle = c.lenientString("editionTitle")
        parts = c.list("Part")
    }
}

struct UltraBlurColors: Decodable, Hashable {
    var topLeft: String?
    var topRight: String?
    var bottomRight: String?
    var bottomLeft: String?
}

struct Nested: Decodable, Hashable {
    var metadata: [Metadata]
    var hubs: [Hub]

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        metadata = c.list("Metadata")
        hubs = c.list("Hub")
    }
}

struct SessionUser: Decodable, Hashable {
    var id: String?
    var title: String?
    var thumb: String?

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        id = c.lenientString("id")
        title = c.lenientString("title")
        thumb = c.lenientString("thumb")
    }
}

struct SessionPlayer: Decodable, Hashable {
    var address: String?
    var product: String?
    var state: String?
    var title: String?
}

struct SessionInfo: Decodable, Hashable {
    var id: String?
    var bandwidth: Int?
    var location: String?

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        id = c.lenientString("id")
        bandwidth = c.lenientInt("bandwidth")
        location = c.lenientString("location")
    }
}

struct TranscodeSession: Decodable, Hashable {
    var key: String?
    var throttled: Bool
    var progress: Double?
    var speed: Double?
    var context: String?
    var sourceVideoCodec: String?
    var videoDecision: String?
    var audioDecision: String?
    var `protocol`: String?
    var videoCodec: String?
    var audioCodec: String?
    var transcodeHwRequested: Bool

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        key = c.lenientString("key")
        throttled = c.lenientBool("throttled")
        progress = c.lenientDouble("progress")
        speed = c.lenientDouble("speed")
        context = c.lenientString("context")
        sourceVideoCodec = c.lenientString("sourceVideoCodec")
        videoDecision = c.lenientString("videoDecision")
        audioDecision = c.lenientString("audioDecision")
        `protocol` = c.lenientString("protocol")
        videoCodec = c.lenientString("videoCodec")
        audioCodec = c.lenientString("audioCodec")
        transcodeHwRequested = c.lenientBool("transcodeHwRequested")
    }
}

struct PlexImage: Decodable, Hashable {
    var type: String?
    var url: String?
}

/// Any Plex item: movie, show, season, episode, artist, album, track, photo,
/// playlist, collection, clip, ...
struct Metadata: Codable, Hashable, Identifiable {
    /// The JSON this was decoded from; encoding writes it back unchanged.
    let raw: JSON

    var ratingKey: String?
    var key: String?
    var guid: String?
    var type: String?
    var title: String?
    var originalTitle: String?
    var studio: String?
    var contentRating: String?
    var summary: String?
    var tagline: String?
    var year: Int?
    var index: Int?
    var parentIndex: Int?
    var rating: Double?
    var audienceRating: Double?
    var userRating: Double?
    var ratingImage: String?
    var audienceRatingImage: String?
    var viewCount: Int?
    var viewOffset: Int?
    var lastViewedAt: Int?
    var addedAt: Int?
    var originallyAvailableAt: String?
    var duration: Int?
    var thumb: String?
    var art: String?
    var composite: String?
    var titleSort: String?
    var editionTitle: String?
    var parentRatingKey: String?
    var parentTitle: String?
    var parentThumb: String?
    var parentArt: String?
    var grandparentRatingKey: String?
    var grandparentTitle: String?
    var grandparentThumb: String?
    var grandparentArt: String?
    var leafCount: Int?
    var viewedLeafCount: Int?
    var childCount: Int?
    var librarySectionID: Int?
    var playlistType: String?
    var smart: Bool
    var playlistItemID: Int?
    var playQueueItemID: Int?
    var collectionMode: Int?
    var collectionSort: Int?
    var historyKey: String?
    var viewedAt: Int?
    var accountID: Int?
    var sessionKey: String?
    var user: SessionUser?
    var player: SessionPlayer?
    var session: SessionInfo?
    var transcodeSession: TranscodeSession?
    var media: [Media]
    var genres: [Tag]
    var directors: [Tag]
    var writers: [Tag]
    var countries: [Tag]
    var collections: [Tag]
    var labels: [Tag]
    var moods: [Tag]
    var styles: [Tag]
    var roles: [Tag]
    var images: [PlexImage]
    var ultraBlurColors: UltraBlurColors?
    var extras: Nested?
    var related: Nested?
    var onDeck: Nested?

    var id: String { ratingKey ?? key ?? guid ?? title ?? "" }

    init(from decoder: Decoder) throws {
        raw = try JSON(from: decoder)
        let c = try decoder.container(keyedBy: K.self)
        ratingKey = c.lenientString("ratingKey")
        key = c.lenientString("key")
        guid = c.lenientString("guid")
        type = c.lenientString("type")
        title = c.lenientString("title")
        originalTitle = c.lenientString("originalTitle")
        studio = c.lenientString("studio")
        contentRating = c.lenientString("contentRating")
        summary = c.lenientString("summary")
        tagline = c.lenientString("tagline")
        year = c.lenientInt("year")
        index = c.lenientInt("index")
        parentIndex = c.lenientInt("parentIndex")
        rating = c.lenientDouble("rating")
        audienceRating = c.lenientDouble("audienceRating")
        userRating = c.lenientDouble("userRating")
        ratingImage = c.lenientString("ratingImage")
        audienceRatingImage = c.lenientString("audienceRatingImage")
        viewCount = c.lenientInt("viewCount")
        viewOffset = c.lenientInt("viewOffset")
        lastViewedAt = c.lenientInt("lastViewedAt")
        addedAt = c.lenientInt("addedAt")
        originallyAvailableAt = c.lenientString("originallyAvailableAt")
        duration = c.lenientInt("duration")
        thumb = c.lenientString("thumb")
        art = c.lenientString("art")
        composite = c.lenientString("composite")
        titleSort = c.lenientString("titleSort")
        editionTitle = c.lenientString("editionTitle")
        parentRatingKey = c.lenientString("parentRatingKey")
        parentTitle = c.lenientString("parentTitle")
        parentThumb = c.lenientString("parentThumb")
        parentArt = c.lenientString("parentArt")
        grandparentRatingKey = c.lenientString("grandparentRatingKey")
        grandparentTitle = c.lenientString("grandparentTitle")
        grandparentThumb = c.lenientString("grandparentThumb")
        grandparentArt = c.lenientString("grandparentArt")
        leafCount = c.lenientInt("leafCount")
        viewedLeafCount = c.lenientInt("viewedLeafCount")
        childCount = c.lenientInt("childCount")
        librarySectionID = c.lenientInt("librarySectionID")
        playlistType = c.lenientString("playlistType")
        smart = c.lenientBool("smart")
        playlistItemID = c.lenientInt("playlistItemID")
        playQueueItemID = c.lenientInt("playQueueItemID")
        collectionMode = c.lenientInt("collectionMode")
        collectionSort = c.lenientInt("collectionSort")
        historyKey = c.lenientString("historyKey")
        viewedAt = c.lenientInt("viewedAt")
        accountID = c.lenientInt("accountID")
        sessionKey = c.lenientString("sessionKey")
        user = try? c.decodeIfPresent(SessionUser.self, forKey: "User")
        player = try? c.decodeIfPresent(SessionPlayer.self, forKey: "Player")
        session = try? c.decodeIfPresent(SessionInfo.self, forKey: "Session")
        transcodeSession = try? c.decodeIfPresent(TranscodeSession.self, forKey: "TranscodeSession")
        media = c.list("Media")
        genres = c.list("Genre")
        directors = c.list("Director")
        writers = c.list("Writer")
        countries = c.list("Country")
        collections = c.list("Collection")
        labels = c.list("Label")
        moods = c.list("Mood")
        styles = c.list("Style")
        roles = c.list("Role")
        images = c.list("Image")
        ultraBlurColors = try? c.decodeIfPresent(UltraBlurColors.self, forKey: "UltraBlurColors")
        extras = try? c.decodeIfPresent(Nested.self, forKey: "Extras")
        related = try? c.decodeIfPresent(Nested.self, forKey: "Related")
        onDeck = try? c.decodeIfPresent(Nested.self, forKey: "OnDeck")
    }

    func encode(to encoder: Encoder) throws {
        try raw.encode(to: encoder)
    }

    static func == (a: Metadata, b: Metadata) -> Bool { a.raw == b.raw }
    func hash(into h: inout Hasher) { h.combine(id) }
}

struct Location: Decodable, Hashable {
    var id: Int?
    var path: String?
}

struct Directory: Decodable, Hashable, Identifiable {
    var key: String?
    var title: String?
    var type: String?
    var thumb: String?
    var count: Int?
    var size: Int?
    var tag: String?
    var filter: String?
    var descKey: String?
    var agent: String?
    var scanner: String?
    var language: String?
    var refreshing: Bool
    var scannedAt: Int?
    var hidden: Bool
    var locations: [Location]

    var id: String { key ?? title ?? "" }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        key = c.lenientString("key")
        title = c.lenientString("title")
        type = c.lenientString("type")
        thumb = c.lenientString("thumb")
        count = c.lenientInt("count")
        size = c.lenientInt("size")
        tag = c.lenientString("tag")
        filter = c.lenientString("filter")
        descKey = c.lenientString("descKey")
        agent = c.lenientString("agent")
        scanner = c.lenientString("scanner")
        language = c.lenientString("language")
        refreshing = c.lenientBool("refreshing")
        scannedAt = c.lenientInt("scannedAt")
        hidden = c.lenientBool("hidden")
        locations = c.list("Location")
    }
}

struct Hub: Decodable, Hashable, Identifiable {
    var hubKey: String?
    var key: String?
    var title: String?
    var type: String?
    var hubIdentifier: String?
    var more: Bool
    var metadata: [Metadata]
    var directories: [Directory]

    var id: String { hubIdentifier ?? hubKey ?? key ?? title ?? "" }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        hubKey = c.lenientString("hubKey")
        key = c.lenientString("key")
        title = c.lenientString("title")
        type = c.lenientString("type")
        hubIdentifier = c.lenientString("hubIdentifier")
        more = c.lenientBool("more")
        metadata = c.list("Metadata")
        directories = c.list("Directory")
    }
}

struct MediaContainer: Decodable, Hashable {
    var size: Int?
    var totalSize: Int?
    var offset: Int?
    var metadata: [Metadata]
    var directories: [Directory]

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        size = c.lenientInt("size")
        totalSize = c.lenientInt("totalSize")
        offset = c.lenientInt("offset")
        metadata = c.list("Metadata")
        directories = c.list("Directory")
    }
}

struct HomeUser: Decodable, Hashable, Identifiable {
    var uuid: String?
    var title: String?
    var username: String?
    var thumb: String?
    var protected: Bool
    var id: String { uuid ?? title ?? "" }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        uuid = c.lenientString("uuid")
        title = c.lenientString("title")
        username = c.lenientString("username")
        thumb = c.lenientString("thumb")
        protected = c.lenientBool("protected")
    }
}

struct ButlerTask: Decodable, Hashable, Identifiable {
    var name: String?
    var title: String?
    var description: String?
    var enabled: Bool
    var id: String { name ?? title ?? "" }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        name = c.lenientString("name")
        title = c.lenientString("title")
        description = c.lenientString("description")
        enabled = c.lenientBool("enabled")
    }
}

struct MatchResult: Decodable, Hashable, Identifiable {
    var guid: String?
    var name: String?
    var score: Double?
    var year: Int?
    var thumb: String?
    var summary: String?
    var matched: Bool
    var id: String { guid ?? name ?? "" }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        guid = c.lenientString("guid")
        name = c.lenientString("name")
        score = c.lenientDouble("score")
        year = c.lenientInt("year")
        thumb = c.lenientString("thumb")
        summary = c.lenientString("summary")
        matched = c.lenientBool("matched")
    }
}

/// A poster/background choice: `key` is what `setArtwork` wants; `thumb` previews it.
struct ArtworkChoice: Decodable, Hashable, Identifiable {
    var key: String?
    var ratingKey: String?
    var thumb: String?
    var provider: String?
    var selected: Bool
    var id: String { key ?? ratingKey ?? thumb ?? "" }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        key = c.lenientString("key")
        ratingKey = c.lenientString("ratingKey")
        thumb = c.lenientString("thumb")
        provider = c.lenientString("provider")
        selected = c.lenientBool("selected")
    }
}
