// Port of desktop/plex/models/metadata.ts.
// Metadata items and their nested blocks (port of crates/plex/src/models/metadata.rs).

import Foundation

/// A metadata item: movie, show, season, episode, artist, album, track, photo,
/// clip, playlist, collection, history entry, or live session.
public struct Metadata: PlexModel {
    public var ratingKey: String? = nil
    public var key: String? = nil
    public var guid: String? = nil
    public var type: String? = nil
    public var subtype: String? = nil
    public var title: String? = nil
    public var titleSort: String? = nil
    public var originalTitle: String? = nil
    public var editionTitle: String? = nil
    public var studio: String? = nil
    public var contentRating: String? = nil
    public var summary: String? = nil
    public var tagline: String? = nil
    public var year: Int? = nil
    public var index: Int? = nil
    public var parentIndex: Int? = nil
    public var rating: Double? = nil
    public var audienceRating: Double? = nil
    public var userRating: Double? = nil
    public var ratingImage: String? = nil
    public var audienceRatingImage: String? = nil
    public var viewCount: Int? = nil
    public var skipCount: Int? = nil
    public var viewOffset: Int? = nil
    public var lastViewedAt: Int? = nil
    public var lastRatedAt: Int? = nil
    public var addedAt: Int? = nil
    public var updatedAt: Int? = nil
    public var originallyAvailableAt: String? = nil
    public var duration: Int? = nil
    public var thumb: String? = nil
    public var art: String? = nil
    public var banner: String? = nil
    public var theme: String? = nil
    public var composite: String? = nil
    public var chapterSource: String? = nil
    public var primaryExtraKey: String? = nil
    public var extraType: Int? = nil
    public var parentRatingKey: String? = nil
    public var parentKey: String? = nil
    public var parentGuid: String? = nil
    public var parentTitle: String? = nil
    public var parentThumb: String? = nil
    public var parentArt: String? = nil
    public var parentTheme: String? = nil
    public var parentYear: Int? = nil
    public var parentStudio: String? = nil
    public var grandparentRatingKey: String? = nil
    public var grandparentKey: String? = nil
    public var grandparentGuid: String? = nil
    public var grandparentTitle: String? = nil
    public var grandparentThumb: String? = nil
    public var grandparentArt: String? = nil
    public var grandparentTheme: String? = nil
    public var leafCount: Int? = nil
    public var viewedLeafCount: Int? = nil
    public var childCount: Int? = nil
    public var seasonCount: Int? = nil
    public var albumCount: Int? = nil
    public var librarySectionID: Int? = nil
    public var librarySectionTitle: String? = nil
    public var librarySectionKey: String? = nil
    public var playlistType: String? = nil
    public var smart: Bool = false
    public var playlistItemID: Int? = nil
    public var playQueueItemID: Int? = nil
    public var minYear: String? = nil
    public var maxYear: String? = nil
    public var collectionMode: Int? = nil
    public var historyKey: String? = nil
    public var viewedAt: Int? = nil
    public var accountID: Int? = nil
    public var deviceID: Int? = nil
    public var watchlistedAt: Int? = nil
    public var sessionKey: String? = nil
    public var user: SessionUser? = nil
    public var player: Player? = nil
    public var session: SessionInfo? = nil
    public var transcodeSession: TranscodeSession? = nil
    public var media: [Media] = []
    public var genre: [Tag] = []
    public var director: [Tag] = []
    public var writer: [Tag] = []
    public var producer: [Tag] = []
    public var country: [Tag] = []
    public var collection: [Tag] = []
    public var label: [Tag] = []
    public var mood: [Tag] = []
    public var style: [Tag] = []
    public var similar: [Tag] = []
    public var role: [Tag] = []
    public var guids: [GuidRef] = []
    public var marker: [Marker] = []
    public var chapter: [Chapter] = []
    public var image: [PlexImage] = []
    public var ratings: [RatingSource] = []
    public var review: [Review] = []
    public var ultraBlurColors: UltraBlurColors? = nil
    public var extras: Nested? = nil
    public var related: Nested? = nil
    public var onDeck: Nested? = nil
    public var children: Nested? = nil
    /// Radio stations (`includeStations=1`); each is a playlist-like item whose key feeds a play queue.
    public var stations: [Metadata] = []
    /// Attributes Plex sent that are not modelled (serde's flattened `extra` map).
    public var extra: [String: JSONValue] = [:]

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        ratingKey: String? = nil,
        key: String? = nil,
        guid: String? = nil,
        type: String? = nil,
        subtype: String? = nil,
        title: String? = nil,
        titleSort: String? = nil,
        originalTitle: String? = nil,
        editionTitle: String? = nil,
        studio: String? = nil,
        contentRating: String? = nil,
        summary: String? = nil,
        tagline: String? = nil,
        year: Int? = nil,
        index: Int? = nil,
        parentIndex: Int? = nil,
        rating: Double? = nil,
        audienceRating: Double? = nil,
        userRating: Double? = nil,
        ratingImage: String? = nil,
        audienceRatingImage: String? = nil,
        viewCount: Int? = nil,
        skipCount: Int? = nil,
        viewOffset: Int? = nil,
        lastViewedAt: Int? = nil,
        lastRatedAt: Int? = nil,
        addedAt: Int? = nil,
        updatedAt: Int? = nil,
        originallyAvailableAt: String? = nil,
        duration: Int? = nil,
        thumb: String? = nil,
        art: String? = nil,
        banner: String? = nil,
        theme: String? = nil,
        composite: String? = nil,
        chapterSource: String? = nil,
        primaryExtraKey: String? = nil,
        extraType: Int? = nil,
        parentRatingKey: String? = nil,
        parentKey: String? = nil,
        parentGuid: String? = nil,
        parentTitle: String? = nil,
        parentThumb: String? = nil,
        parentArt: String? = nil,
        parentTheme: String? = nil,
        parentYear: Int? = nil,
        parentStudio: String? = nil,
        grandparentRatingKey: String? = nil,
        grandparentKey: String? = nil,
        grandparentGuid: String? = nil,
        grandparentTitle: String? = nil,
        grandparentThumb: String? = nil,
        grandparentArt: String? = nil,
        grandparentTheme: String? = nil,
        leafCount: Int? = nil,
        viewedLeafCount: Int? = nil,
        childCount: Int? = nil,
        seasonCount: Int? = nil,
        albumCount: Int? = nil,
        librarySectionID: Int? = nil,
        librarySectionTitle: String? = nil,
        librarySectionKey: String? = nil,
        playlistType: String? = nil,
        smart: Bool = false,
        playlistItemID: Int? = nil,
        playQueueItemID: Int? = nil,
        minYear: String? = nil,
        maxYear: String? = nil,
        collectionMode: Int? = nil,
        historyKey: String? = nil,
        viewedAt: Int? = nil,
        accountID: Int? = nil,
        deviceID: Int? = nil,
        watchlistedAt: Int? = nil,
        sessionKey: String? = nil,
        user: SessionUser? = nil,
        player: Player? = nil,
        session: SessionInfo? = nil,
        transcodeSession: TranscodeSession? = nil,
        media: [Media] = [],
        genre: [Tag] = [],
        director: [Tag] = [],
        writer: [Tag] = [],
        producer: [Tag] = [],
        country: [Tag] = [],
        collection: [Tag] = [],
        label: [Tag] = [],
        mood: [Tag] = [],
        style: [Tag] = [],
        similar: [Tag] = [],
        role: [Tag] = [],
        guids: [GuidRef] = [],
        marker: [Marker] = [],
        chapter: [Chapter] = [],
        image: [PlexImage] = [],
        ratings: [RatingSource] = [],
        review: [Review] = [],
        ultraBlurColors: UltraBlurColors? = nil,
        extras: Nested? = nil,
        related: Nested? = nil,
        onDeck: Nested? = nil,
        children: Nested? = nil,
        stations: [Metadata] = [],
        extra: [String: JSONValue] = [:]
    ) {
        self.ratingKey = ratingKey
        self.key = key
        self.guid = guid
        self.type = type
        self.subtype = subtype
        self.title = title
        self.titleSort = titleSort
        self.originalTitle = originalTitle
        self.editionTitle = editionTitle
        self.studio = studio
        self.contentRating = contentRating
        self.summary = summary
        self.tagline = tagline
        self.year = year
        self.index = index
        self.parentIndex = parentIndex
        self.rating = rating
        self.audienceRating = audienceRating
        self.userRating = userRating
        self.ratingImage = ratingImage
        self.audienceRatingImage = audienceRatingImage
        self.viewCount = viewCount
        self.skipCount = skipCount
        self.viewOffset = viewOffset
        self.lastViewedAt = lastViewedAt
        self.lastRatedAt = lastRatedAt
        self.addedAt = addedAt
        self.updatedAt = updatedAt
        self.originallyAvailableAt = originallyAvailableAt
        self.duration = duration
        self.thumb = thumb
        self.art = art
        self.banner = banner
        self.theme = theme
        self.composite = composite
        self.chapterSource = chapterSource
        self.primaryExtraKey = primaryExtraKey
        self.extraType = extraType
        self.parentRatingKey = parentRatingKey
        self.parentKey = parentKey
        self.parentGuid = parentGuid
        self.parentTitle = parentTitle
        self.parentThumb = parentThumb
        self.parentArt = parentArt
        self.parentTheme = parentTheme
        self.parentYear = parentYear
        self.parentStudio = parentStudio
        self.grandparentRatingKey = grandparentRatingKey
        self.grandparentKey = grandparentKey
        self.grandparentGuid = grandparentGuid
        self.grandparentTitle = grandparentTitle
        self.grandparentThumb = grandparentThumb
        self.grandparentArt = grandparentArt
        self.grandparentTheme = grandparentTheme
        self.leafCount = leafCount
        self.viewedLeafCount = viewedLeafCount
        self.childCount = childCount
        self.seasonCount = seasonCount
        self.albumCount = albumCount
        self.librarySectionID = librarySectionID
        self.librarySectionTitle = librarySectionTitle
        self.librarySectionKey = librarySectionKey
        self.playlistType = playlistType
        self.smart = smart
        self.playlistItemID = playlistItemID
        self.playQueueItemID = playQueueItemID
        self.minYear = minYear
        self.maxYear = maxYear
        self.collectionMode = collectionMode
        self.historyKey = historyKey
        self.viewedAt = viewedAt
        self.accountID = accountID
        self.deviceID = deviceID
        self.watchlistedAt = watchlistedAt
        self.sessionKey = sessionKey
        self.user = user
        self.player = player
        self.session = session
        self.transcodeSession = transcodeSession
        self.media = media
        self.genre = genre
        self.director = director
        self.writer = writer
        self.producer = producer
        self.country = country
        self.collection = collection
        self.label = label
        self.mood = mood
        self.style = style
        self.similar = similar
        self.role = role
        self.guids = guids
        self.marker = marker
        self.chapter = chapter
        self.image = image
        self.ratings = ratings
        self.review = review
        self.ultraBlurColors = ultraBlurColors
        self.extras = extras
        self.related = related
        self.onDeck = onDeck
        self.children = children
        self.stations = stations
        self.extra = extra
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Metadata")
        ratingKey = De.optString(o["ratingKey"])
        key = De.optString(o["key"])
        guid = De.optString(o["guid"])
        type = De.optString(o["type"])
        subtype = De.optString(o["subtype"])
        title = De.optString(o["title"])
        titleSort = De.optString(o["titleSort"])
        originalTitle = De.optString(o["originalTitle"])
        editionTitle = De.optString(o["editionTitle"])
        studio = De.optString(o["studio"])
        contentRating = De.optString(o["contentRating"])
        summary = De.optString(o["summary"])
        tagline = De.optString(o["tagline"])
        year = De.optI64(o["year"])
        index = De.optI64(o["index"])
        parentIndex = De.optI64(o["parentIndex"])
        rating = De.optF64(o["rating"])
        audienceRating = De.optF64(o["audienceRating"])
        userRating = De.optF64(o["userRating"])
        ratingImage = De.optString(o["ratingImage"])
        audienceRatingImage = De.optString(o["audienceRatingImage"])
        viewCount = De.optI64(o["viewCount"])
        skipCount = De.optI64(o["skipCount"])
        viewOffset = De.optI64(o["viewOffset"])
        lastViewedAt = De.optI64(o["lastViewedAt"])
        lastRatedAt = De.optI64(o["lastRatedAt"])
        addedAt = De.optI64(o["addedAt"])
        updatedAt = De.optI64(o["updatedAt"])
        originallyAvailableAt = De.optString(o["originallyAvailableAt"])
        duration = De.optI64(o["duration"])
        thumb = De.optString(o["thumb"])
        art = De.optString(o["art"])
        banner = De.optString(o["banner"])
        theme = De.optString(o["theme"])
        composite = De.optString(o["composite"])
        chapterSource = De.optString(o["chapterSource"])
        primaryExtraKey = De.optString(o["primaryExtraKey"])
        extraType = De.optI64(o["extraType"])
        parentRatingKey = De.optString(o["parentRatingKey"])
        parentKey = De.optString(o["parentKey"])
        parentGuid = De.optString(o["parentGuid"])
        parentTitle = De.optString(o["parentTitle"])
        parentThumb = De.optString(o["parentThumb"])
        parentArt = De.optString(o["parentArt"])
        parentTheme = De.optString(o["parentTheme"])
        parentYear = De.optI64(o["parentYear"])
        parentStudio = De.optString(o["parentStudio"])
        grandparentRatingKey = De.optString(o["grandparentRatingKey"])
        grandparentKey = De.optString(o["grandparentKey"])
        grandparentGuid = De.optString(o["grandparentGuid"])
        grandparentTitle = De.optString(o["grandparentTitle"])
        grandparentThumb = De.optString(o["grandparentThumb"])
        grandparentArt = De.optString(o["grandparentArt"])
        grandparentTheme = De.optString(o["grandparentTheme"])
        leafCount = De.optI64(o["leafCount"])
        viewedLeafCount = De.optI64(o["viewedLeafCount"])
        childCount = De.optI64(o["childCount"])
        seasonCount = De.optI64(o["seasonCount"])
        albumCount = De.optI64(o["albumCount"])
        librarySectionID = De.optI64(o["librarySectionID"])
        librarySectionTitle = De.optString(o["librarySectionTitle"])
        librarySectionKey = De.optString(o["librarySectionKey"])
        playlistType = De.optString(o["playlistType"])
        smart = De.boolean(o["smart"])
        playlistItemID = De.optI64(o["playlistItemID"])
        playQueueItemID = De.optI64(o["playQueueItemID"])
        minYear = De.optString(o["minYear"])
        maxYear = De.optString(o["maxYear"])
        collectionMode = De.optI64(o["collectionMode"])
        historyKey = De.optString(o["historyKey"])
        viewedAt = De.optI64(o["viewedAt"])
        accountID = De.optI64(o["accountID"])
        deviceID = De.optI64(o["deviceID"])
        watchlistedAt = De.optI64(o["watchlistedAt"])
        sessionKey = De.optString(o["sessionKey"])
        user = try De.optional(o["User"], SessionUser.init(json:), key: "User")
        player = try De.optional(o["Player"], Player.init(json:), key: "Player")
        session = try De.optional(o["Session"], SessionInfo.init(json:), key: "Session")
        transcodeSession = try De.optional(o["TranscodeSession"], TranscodeSession.init(json:), key: "TranscodeSession")
        media = try De.list(o["Media"], Media.init(json:), key: "Media")
        genre = try De.list(o["Genre"], Tag.init(json:), key: "Genre")
        director = try De.list(o["Director"], Tag.init(json:), key: "Director")
        writer = try De.list(o["Writer"], Tag.init(json:), key: "Writer")
        producer = try De.list(o["Producer"], Tag.init(json:), key: "Producer")
        country = try De.list(o["Country"], Tag.init(json:), key: "Country")
        collection = try De.list(o["Collection"], Tag.init(json:), key: "Collection")
        label = try De.list(o["Label"], Tag.init(json:), key: "Label")
        mood = try De.list(o["Mood"], Tag.init(json:), key: "Mood")
        style = try De.list(o["Style"], Tag.init(json:), key: "Style")
        similar = try De.list(o["Similar"], Tag.init(json:), key: "Similar")
        role = try De.list(o["Role"], Tag.init(json:), key: "Role")
        guids = try De.list(o["Guid"], GuidRef.init(json:), key: "Guid")
        marker = try De.list(o["Marker"], Marker.init(json:), key: "Marker")
        chapter = try De.list(o["Chapter"], Chapter.init(json:), key: "Chapter")
        image = try De.list(o["Image"], PlexImage.init(json:), key: "Image")
        ratings = try De.list(o["Rating"], RatingSource.init(json:), key: "Rating")
        review = try De.list(o["Review"], Review.init(json:), key: "Review")
        ultraBlurColors = try De.optional(o["UltraBlurColors"], UltraBlurColors.init(json:), key: "UltraBlurColors")
        extras = try De.optional(o["Extras"], Nested.init(json:), key: "Extras")
        related = try De.optional(o["Related"], Nested.init(json:), key: "Related")
        onDeck = try De.optional(o["OnDeck"], Nested.init(json:), key: "OnDeck")
        children = try De.optional(o["Children"], Nested.init(json:), key: "Children")
        stations = try De.oneOrMany(o["Stations"], Metadata.init(json:), key: "Stations")
        extra = De.extraOf(o, Self.keys)
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = extra
        o["ratingKey"] = JSONValue(ratingKey)
        o["key"] = JSONValue(key)
        o["guid"] = JSONValue(guid)
        o["type"] = JSONValue(type)
        o["subtype"] = JSONValue(subtype)
        o["title"] = JSONValue(title)
        o["titleSort"] = JSONValue(titleSort)
        o["originalTitle"] = JSONValue(originalTitle)
        o["editionTitle"] = JSONValue(editionTitle)
        o["studio"] = JSONValue(studio)
        o["contentRating"] = JSONValue(contentRating)
        o["summary"] = JSONValue(summary)
        o["tagline"] = JSONValue(tagline)
        o["year"] = JSONValue(year)
        o["index"] = JSONValue(index)
        o["parentIndex"] = JSONValue(parentIndex)
        o["rating"] = JSONValue(rating)
        o["audienceRating"] = JSONValue(audienceRating)
        o["userRating"] = JSONValue(userRating)
        o["ratingImage"] = JSONValue(ratingImage)
        o["audienceRatingImage"] = JSONValue(audienceRatingImage)
        o["viewCount"] = JSONValue(viewCount)
        o["skipCount"] = JSONValue(skipCount)
        o["viewOffset"] = JSONValue(viewOffset)
        o["lastViewedAt"] = JSONValue(lastViewedAt)
        o["lastRatedAt"] = JSONValue(lastRatedAt)
        o["addedAt"] = JSONValue(addedAt)
        o["updatedAt"] = JSONValue(updatedAt)
        o["originallyAvailableAt"] = JSONValue(originallyAvailableAt)
        o["duration"] = JSONValue(duration)
        o["thumb"] = JSONValue(thumb)
        o["art"] = JSONValue(art)
        o["banner"] = JSONValue(banner)
        o["theme"] = JSONValue(theme)
        o["composite"] = JSONValue(composite)
        o["chapterSource"] = JSONValue(chapterSource)
        o["primaryExtraKey"] = JSONValue(primaryExtraKey)
        o["extraType"] = JSONValue(extraType)
        o["parentRatingKey"] = JSONValue(parentRatingKey)
        o["parentKey"] = JSONValue(parentKey)
        o["parentGuid"] = JSONValue(parentGuid)
        o["parentTitle"] = JSONValue(parentTitle)
        o["parentThumb"] = JSONValue(parentThumb)
        o["parentArt"] = JSONValue(parentArt)
        o["parentTheme"] = JSONValue(parentTheme)
        o["parentYear"] = JSONValue(parentYear)
        o["parentStudio"] = JSONValue(parentStudio)
        o["grandparentRatingKey"] = JSONValue(grandparentRatingKey)
        o["grandparentKey"] = JSONValue(grandparentKey)
        o["grandparentGuid"] = JSONValue(grandparentGuid)
        o["grandparentTitle"] = JSONValue(grandparentTitle)
        o["grandparentThumb"] = JSONValue(grandparentThumb)
        o["grandparentArt"] = JSONValue(grandparentArt)
        o["grandparentTheme"] = JSONValue(grandparentTheme)
        o["leafCount"] = JSONValue(leafCount)
        o["viewedLeafCount"] = JSONValue(viewedLeafCount)
        o["childCount"] = JSONValue(childCount)
        o["seasonCount"] = JSONValue(seasonCount)
        o["albumCount"] = JSONValue(albumCount)
        o["librarySectionID"] = JSONValue(librarySectionID)
        o["librarySectionTitle"] = JSONValue(librarySectionTitle)
        o["librarySectionKey"] = JSONValue(librarySectionKey)
        o["playlistType"] = JSONValue(playlistType)
        o["smart"] = JSONValue(smart)
        o["playlistItemID"] = JSONValue(playlistItemID)
        o["playQueueItemID"] = JSONValue(playQueueItemID)
        o["minYear"] = JSONValue(minYear)
        o["maxYear"] = JSONValue(maxYear)
        o["collectionMode"] = JSONValue(collectionMode)
        o["historyKey"] = JSONValue(historyKey)
        o["viewedAt"] = JSONValue(viewedAt)
        o["accountID"] = JSONValue(accountID)
        o["deviceID"] = JSONValue(deviceID)
        o["watchlistedAt"] = JSONValue(watchlistedAt)
        o["sessionKey"] = JSONValue(sessionKey)
        o["User"] = user?.json ?? .null
        o["Player"] = player?.json ?? .null
        o["Session"] = session?.json ?? .null
        o["TranscodeSession"] = transcodeSession?.json ?? .null
        o["Media"] = .array(media.map(\.json))
        o["Genre"] = .array(genre.map(\.json))
        o["Director"] = .array(director.map(\.json))
        o["Writer"] = .array(writer.map(\.json))
        o["Producer"] = .array(producer.map(\.json))
        o["Country"] = .array(country.map(\.json))
        o["Collection"] = .array(collection.map(\.json))
        o["Label"] = .array(label.map(\.json))
        o["Mood"] = .array(mood.map(\.json))
        o["Style"] = .array(style.map(\.json))
        o["Similar"] = .array(similar.map(\.json))
        o["Role"] = .array(role.map(\.json))
        o["Guid"] = .array(guids.map(\.json))
        o["Marker"] = .array(marker.map(\.json))
        o["Chapter"] = .array(chapter.map(\.json))
        o["Image"] = .array(image.map(\.json))
        o["Rating"] = .array(ratings.map(\.json))
        o["Review"] = .array(review.map(\.json))
        o["UltraBlurColors"] = ultraBlurColors?.json ?? .null
        o["Extras"] = extras?.json ?? .null
        o["Related"] = related?.json ?? .null
        o["OnDeck"] = onDeck?.json ?? .null
        o["Children"] = children?.json ?? .null
        o["Stations"] = .array(stations.map(\.json))
        return .object(o)
    }

    /// The modelled keys; everything else goes to `extra`.
    static let keys: Set<String> = ["ratingKey", "key", "guid", "type", "subtype", "title", "titleSort", "originalTitle", "editionTitle", "studio", "contentRating", "summary", "tagline", "year", "index", "parentIndex", "rating", "audienceRating", "userRating", "ratingImage", "audienceRatingImage", "viewCount", "skipCount", "viewOffset", "lastViewedAt", "lastRatedAt", "addedAt", "updatedAt", "originallyAvailableAt", "duration", "thumb", "art", "banner", "theme", "composite", "chapterSource", "primaryExtraKey", "extraType", "parentRatingKey", "parentKey", "parentGuid", "parentTitle", "parentThumb", "parentArt", "parentTheme", "parentYear", "parentStudio", "grandparentRatingKey", "grandparentKey", "grandparentGuid", "grandparentTitle", "grandparentThumb", "grandparentArt", "grandparentTheme", "leafCount", "viewedLeafCount", "childCount", "seasonCount", "albumCount", "librarySectionID", "librarySectionTitle", "librarySectionKey", "playlistType", "smart", "playlistItemID", "playQueueItemID", "minYear", "maxYear", "collectionMode", "historyKey", "viewedAt", "accountID", "deviceID", "watchlistedAt", "sessionKey", "User", "Player", "Session", "TranscodeSession", "Media", "Genre", "Director", "Writer", "Producer", "Country", "Collection", "Label", "Mood", "Style", "Similar", "Role", "Guid", "Marker", "Chapter", "Image", "Rating", "Review", "UltraBlurColors", "Extras", "Related", "OnDeck", "Children", "Stations"]
}

