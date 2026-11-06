// Port of desktop/core/livetv.ts.
// Live TV & DVR: a channel guide, tuning, and recordings. The server's responses are loosely
// structured, so they are read defensively from JSON.

import Foundation
import PlexKit

public enum LiveTv {
    public nonisolated struct Program: Sendable, Equatable {
        public var title: String
        /// Episode title or year.
        public var subtitle: String?
        public var guid: String?
        public var summary: String?
        public var thumb: String?
        public var beginsAt: Int
        public var endsAt: Int
    }

    public nonisolated struct Channel: Sendable, Equatable {
        public var dvr: String
        public var id: String
        public var number: String?
        public var title: String
        public var thumb: String?
        public var programs: [Program]
    }

    public nonisolated struct Recording: Sendable, Equatable {
        public var id: String
        public var title: String
        public var subtitle: String?
        public var beginsAt: Int?
        public var channel: String?
        public var status: String?
    }

    public nonisolated struct Rule: Sendable, Equatable {
        public var id: String
        public var title: String
        public var kind: String?
    }

    public nonisolated struct Recordings: Sendable, Equatable {
        public var upcoming: [Recording]
        public var rules: [Rule]
    }

    private nonisolated static func get(_ v: JSONValue?, _ key: String) -> JSONValue? {
        guard case .object(let o)? = v else { return nil }
        return o[key]
    }

    /// `{Key: [...]}` or `{Key: {...}}` as a list.
    nonisolated static func list(_ v: JSONValue?, _ key: String) -> [JSONValue] {
        switch get(v, key) {
        case .array(let a)?: a
        case let x? where x.objectValue != nil: [x]
        default: []
        }
    }

    private nonisolated static func mc(_ v: JSONValue?) -> JSONValue? {
        get(v, "MediaContainer") ?? v
    }

    nonisolated static func text(_ v: JSONValue?, _ key: String) -> String? {
        switch get(v, key) {
        case .string(let s)? where !s.isEmpty: s
        case .number(let n)?: JSONValue.number(n).serialized()
        default: nil
        }
    }

