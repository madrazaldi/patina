import Foundation

// Helpers over Plex metadata, mirroring web/src/lib/plex.ts and the methods
// on the Rust models (crates/plex/src/models).

extension Metadata {
    var kind: String { type ?? "" }
    var name: String { title ?? "" }

    var isPlayable: Bool {
        ["movie", "episode", "track", "clip", "trailer", "photo"].contains(kind)
            || (!media.isEmpty && !["show", "season", "artist", "album"].contains(kind))
    }

    var isContainer: Bool {
        ["show", "season", "artist", "album", "collection", "playlist", "photoalbum"].contains(kind)
    }

    /// Something a Play button makes sense for.
    var canPlay: Bool { isPlayable || isContainer }

    var isWatched: Bool {
        if let leaves = leafCount, leaves > 0, let viewed = viewedLeafCount { return viewed >= leaves }
        return (viewCount ?? 0) > 0 && (viewOffset ?? 0) == 0
    }

    var unwatchedLeaves: Int? { leafCount.map { $0 - (viewedLeafCount ?? 0) } }

    /// Resume progress in 0...1, if partially watched.
    var progress: Double? {
        let offset = Double(viewOffset ?? 0), total = Double(duration ?? 0)
        return total > 0 && offset > 0 ? min(1, offset / total) : nil
    }

    /// Best image for a poster-shaped card.
    var posterPath: String? {
        switch kind {
        case "episode": grandparentThumb ?? parentThumb ?? thumb
        case "season": thumb ?? parentThumb
        case "playlist": composite ?? thumb
        default: thumb ?? composite ?? parentThumb
        }
    }

    /// Best background art.
    var backdropPath: String? {
        art ?? grandparentArt ?? parentArt ?? images.first(where: { $0.type == "background" })?.url
    }

    /// "S01E02" for episodes.
    var episodeCode: String {
        let pad = { (n: Int) in String(format: "%02d", n) }
        if let s = parentIndex, let e = index { return "S\(pad(s))E\(pad(e))" }
        if let e = index { return "E\(pad(e))" }
        return ""
    }

    /// Title with show/season context, e.g. "Show — S01E02 · Pilot".
    var fullTitle: String {
        switch kind {
        case "episode": "\(grandparentTitle ?? "") — \(episodeCode) · \(name)"
        case "season": "\(parentTitle ?? "") — \(name)"
        case "track": "\(grandparentTitle ?? "") — \(name)"
        default: year.map { "\(name) (\($0))" } ?? name
        }
    }

    /// The plex.tv discover id from a `plex://type/<id>` guid.
    var discoverId: String? {
        guard let g = guid, g.hasPrefix("plex://") else { return nil }
        return g.split(separator: "/").last.map(String.init)
    }

    /// Secondary line under a card title.
    var subtitle: String {
        switch kind {
        case "episode":
            let show = grandparentTitle ?? ""
            return show.isEmpty ? episodeCode : "\(show) · \(episodeCode)"
        case "season": return leafCount.map { Fmt.plural($0, "episode") } ?? (parentTitle ?? "")
        case "show": return childCount.map { Fmt.plural($0, "season") } ?? year.map(String.init) ?? ""
        case "album": return parentTitle ?? year.map(String.init) ?? ""
        case "track": return grandparentTitle ?? ""
        case "playlist": return Fmt.plural(leafCount ?? 0, "item")
        case "collection": return childCount.map { Fmt.plural($0, "item") } ?? ""
        default: return year.map(String.init) ?? ""
        }
    }

    /// Title and subtitle for "now playing"-style rows.
    var playingLines: (String, String) {
        switch kind {
        case "episode": (grandparentTitle ?? "", "\(episodeCode) · \(name)")
        case "track": (name, grandparentTitle ?? "")
        default: (name, year.map(String.init) ?? "")
        }
    }

    var shape: CardShape {
        if kind == "episode" || kind == "clip" { return .landscape }
        if ["album", "artist", "track", "playlist", "photo", "photoalbum"].contains(kind) { return .square }
        return .poster
    }

    /// Playlist type an item can be added to.
    var playlistKind: String {
        if ["track", "album", "artist"].contains(kind) { return "audio" }
        if ["photo", "photoalbum"].contains(kind) { return "photo" }
        return "video"
    }

    /// Whether the bulk audio/subtitle dialog applies.
    var supportsTracks: Bool { ["show", "season", "episode"].contains(kind) }

    /// Things that have a watched state worth toggling.
    var hasWatchState: Bool {
        !["playlist", "photo", "photoalbum", "artist", "album", "track", "collection"].contains(kind)
    }

