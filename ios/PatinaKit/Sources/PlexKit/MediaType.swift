// Port of desktop/plex/media-type.ts.
// Plex's numeric media type ids and stream types (from crates/plex/src/lib.rs).

/// Plex's numeric media type identifiers, used for `type=` query parameters.
public enum MediaType: Int, Sendable, Hashable, CaseIterable {
    case movie = 1
    case show = 2
    case season = 3
    case episode = 4
    case trailer = 5
    case person = 7
    case artist = 8
    case album = 9
    case track = 10
    case clip = 12
    case photo = 13
    case photoAlbum = 14
    case playlist = 15
    case playlistFolder = 16
    case collection = 18

    /// `MediaType::id()`: the variants are already their ids.
    public var id: Int { rawValue }

    /// Map a metadata `type` string ("movie", "episode", ...) to its id.
    public static func fromName(_ name: String) -> MediaType? {
        switch name {
        case "movie": .movie
        case "show": .show
        case "season": .season
        case "episode": .episode
        case "trailer": .trailer
        case "person": .person
        case "artist": .artist
        case "album": .album
        case "track": .track
        case "clip": .clip
        case "photo": .photo
        case "photoalbum": .photoAlbum
        case "playlist": .playlist
        case "playlistFolder": .playlistFolder
        case "collection": .collection
        default: nil
        }
    }
}

/// Stream types found on `MediaStream.streamType` (`streamType` in TypeScript).
public enum StreamType {
    public static let video = 1
    public static let audio = 2
    public static let subtitle = 3
    public static let lyrics = 4
}