    /// `Value::Number::as_i64` / `str::parse::<i64>`.
    nonisolated static func int(_ v: JSONValue?, _ key: String) -> Int? {
        switch get(v, key) {
        case .number(let n)?: n.rounded() == n ? Int(exactly: n) : nil
        case .string(let s)? where s.wholeMatch(of: #/[+-]?[0-9]+/#) != nil: Int(s)
        default: nil
        }
    }

    /// `str::parse::<f64>` for plain decimal numbers.
    private nonisolated static func parseF64(_ s: String) -> Double? {
        s.wholeMatch(of: #/[+-]?([0-9]+\.?[0-9]*|\.[0-9]+)([eE][+-]?[0-9]+)?/#) != nil ? Double(s) : nil
    }

    private nonisolated struct Dvr {
        var id: String
        var epg: String
        var lineup: String
        /// Channel keys enabled on the tuners; empty means all.
        var enabled: [String]
    }

    private static func dvrs(_ server: PlexServer) async throws -> [Dvr] {
        let v = try await server.dvrs()
        var out: [Dvr] = []
        for d in list(mc(v), "Dvr") {
            let enabled = list(d, "Device")
                .flatMap { list($0, "ChannelMapping") }
                .filter { m in
                    switch get(m, "enabled") {
                    case nil, .string("1")?, .number(1)?, .bool(true)?: true
                    default: false
                    }
                }
                .compactMap { text($0, "channelKey") }
            guard let id = text(d, "key"), let epg = text(d, "epgIdentifier"), let lineup = text(d, "lineup") else {
                continue
            }
            out.append(Dvr(id: id, epg: epg, lineup: lineup, enabled: enabled))
        }
        return out
    }

    public static func available(_ server: PlexServer) async -> Bool {
        ((try? await dvrs(server)) ?? []).count > 0
    }

    /// Channels with what is on from now for `hours`.
    public static func guide(_ server: PlexServer, _ hours: Int) async throws -> [Channel] {
        let now = Int(Date().timeIntervalSince1970)
        var out: [Channel] = []
        for dvr in try await dvrs(server) {
            let epg = "/" + dvr.epg.drop { $0 == "/" }
            let channels = try await server.lineupChannels(epg, lineup: dvr.lineup)
            let grid = try? await server.epgGrid(epg, begins: now, ends: now + hours * 3600)

            var programs: [String: [Program]] = [:]
            for p in list(mc(grid), "Metadata") {
                for media in list(p, "Media") {
                    guard let channel = text(media, "channelIdentifier") else { continue }
                    guard let beginsAt = int(media, "beginsAt"), let endsAt = int(media, "endsAt") else { continue }
                    let episode = get(p, "type") == .string("episode")
                    let entry = Program(
                        title: (episode ? (text(p, "grandparentTitle") ?? text(p, "title")) : text(p, "title")) ?? "",
                        subtitle: episode ? text(p, "title") : text(p, "year"), guid: text(p, "guid"),
                        summary: text(p, "summary"), thumb: text(p, "thumb") ?? text(p, "grandparentThumb"),
                        beginsAt: beginsAt, endsAt: endsAt)
                    programs[channel, default: []].append(entry)
                }
            }
            for c in list(mc(channels), "Channel") {
                guard let id = text(c, "identifier") else { continue }
                let key = text(c, "key")
                if !dvr.enabled.isEmpty, let key, !dvr.enabled.contains(key) { continue }
                var progs = programs.removeValue(forKey: id) ?? []
                // Stable sort by start, then drop consecutive duplicates of a start time.
                progs = progs.enumerated()
                    .sorted { ($0.element.beginsAt, $0.offset) < ($1.element.beginsAt, $1.offset) }
                    .map(\.element)
                progs = progs.enumerated().filter { i, p in i == 0 || p.beginsAt != progs[i - 1].beginsAt }.map(\.element)
                progs = progs.filter { $0.endsAt > now }
                out.append(
                    Channel(
                        dvr: dvr.id, id: id, number: text(c, "channelVcn") ?? text(c, "vcn"),
                        title: text(c, "title") ?? text(c, "callSign") ?? id, thumb: text(c, "thumb"), programs: progs))
            }
        }
        func number(_ c: Channel) -> Double {
            c.number.flatMap(parseF64) ?? Double.greatestFiniteMagnitude
        }
        return out.enumerated()
            .sorted { a, b in
                let (na, nb) = (number(a.element), number(b.element))
                if na != nb { return na < nb }
                if a.element.title != b.element.title { return a.element.title < b.element.title }
                return a.offset < b.offset
            }
            .map(\.element)
    }

    /// Find the first `Metadata` object anywhere in a response.
    private nonisolated static func firstMetadata(_ v: JSONValue) -> JSONValue? {
        switch v {
        case .array(let a):
            return a.lazy.compactMap(firstMetadata).first
        case .object(let o):
            if let m = o["Metadata"] {
                if case .array(let a) = m { return a.first }
                return m
            }
            return o.values.lazy.compactMap(firstMetadata).first
        default:
            return nil
        }
    }

    /// Tune a channel; returns the live item to hand to the player.
    public static func tune(_ server: PlexServer, _ dvr: String, _ channel: String) async throws -> Metadata {
        let v = try await server.tune(dvr, channel: channel)
        guard let m = firstMetadata(v), m != .null else { throw CoreError("The tuner did not start a session.") }
        let item = try Ops.Util.metadataFromJson(m)
        if item.key == nil { throw CoreError("The tuner did not start a session.") }
        return item
    }

    public static func recordings(_ server: PlexServer) async throws -> Recordings {
        async let scheduledResult = try? server.scheduledRecordings()
        let subscriptions = try await server.subscriptions()
        let scheduled = await scheduledResult
        var upcoming: [Recording] = []
        for g in list(mc(scheduled), "MediaGrabOperation") {
            guard let m = get(g, "Metadata"), m != .null else { continue }
            let media = list(m, "Media").first
            let episode = get(m, "type") == .string("episode")
            guard let id = text(g, "id") ?? text(g, "mediaSubscriptionID") else { continue }
            upcoming.append(
                Recording(
                    id: id, title: (episode ? text(m, "grandparentTitle") : text(m, "title")) ?? "",
                    subtitle: episode ? text(m, "title") : nil, beginsAt: media.flatMap { int($0, "beginsAt") },
                    channel: media.flatMap { text($0, "channelTitle") ?? text($0, "channelCallSign") },
                    status: text(g, "status")))
        }
        var rules: [Rule] = []
        for s in list(mc(subscriptions), "MediaSubscription") {
            let target = ["Directory", "Video", "Metadata"].lazy.compactMap { list(s, $0).first }.first
            guard let id = text(s, "key") else { continue }
            rules.append(Rule(id: id, title: text(s, "title") ?? target.flatMap { text($0, "title") } ?? "", kind: text(s, "type")))
        }
        return Recordings(upcoming: upcoming, rules: rules)
    }

    /// Record a guide item with the server's default rule for it.
    public static func record(_ server: PlexServer, _ guid: String) async throws {
        let v = try await server.subscriptionTemplate(guid)
        guard let template = list(mc(v), "SubscriptionTemplate").flatMap({ list($0, "MediaSubscription") }).first else {
            throw CoreError("This program cannot be recorded.")
        }
        var p = Params().set("includeGrabs", 1)
        if let params = text(template, "parameters") {
            for (k, val) in formDecode(params) { p.push(k, val) }
        }
        for key in ["type", "targetLibrarySectionID", "targetSectionLocationID"] {
            if let val = text(template, key) { p.push(key, val) }
        }
        for s in list(template, "Setting") {
            if let id = text(s, "id"), let value = text(s, "value") ?? text(s, "default") { p.push("prefs[\(id)]", value) }
        }
        _ = try await server.createSubscription(p)
    }

    /// `new URLSearchParams(text)`: `+` is a space, `%XX` a byte.
    private nonisolated static func formDecode(_ text: String) -> [(String, String)] {
        func decode(_ s: Substring) -> String {
            let spaced = s.replacingOccurrences(of: "+", with: " ")
            return spaced.removingPercentEncoding ?? spaced
        }
        let query = text.hasPrefix("?") ? text.dropFirst() : Substring(text)
        return query.split(separator: "&").map { pair in
            guard let eq = pair.firstIndex(of: "=") else { return (decode(pair), "") }
            return (decode(pair[..<eq]), decode(pair[pair.index(after: eq)...]))
        }
    }

    /// Delete a recording rule by its id or key.
    public static func deleteRule(_ server: PlexServer, _ id: String) async throws {
        try await server.deleteSubscription(Ops.Lists.lastSegment(id))
    }
}