    var remaining: String? {
        guard let d = duration, let o = viewOffset else { return nil }
        return d - o > 0 ? "\(Fmt.duration(d - o)) left" : nil
    }
}

enum CardShape {
    case poster, landscape, square

    /// Height over width.
    var ratio: CGFloat {
        switch self {
        case .poster: 1.5
        case .landscape: 9.0 / 16.0
        case .square: 1
        }
    }
}

extension Part {
    var audioStreams: [Stream] { streams.filter(\.isAudio) }
    var subtitleStreams: [Stream] { streams.filter(\.isSubtitle) }
    var selectedAudio: Stream? { audioStreams.first(where: \.selected) ?? audioStreams.first }
    var selectedSubtitle: Stream? { subtitleStreams.first(where: \.selected) }
}

extension Stream {
    var hdrLabel: String? {
        if doviPresent { return "Dolby Vision" }
        if colorTrc == "smpte2084" { return "HDR10" }
        if colorTrc == "arib-std-b67" { return "HLG" }
        return nil
    }

    var isExternal: Bool { key != nil && index == nil }
}

extension Media {
    /// "4K · HEVC · Dolby Vision · TRUEHD 7.1"
    var label: String {
        var bits: [String] = []
        if let name = editionTitle ?? title { bits.append(name) }
        if let r = videoResolution {
            if r.lowercased() == "4k" { bits.append("4K") }
            else if r.lowercased() == "sd" { bits.append("SD") }
            else if Int(r) != nil { bits.append("\(r)p") }
            else { bits.append(r.uppercased()) }
        }
        if let v = videoCodec { bits.append(v.uppercased()) }
        if let hdr = parts.first?.streams.first(where: \.isVideo)?.hdrLabel { bits.append(hdr) }
        if let a = audioCodec {
            let ch = audioChannels.map(Fmt.channels) ?? ""
            bits.append("\(a.uppercased()) \(ch)".trimmingCharacters(in: .whitespaces))
        }
        return bits.joined(separator: " · ")
    }
}

/// Small formatting helpers, as in web/src/lib/format.ts.
enum Fmt {
    /// "2h 14m", "48m", "35s".
    static func duration(_ ms: Int) -> String {
        let secs = ms / 1000, h = secs / 3600, m = (secs % 3600) / 60
        if h == 0 && m == 0 { return "\(secs)s" }
        if h == 0 { return "\(m)m" }
        if m == 0 { return "\(h)h" }
        return "\(h)h \(m)m"
    }

    /// "1:02:03" / "4:05".
    static func clock(_ ms: Int) -> String {
        let secs = max(0, ms) / 1000, h = secs / 3600, m = (secs % 3600) / 60, s = secs % 60
        return h > 0 ? String(format: "%d:%02d:%02d", h, m, s) : String(format: "%d:%02d", m, s)
    }

    static func bytes<N: BinaryInteger>(_ n: N) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(n), countStyle: .binary)
    }

    static func bitrate(_ kbps: Int) -> String {
        kbps >= 1000 ? String(format: "%.1f Mbps", Double(kbps) / 1000) : "\(kbps) kbps"
    }

    private static let dateFormat: DateFormatter = {
        let f = DateFormatter()
        f.dateStyle = .medium
        f.timeStyle = .none
        return f
    }()

    /// "Mar 4, 2024" from a unix timestamp.
    static func date(_ ts: Int) -> String {
        dateFormat.string(from: Date(timeIntervalSince1970: TimeInterval(ts)))
    }

    /// "3 hr. ago", "yesterday", ...
    static func ago(_ ts: Int) -> String {
        let f = RelativeDateTimeFormatter()
        f.unitsStyle = .short
        return f.localizedString(for: Date(timeIntervalSince1970: TimeInterval(ts)), relativeTo: Date())
    }

    /// "2024-03-04" → "Mar 4, 2024".
    static func airDate(_ s: String) -> String {
        let p = DateFormatter()
        p.dateFormat = "yyyy-MM-dd"
        p.locale = Locale(identifier: "en_US_POSIX")
        return p.date(from: s).map { dateFormat.string(from: $0) } ?? s
    }

    static func initials(_ name: String) -> String {
        name.split(separator: " ").prefix(2).compactMap(\.first).map(String.init).joined().uppercased()
    }

    static func count(_ n: Int) -> String {
        n.formatted()
    }

    static func plural(_ n: Int, _ word: String, _ many: String? = nil) -> String {
        "\(count(n)) \(n == 1 ? word : (many ?? word + "s"))"
    }

    static func channels(_ n: Int) -> String {
        [1: "Mono", 2: "Stereo", 3: "2.1", 6: "5.1", 7: "6.1", 8: "7.1"][n] ?? "\(n)ch"
    }
}
