// Port of desktop/core/lyrics.ts.
// Track lyrics from Plex lyric streams (stream type 4): LRC files become timed lines, plain
// text stays untimed.

import Foundation
import PlexKit

public enum Lyrics {
    public nonisolated struct Line: Sendable, Equatable {
        public var timeMs: Int?
        public var text: String
    }

    public nonisolated struct Lyrics: Sendable, Equatable {
        public var synced: Bool
        public var lines: [Line]
    }

    public static func load(_ server: PlexServer, _ ratingKey: String) async throws -> Lyrics? {
        guard let item = try await server.metadata(ratingKey) else { return nil }
        let key = item.media.flatMap(\.part).flatMap(\.stream).first {
            $0.streamType == StreamType.lyrics && $0.key != nil
        }?.key
        guard let key else { return nil }
        let (bytes, _) = try await server.bytes(key, Params())
        let lyrics = parse(String(decoding: bytes, as: UTF8.self))
        return lyrics.lines.isEmpty ? nil : lyrics
    }

    /// Rust's `str::lines`: split on `\n`, drop a trailing `\r`, no empty final line.
    nonisolated static func lines(_ text: String) -> [String] {
        var out = text.split(separator: "\n", omittingEmptySubsequences: false).map { l in
            l.hasSuffix("\r") ? String(l.dropLast()) : String(l)
        }
        if out.last == "" { out.removeLast() }
        return out
    }

    /// Parse LRC (`[mm:ss.xx] text`, several stamps per line allowed) or plain text.
    public nonisolated static func parse(_ text: String) -> Lyrics {
        var timed: [Line] = []
        var plain: [Line] = []
        for raw in lines(text) {
            var rest = Substring(raw.trimmingCharacters(in: .whitespaces))
            var stamps: [Int] = []
            while rest.hasPrefix("[") {
                let body = rest.dropFirst()
                guard let end = body.firstIndex(of: "]") else { break }
                let tag = body[..<end]
                rest = body[body.index(after: end)...].drop { $0.isWhitespace }
                if let ms = stamp(String(tag)) { stamps.append(ms) }
                // Other tags ([ar:], [ti:], [offset:]) are metadata; drop them.
            }
            if stamps.isEmpty {
                if !raw.drop(while: \.isWhitespace).hasPrefix("[") {
                    plain.append(Line(timeMs: nil, text: raw.trimmingCharacters(in: .whitespaces)))
                }
            } else {
                for ms in stamps { timed.append(Line(timeMs: ms, text: String(rest))) }
            }
        }
        if timed.isEmpty {
            while plain.last?.text == "" { plain.removeLast() }
            return Lyrics(synced: false, lines: plain)
        }
        // Stable sort by time, like `sort_by_key`.
        let sorted = timed.enumerated().sorted { ($0.element.timeMs!, $0.offset) < ($1.element.timeMs!, $1.offset) }
        return Lyrics(synced: true, lines: sorted.map(\.element))
    }

    nonisolated static func stamp(_ tag: String) -> Int? {
        guard let i = tag.firstIndex(of: ":") else { return nil }
        let m = tag[..<i].trimmingCharacters(in: .whitespaces)
        let s = tag[tag.index(after: i)...].trimmingCharacters(in: .whitespaces).replacing(",", with: ".", maxReplacements: 1)
        // Rust's integer / f64 parsing is strict: no stray characters.
        guard m.wholeMatch(of: /[+-]?\d+/) != nil, s.wholeMatch(of: /[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?/) != nil,
            let minutes = Int(m), let seconds = Double(s)
        else { return nil }
        return minutes * 60_000 + Int((seconds * 1000).rounded())
    }
}
