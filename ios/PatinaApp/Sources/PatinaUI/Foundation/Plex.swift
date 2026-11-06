// Port of web/src/lib/plex.ts: the helpers over Plex metadata that the screens use and
// PlexKit's models don't have (those are `Metadata.title(m)`, `Metadata.poster(m)`, ...).

import Foundation
import PlexKit

/// The shape of a card: 2:3 posters, 16:9 episodes and clips, square music.
public enum CardShape: Sendable {
    case poster
    case landscape
    case square

    /// Width over height.
    public var aspect: CGFloat {
        switch self {
        case .poster: 2.0 / 3.0
        case .landscape: 16.0 / 9.0
        case .square: 1
        }
    }

    /// The transcoder size the desktop asks for (`SIZES` in Card.svelte).
    public var imageSize: (Int, Int) {
        switch self {
        case .poster: (300, 450)
        case .landscape: (560, 315)
        case .square: (360, 360)
        }
    }
}

public enum PlexUI {
    /// Something a Play button makes sense for.
    public static func canPlay(_ m: Metadata) -> Bool {
        Metadata.isPlayable(m) || Metadata.isContainer(m)
    }

    /// Things that have a watched state worth toggling.
    public static func hasWatchState(_ m: Metadata) -> Bool {
        !["playlist", "photo", "photoalbum", "artist", "album", "track", "collection"].contains(Metadata.kind(m))
    }

    /// Whether the bulk audio/subtitle dialog applies.
    public static func supportsTracks(_ m: Metadata) -> Bool {
        ["show", "season", "episode"].contains(Metadata.kind(m))
    }

    /// Playlist type an item can be added to.
    public static func playlistKind(_ m: Metadata) -> String {
        let k = Metadata.kind(m)
        if ["track", "album", "artist"].contains(k) { return "audio" }
        if ["photo", "photoalbum"].contains(k) { return "photo" }
        return "video"
    }

    /// Secondary line under a card title.
    public static func subtitle(_ m: Metadata) -> String {
        switch Metadata.kind(m) {
        case "episode":
            let show = m.grandparentTitle ?? ""
            let code = Metadata.episodeCode(m)
            return show.isEmpty ? code : "\(show) · \(code)"
        case "season":
            return m.leafCount.map { Format.plural($0, "episode") } ?? (m.parentTitle ?? "")
        case "show":
            return m.childCount.map { Format.plural($0, "season") } ?? (m.year.map(String.init) ?? "")
        case "album":
            return m.parentTitle ?? (m.year.map(String.init) ?? "")
        case "track":
            return m.grandparentTitle ?? ""
        case "playlist":
            return Format.plural(m.leafCount ?? 0, "item")
        case "collection":
            return m.childCount.map { Format.plural($0, "item") } ?? ""
        default:
            return m.year.map(String.init) ?? ""
        }
    }

    /// Music (and other sound-only items): the player shows the "now playing" layout.
    public static func isAudio(_ m: Metadata) -> Bool {
        Metadata.kind(m) == "track"
    }

    /// Title and subtitle for "now playing"-style rows.
    public static func playingLines(_ m: Metadata) -> (String, String) {
        switch Metadata.kind(m) {
        case "episode": (m.grandparentTitle ?? "", "\(Metadata.episodeCode(m)) · \(Metadata.title(m))")
        case "track": (Metadata.title(m), m.grandparentTitle ?? "")
        default: (Metadata.title(m), m.year.map(String.init) ?? "")
        }
    }

    public static func shape(_ m: Metadata) -> CardShape {
        let k = Metadata.kind(m)
        if k == "episode" || k == "clip" { return .landscape }
        if ["album", "artist", "track", "playlist", "photo", "photoalbum"].contains(k) { return .square }
        return .poster
    }

    /// One shape for a row of mixed items, so the cards line up: theirs if they agree, else
    /// posters (episodes show their show's poster).
    public static func commonShape(_ items: [Metadata]) -> CardShape {
        let shapes = Set(items.map { shape($0) })
        if shapes.count == 1, let only = shapes.first { return only }
        return shapes.contains(.poster) || shapes.isEmpty ? .poster : .square
    }

    /// Where opening an item goes (`routeFor`).
    public static func route(_ m: Metadata) -> Route {
        if Metadata.kind(m) == "playlist" { return .playlist(id: Metadata.ratingKey(m)) }
        return .item(ratingKey: Metadata.ratingKey(m), server: Metadata.serverId(m))
    }

    /// "4K", "1080p", "SD" from `videoResolution`.
    public static func resolution(_ r: String?) -> String? {
        guard let r, !r.isEmpty else { return nil }
        if r.lowercased() == "4k" { return "4K" }
        if r.lowercased() == "sd" { return "SD" }
        if r.allSatisfy(\.isNumber) { return "\(r)p" }
        return r.uppercased()
    }

    /// A value the server sent that the model keeps in `extra` (`collectionSort`, artwork `provider`).
    public static func extra(_ m: Metadata, _ key: String) -> String? {
        switch m.extra[key] {
        case .string(let s)?: s
        case .number(let n)?: n.rounded() == n ? String(Int(n)) : String(n)
        case .bool(let b)?: b ? "1" : "0"
        default: nil
        }
    }

    /// Artwork choices mark the current one with `selected` (a bool or 1).
    public static func isSelected(_ m: Metadata) -> Bool {
        switch m.extra["selected"] {
        case .bool(let b)?: b
        case .number(let n)?: n != 0
        case .string(let s)?: s == "1" || s == "true"
        default: false
        }
    }
}
