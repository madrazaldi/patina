// Port of desktop/core/lyrics.ts.
// Track lyrics from Plex lyric streams (stream type 4): LRC files become timed lines, plain
// text stays untimed. The module's functions are statics of its `Lyrics` result, as with
// `Session`.

import Foundation
import PlexKit

public nonisolated struct Lyrics: Sendable, Equatable {
    public struct Line: Sendable, Equatable {
        public var timeMs: Int?
        public var text: String

        public init(timeMs: Int?, text: String) {
            self.timeMs = timeMs
            self.text = text
        }
    }

    public var synced: Bool
    public var lines: [Line]

    public init(synced: Bool, lines: [Line]) {
        self.synced = synced
        self.lines = lines
    }

    public static func load(_ server: PlexServer, _ ratingKey: String) async throws -> Lyrics? {
        guard let item = try await server.metadata(ratingKey) else { return nil }
        let key = item.media.lazy.flatMap(\.part).flatMap(\.stream)
            .first { $0.streamType == StreamType.lyrics && $0.key != nil }?.key
        guard let key else { return nil }
        let (bytes, _) = try await server.bytes(key, Params())
        let lyrics = parse(String(decoding: bytes, as: UTF8.self))
        return lyrics.lines.isEmpty ? nil : lyrics
    }

    /// Rust's `str::lines`: split on `\n`, drop a trailing `\r`, no empty final line.
    private static func lines(_ text: String) -> [String] {
        // By scalars: "\r\n" is one Character, so splitting Characters would miss it.
        var out = text.unicodeScalars.split(separator: "\n", omittingEmptySubsequences: false).map { line in
            String(String.UnicodeScalarView(line.last == "\r" ? line.dropLast() : line))
        }
        if out.last == "" { out.removeLast() }
        return out
    }

    private static func trim(_ s: some StringProtocol) -> String {
        s.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private static func trimStart(_ s: some StringProtocol) -> String {
        String(s.drop { $0.isWhitespace })
    }

    /// Parse LRC (`[mm:ss.xx] text`, several stamps per line allowed) or plain text.
    public static func parse(_ text: String) -> Lyrics {
        var timed: [Line] = []
        var plain: [Line] = []
        for raw in lines(text) {
            var rest = trim(raw)
            var stamps: [Int] = []
            while rest.hasPrefix("[") {
                let body = rest.dropFirst()
                guard let end = body.firstIndex(of: "]") else { break }
                let tag = body[..<end]
                rest = trimStart(body[body.index(after: end)...])
                if let ms = stamp(String(tag)) { stamps.append(ms) }
                // Other tags ([ar:], [ti:], [offset:]) are metadata; drop them.
            }
            if stamps.isEmpty {
                if !trimStart(raw).hasPrefix("[") { plain.append(Line(timeMs: nil, text: trim(raw))) }
            } else {
                for ms in stamps { timed.append(Line(timeMs: ms, text: rest)) }
            }
        }
        if timed.isEmpty {
            while plain.last?.text == "" { plain.removeLast() }
            return Lyrics(synced: false, lines: plain)
        }
        // Stable sort by time, like `sort_by_key`.
        let sorted = timed.enumerated()
            .sorted { ($0.element.timeMs!, $0.offset) < ($1.element.timeMs!, $1.offset) }
            .map(\.element)
        return Lyrics(synced: true, lines: sorted)
    }

    private static func stamp(_ tag: String) -> Int? {
        guard let i = tag.firstIndex(of: ":") else { return nil }
        let m = trim(tag[..<i])
        let s = trim(tag[tag.index(after: i)...]).replacing(",", with: ".", maxReplacements: 1)
        // Rust's integer / f64 parsing is strict: no stray characters.
        guard m.wholeMatch(of: #/[+-]?[0-9]+/#) != nil,
            s.wholeMatch(of: #/[+-]?([0-9]+\.?[0-9]*|\.[0-9]+)([eE][+-]?[0-9]+)?/#) != nil,
            let minutes = Int(m), let seconds = Double(s)
        else { return nil }
        // `Math.round`: halves round up.
        return minutes * 60_000 + Int((seconds * 1000 + 0.5).rounded(.down))
    }
}