/// Nested `{ "Metadata": [...], "Hub": [...] }` blocks (Extras, Related, OnDeck).
public struct Nested: PlexModel {
    public var size: Int? = nil
    public var metadata: [Metadata] = []
    public var hub: [Hub] = []

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        size: Int? = nil,
        metadata: [Metadata] = [],
        hub: [Hub] = []
    ) {
        self.size = size
        self.metadata = metadata
        self.hub = hub
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Nested")
        size = De.optI64(o["size"])
        metadata = try De.oneOrMany(o["Metadata"], Metadata.init(json:), key: "Metadata")
        hub = try De.oneOrMany(o["Hub"], Hub.init(json:), key: "Hub")
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["size"] = JSONValue(size)
        o["Metadata"] = .array(metadata.map(\.json))
        o["Hub"] = .array(hub.map(\.json))
        return .object(o)
    }
}

/// A tag reference: genre, director, actor (role), collection, label, ...
public struct Tag: PlexModel {
    public var id: Int? = nil
    public var tag: String? = nil
    public var tagKey: String? = nil
    public var filter: String? = nil
    public var role: String? = nil
    public var thumb: String? = nil
    public var count: Int? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        id: Int? = nil,
        tag: String? = nil,
        tagKey: String? = nil,
        filter: String? = nil,
        role: String? = nil,
        thumb: String? = nil,
        count: Int? = nil
    ) {
        self.id = id
        self.tag = tag
        self.tagKey = tagKey
        self.filter = filter
        self.role = role
        self.thumb = thumb
        self.count = count
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Tag")
        id = De.optI64(o["id"])
        tag = De.optString(o["tag"])
        tagKey = De.optString(o["tagKey"])
        filter = De.optString(o["filter"])
        role = De.optString(o["role"])
        thumb = De.optString(o["thumb"])
        count = De.optI64(o["count"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["id"] = JSONValue(id)
        o["tag"] = JSONValue(tag)
        o["tagKey"] = JSONValue(tagKey)
        o["filter"] = JSONValue(filter)
        o["role"] = JSONValue(role)
        o["thumb"] = JSONValue(thumb)
        o["count"] = JSONValue(count)
        return .object(o)
    }
}

public struct GuidRef: PlexModel {
    public var id: String? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        id: String? = nil
    ) {
        self.id = id
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "GuidRef")
        id = De.optString(o["id"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["id"] = JSONValue(id)
        return .object(o)
    }
}

/// Intro / credits / commercial markers.
public struct Marker: PlexModel {
    public var id: Int? = nil
    public var type: String? = nil
    public var startTimeOffset: Int? = nil
    public var endTimeOffset: Int? = nil
    public var `final`: Bool = false

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        id: Int? = nil,
        type: String? = nil,
        startTimeOffset: Int? = nil,
        endTimeOffset: Int? = nil,
        final: Bool = false
    ) {
        self.id = id
        self.type = type
        self.startTimeOffset = startTimeOffset
        self.endTimeOffset = endTimeOffset
        self.`final` = `final`
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Marker")
        id = De.optI64(o["id"])
        type = De.optString(o["type"])
        startTimeOffset = De.optI64(o["startTimeOffset"])
        endTimeOffset = De.optI64(o["endTimeOffset"])
        `final` = De.boolean(o["final"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["id"] = JSONValue(id)
        o["type"] = JSONValue(type)
        o["startTimeOffset"] = JSONValue(startTimeOffset)
        o["endTimeOffset"] = JSONValue(endTimeOffset)
        o["final"] = JSONValue(`final`)
        return .object(o)
    }
}

public struct Chapter: PlexModel {
    public var id: Int? = nil
    public var index: Int? = nil
    public var tag: String? = nil
    public var startTimeOffset: Int? = nil
    public var endTimeOffset: Int? = nil
    public var thumb: String? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        id: Int? = nil,
        index: Int? = nil,
        tag: String? = nil,
        startTimeOffset: Int? = nil,
        endTimeOffset: Int? = nil,
        thumb: String? = nil
    ) {
        self.id = id
        self.index = index
        self.tag = tag
        self.startTimeOffset = startTimeOffset
        self.endTimeOffset = endTimeOffset
        self.thumb = thumb
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Chapter")
        id = De.optI64(o["id"])
        index = De.optI64(o["index"])
        tag = De.optString(o["tag"])
        startTimeOffset = De.optI64(o["startTimeOffset"])
        endTimeOffset = De.optI64(o["endTimeOffset"])
        thumb = De.optString(o["thumb"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["id"] = JSONValue(id)
        o["index"] = JSONValue(index)
        o["tag"] = JSONValue(tag)
        o["startTimeOffset"] = JSONValue(startTimeOffset)
        o["endTimeOffset"] = JSONValue(endTimeOffset)
        o["thumb"] = JSONValue(thumb)
        return .object(o)
    }
}

public struct PlexImage: PlexModel {
    public var alt: String? = nil
    public var type: String? = nil
    public var url: String? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        alt: String? = nil,
        type: String? = nil,
        url: String? = nil
    ) {
        self.alt = alt
        self.type = type
        self.url = url
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Image")
        alt = De.optString(o["alt"])
        type = De.optString(o["type"])
        url = De.optString(o["url"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["alt"] = JSONValue(alt)
        o["type"] = JSONValue(type)
        o["url"] = JSONValue(url)
        return .object(o)
    }
}

public struct RatingSource: PlexModel {
    public var image: String? = nil
    public var value: Double? = nil
    public var type: String? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        image: String? = nil,
        value: Double? = nil,
        type: String? = nil
    ) {
        self.image = image
        self.value = value
        self.type = type
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "RatingSource")
        image = De.optString(o["image"])
        value = De.optF64(o["value"])
        type = De.optString(o["type"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["image"] = JSONValue(image)
        o["value"] = JSONValue(value)
        o["type"] = JSONValue(type)
        return .object(o)
    }
}

public struct Review: PlexModel {
    public var tag: String? = nil
    public var text: String? = nil
    public var image: String? = nil
    public var source: String? = nil
    public var link: String? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        tag: String? = nil,
        text: String? = nil,
        image: String? = nil,
        source: String? = nil,
        link: String? = nil
    ) {
        self.tag = tag
        self.text = text
        self.image = image
        self.source = source
        self.link = link
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "Review")
        tag = De.optString(o["tag"])
        text = De.optString(o["text"])
        image = De.optString(o["image"])
        source = De.optString(o["source"])
        link = De.optString(o["link"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["tag"] = JSONValue(tag)
        o["text"] = JSONValue(text)
        o["image"] = JSONValue(image)
        o["source"] = JSONValue(source)
        o["link"] = JSONValue(link)
        return .object(o)
    }
}

/// Four corner colours (hex, no `#`) for Plex's "ultra blur" backgrounds.
public struct UltraBlurColors: PlexModel {
    public var topLeft: String? = nil
    public var topRight: String? = nil
    public var bottomRight: String? = nil
    public var bottomLeft: String? = nil

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        topLeft: String? = nil,
        topRight: String? = nil,
        bottomRight: String? = nil,
        bottomLeft: String? = nil
    ) {
        self.topLeft = topLeft
        self.topRight = topRight
        self.bottomRight = bottomRight
        self.bottomLeft = bottomLeft
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "UltraBlurColors")
        topLeft = De.optString(o["topLeft"])
        topRight = De.optString(o["topRight"])
        bottomRight = De.optString(o["bottomRight"])
        bottomLeft = De.optString(o["bottomLeft"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["topLeft"] = JSONValue(topLeft)
        o["topRight"] = JSONValue(topRight)
        o["bottomRight"] = JSONValue(bottomRight)
        o["bottomLeft"] = JSONValue(bottomLeft)
        return .object(o)
    }
}

/// Match candidate from `/library/metadata/{id}/matches`.
public struct MatchResult: PlexModel {
    public var guid: String? = nil
    public var name: String? = nil
    public var score: Int? = nil
    public var year: Int? = nil
    public var thumb: String? = nil
    public var summary: String? = nil
    public var matched: Bool = false

    public init() {}

    /// `defaultX({...})`: any field can be given; the rest are the defaults.
    public init(
        guid: String? = nil,
        name: String? = nil,
        score: Int? = nil,
        year: Int? = nil,
        thumb: String? = nil,
        summary: String? = nil,
        matched: Bool = false
    ) {
        self.guid = guid
        self.name = name
        self.score = score
        self.year = year
        self.thumb = thumb
        self.summary = summary
        self.matched = matched
    }

    public init(json v: JSONValue) throws {
        let o = try De.obj(v, "MatchResult")
        guid = De.optString(o["guid"])
        name = De.optString(o["name"])
        score = De.optI64(o["score"])
        year = De.optI64(o["year"])
        thumb = De.optString(o["thumb"])
        summary = De.optString(o["summary"])
        matched = De.boolean(o["matched"])
    }

    public var json: JSONValue {
        var o: [String: JSONValue] = [:]
        o["guid"] = JSONValue(guid)
        o["name"] = JSONValue(name)
        o["score"] = JSONValue(score)
        o["year"] = JSONValue(year)
        o["thumb"] = JSONValue(thumb)
        o["summary"] = JSONValue(summary)
        o["matched"] = JSONValue(matched)
        return .object(o)
    }
}

private func pad2(_ n: Int) -> String {
    let s = String(n)
    return s.count < 2 ? String(repeating: "0", count: 2 - s.count) + s : s
}

/// `impl Metadata`: pure helpers over the item.
extension Metadata {
    /// Key under which Patina tags an item with the server it came from (see `serverId`).
    public static let serverTag = "patinaServer"

    /// The machine identifier of the server this item came from, when it is not the
    /// session's own server. Patina sets it on items found on other servers so that
    /// navigation, playback and reporting stay with the right one. Plex never sends it.
    public static func serverId(_ m: Metadata) -> String? {
        guard case .string(let v)? = m.extra[serverTag], !v.isEmpty else { return nil }
        return v
    }

    /// Tag the item with the server it lives on (`nil` clears the tag).
    public static func setServerId(_ m: inout Metadata, _ server: String?) {
        m.extra[serverTag] = server.map { .string($0) }
    }

    public static func ratingKey(_ m: Metadata) -> String {
        m.ratingKey ?? ""
    }

    public static func title(_ m: Metadata) -> String {
        m.title ?? ""
    }

    public static func kind(_ m: Metadata) -> String {
        m.type ?? ""
    }

    /// Whether this item can be handed to a player directly.
    public static func isPlayable(_ m: Metadata) -> Bool {
        let k = kind(m)
        return ["movie", "episode", "track", "clip", "trailer", "photo"].contains(k)
            || (!m.media.isEmpty && !["show", "season", "artist", "album"].contains(k))
    }

    /// Whether this item is a container of other items (show, season, album, ...).
    public static func isContainer(_ m: Metadata) -> Bool {
        ["show", "season", "artist", "album", "collection", "playlist", "photoalbum"].contains(kind(m))
    }

    public static func isWatched(_ m: Metadata) -> Bool {
        if let leaves = m.leafCount, let viewed = m.viewedLeafCount, leaves > 0 {
            return viewed >= leaves
        }
        return (m.viewCount ?? 0) > 0 && (m.viewOffset ?? 0) == 0
    }

    public static func unwatchedLeaves(_ m: Metadata) -> Int? {
        guard let leaves = m.leafCount else { return nil }
        return leaves - (m.viewedLeafCount ?? 0)
    }

    /// Resume progress in `0.0...1.0`, if partially watched.
    public static func progress(_ m: Metadata) -> Double? {
        guard let offset = m.viewOffset, let duration = m.duration else { return nil }
        return duration > 0 && offset > 0 ? min(1, max(0, Double(offset) / Double(duration))) : nil
    }

    /// Best available image for a poster-shaped card.
    public static func poster(_ m: Metadata) -> String? {
        switch kind(m) {
        case "episode": m.grandparentThumb ?? m.parentThumb ?? m.thumb
        case "season": m.thumb ?? m.parentThumb
        case "playlist": m.composite ?? m.thumb
        default: m.thumb ?? m.composite ?? m.parentThumb
        }
    }

    /// Best available background art.
    public static func backdrop(_ m: Metadata) -> String? {
        m.art ?? m.grandparentArt ?? m.parentArt ?? imageOfType(m, "background")
    }

    public static func imageOfType(_ m: Metadata, _ kind: String) -> String? {
        m.image.first(where: { $0.type == kind })?.url
    }

    /// Title including show/season context, e.g. "Show — S01E02 · Pilot".
    public static func fullTitle(_ m: Metadata) -> String {
        switch kind(m) {
        case "episode": "\(m.grandparentTitle ?? "") — \(episodeCode(m)) · \(title(m))"
        case "season": "\(m.parentTitle ?? "") — \(title(m))"
        case "track": "\(m.grandparentTitle ?? "") — \(title(m))"
        default: m.year.map { "\(title(m)) (\($0))" } ?? title(m)
        }
    }

    /// "S01E02" style code for episodes.
    public static func episodeCode(_ m: Metadata) -> String {
        if let season = m.parentIndex, let index = m.index { return "S\(pad2(season))E\(pad2(index))" }
        if m.parentIndex == nil, let index = m.index { return "E\(pad2(index))" }
        return ""
    }

    /// The plex.tv discover id derived from a `plex://type/<id>` guid.
    public static func discoverId(_ m: Metadata) -> String? {
        guard let guid = m.guid, guid.hasPrefix("plex://") else { return nil }
        return guid.dropFirst("plex://".count).split(separator: "/", omittingEmptySubsequences: false).last
            .map(String.init)
    }

    public static func markersOf(_ m: Metadata, _ kind: String) -> [Marker] {
        m.marker.filter { $0.type == kind }
    }

    public static func extras(_ m: Metadata) -> [Metadata] {
        m.extras?.metadata ?? []
    }

    public static func relatedHubs(_ m: Metadata) -> [Hub] {
        m.related?.hub ?? []
    }

    public static func onDeckItem(_ m: Metadata) -> Metadata? {
        m.onDeck?.metadata.first
    }
}

extension Tag {
    public static func name(_ t: Tag) -> String {
        t.tag ?? ""
    }
}
