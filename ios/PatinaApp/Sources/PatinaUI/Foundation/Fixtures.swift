// Sample items for `#Preview`s (no desktop twin). Artwork doesn't load without a server,
// so previews show the placeholders.

import PatinaCore
import PlexKit

enum Fixtures {
    static func movie(_ key: String = "1", _ title: String = "The Long Voyage", year: Int = 2021, progress: Double? = nil)
        -> Metadata
    {
        var m = Metadata()
        m.ratingKey = key
        m.key = "/library/metadata/\(key)"
        m.type = "movie"
        m.title = title
        m.year = year
        m.duration = 7_440_000
        m.contentRating = "PG-13"
        m.summary = "A crew of misfits sets out across a frozen sea to find the ship that vanished with their families, and finds more than they bargained for."
        m.tagline = "Some journeys never end."
        m.rating = 8.1
        m.ratingImage = "rottentomatoes://image.rating.ripe"
        m.audienceRating = 7.6
        m.audienceRatingImage = "rottentomatoes://image.rating.upright"
        m.thumb = "/library/metadata/\(key)/thumb"
        m.art = "/library/metadata/\(key)/art"
        m.librarySectionID = 1
        if let progress { m.viewOffset = Int(Double(m.duration ?? 0) * progress) }
        var genre = Tag()
        genre.tag = "Adventure"
        genre.filter = "genre=1"
        m.genre = [genre]
        var media = Media()
        media.videoResolution = "4k"
        media.videoCodec = "hevc"
        media.audioCodec = "truehd"
        media.audioChannels = 8
        var part = Part()
        part.id = 1
        var audio = MediaStream()
        audio.id = 2
        audio.streamType = StreamType.audio
        audio.displayTitle = "English (TRUEHD 7.1)"
        audio.selected = true
        var sub = MediaStream()
        sub.id = 3
        sub.streamType = StreamType.subtitle
        sub.displayTitle = "English (SRT)"
        part.stream = [audio, sub]
        media.part = [part]
        m.media = [media]
        return m
    }

    static func episode(_ key: String = "20", index: Int = 3, watched: Bool = false) -> Metadata {
        var m = Metadata()
        m.ratingKey = key
        m.type = "episode"
        m.title = "The Lighthouse"
        m.grandparentTitle = "Northern Lights"
        m.parentIndex = 1
        m.index = index
        m.duration = 2_580_000
        m.viewCount = watched ? 1 : nil
        m.summary = "The keeper of the lighthouse tells a story nobody believes."
        m.originallyAvailableAt = "2023-02-14"
        return m
    }

    static func show(_ key: String = "10", unwatched: Int = 4) -> Metadata {
        var m = Metadata()
        m.ratingKey = key
        m.type = "show"
        m.title = "Northern Lights"
        m.year = 2023
        m.childCount = 2
        m.leafCount = 16
        m.viewedLeafCount = 16 - unwatched
        return m
    }

    static func album(_ key: String = "30") -> Metadata {
        var m = Metadata()
        m.ratingKey = key
        m.type = "album"
        m.title = "Glass Harbour"
        m.parentTitle = "The Tidewater"
        m.year = 2019
        return m
    }

    static var hub: Hub {
        var h = Hub()
        h.title = "Recently Added Movies"
        h.hubIdentifier = "movie.recentlyadded.1"
        h.key = "/hubs/sections/1/recentlyAdded"
        h.more = true
        h.metadata = (1...8).map { movie(String($0), ["Arrival", "Dune", "Heat", "Alien", "Coco", "Up", "Jaws", "Big"][$0 - 1]) }
        return h
    }

    static var sections: [Directory] {
        func section(_ key: String, _ title: String, _ type: String) -> Directory {
            var d = Directory()
            d.key = key
            d.title = title
            d.type = type
            return d
        }
        return [section("1", "Movies", "movie"), section("2", "TV Shows", "show"), section("3", "Music", "artist")]
    }
}
