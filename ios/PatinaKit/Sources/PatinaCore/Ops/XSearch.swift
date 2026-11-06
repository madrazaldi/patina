// Port of desktop/core/ops/xsearch.ts.
// Search across every server of the signed-in account.
//
// Each server is searched on its own connection, a few at a time and with a timeout, so a
// slow or unreachable server never holds the others back. Results arrive as a growing
// snapshot (`Results`): after every server that finishes, the same title found on several
// servers is shown once, listing each copy with its quality. Items found on a server other
// than the current one are tagged with it (see `Metadata.setServerId`), which keeps
// navigation, playback and reporting on the server that has the copy.

import Foundation
import PlexKit

nonisolated private let log = logger("xsearch")

extension Ops {
    public enum XSearch {
        /// Servers searched at once.
        public nonisolated static let CONCURRENCY = 4
        /// How long one server gets to connect and answer before it is given up on.
        public nonisolated static let PER_SERVER: Duration = .seconds(12)
        nonisolated static let HUB_LIMIT = 24
        nonisolated static let PER_SERVER_ITEMS = 40

        /// One file of a copy: what a person compares when choosing where to watch.
        public nonisolated struct Version: Sendable, Equatable {
            /// "1080p · H264 · MKV · 6.2 Mbps · 2.1 GB".
            public var label: String
            public var resolution: String?
            public var videoCodec: String?
            public var container: String?
            /// Kilobits per second.
            public var bitrate: Int?
            /// Bytes.
            public var size: Int?
            public var hdr: String?
        }

        /// A title as one server has it.
        public nonisolated struct ServerCopy: Sendable, Equatable {
            public var serverId: String
            public var serverName: String
            /// On the server this session is connected to.
            public var current: Bool
            public var library: String?
            /// The item, tagged with its server unless it is the current one.
            public var item: Metadata
            public var versions: [Version]
        }

        /// One title and every server that has it.
        public nonisolated struct Group: Sendable, Equatable {
            public var key: String
            public var title: String
            public var year: Int?
            public var kind: String
            /// The show and episode number for an episode, the artist for an album, ...
            public var subtitle: String?
            public var copies: [ServerCopy]
        }

        public nonisolated enum ServerState: String, Sendable, Equatable {
            case pending, done, failed, timedOut, offline
        }

        public nonisolated struct ServerStatus: Sendable, Equatable {
            public var id: String
            public var name: String
            public var current: Bool
            public var state: ServerState
            /// Titles this server contributed.
            public var count: Int
            public var error: String?
        }

        /// A server to search.
        public nonisolated struct Target: Sendable, Equatable {
            public var id: String
            public var name: String
            public var current: Bool
            public var online: Bool

            public init(id: String, name: String, current: Bool, online: Bool) {
                self.id = id
                self.name = name
                self.current = current
                self.online = online
            }
        }

        /// The current server first, then the account's other servers.
        public static func targets(_ app: AppState) -> [Target] {
            guard let session = app.session() else { return [] }
            var out = [Target(id: session.serverId, name: session.serverName, current: true, online: true)]
            for r in session.servers where Resource.id(r) != session.serverId && Resource.isServer(r) {
                out.append(Target(id: Resource.id(r), name: Resource.name(r), current: false, online: r.presence))
            }
            return out
        }

        /// Everything found so far.
        public nonisolated struct Results: Sendable, Equatable {
            public var query: String
            public var groups: [Group] = []
            public var servers: [ServerStatus]
            /// Every server has answered, failed or timed out.
            public var done = false

            public init(_ query: String, _ targets: [Target]) {
                self.query = query
                servers = targets.map {
                    ServerStatus(
                        id: $0.id, name: $0.name, current: $0.current, state: $0.online ? .pending : .offline, count: 0,
                        error: nil)
                }
            }

            /// Fold in what one server found.
            public mutating func add(_ target: Target, _ items: [Metadata]) {
                for var item in items {
                    Metadata.setServerId(&item, target.current ? nil : target.id)
                    let copy = ServerCopy(
                        serverId: target.id, serverName: target.name, current: target.current,
                        library: item.librarySectionTitle, item: item, versions: item.media.map(version))
                    insert(&groups, copy)
                }
                finish(target, .done, items.count, nil)
            }

            /// Note that a server could not be searched.
            public mutating func fail(_ target: Target, _ state: ServerState, _ error: String) {
                finish(target, state, 0, error)
            }

            private mutating func finish(_ target: Target, _ state: ServerState, _ count: Int, _ error: String?) {
                if let at = servers.firstIndex(where: { $0.id == target.id }) {
                    servers[at].state = state
                    servers[at].count = count
                    servers[at].error = error
                }
                done = servers.allSatisfy { $0.state != .pending }
            }
        }

        /// Put a copy into the group of its title, starting a new group when it is the first.
        /// The current server's copy is listed first, then the others by server name.
        private nonisolated static func insert(_ groups: inout [Group], _ copy: ServerCopy) {
            let key = groupKey(copy.item)
            guard let at = groups.firstIndex(where: { $0.key == key }) else {
                groups.append(
                    Group(
                        key: key, title: Metadata.title(copy.item), year: copy.item.year, kind: Metadata.kind(copy.item),
                        subtitle: subtitle(copy.item), copies: [copy]))
                return
            }
            // A server sometimes lists the same item under more than one hub.
            if groups[at].copies.contains(where: {
                $0.serverId == copy.serverId && Metadata.ratingKey($0.item) == Metadata.ratingKey(copy.item)
            }) {
                return
            }
            groups[at].copies.append(copy)
            groups[at].copies = groups[at].copies.enumerated()
                .sorted { a, b in
                    if a.element.current != b.element.current { return a.element.current }
                    let (an, bn) = (a.element.serverName.lowercased(), b.element.serverName.lowercased())
                    return an != bn ? an < bn : a.offset < b.offset
                }
                .map(\.element)
        }

        private nonisolated static func pad2(_ n: Int) -> String {
            n < 10 && n >= 0 ? "0\(n)" : String(n)
        }

        /// The show and episode number for an episode, the artist for an album.
        nonisolated static func subtitle(_ m: Metadata) -> String? {
            switch Metadata.kind(m) {
            case "episode":
                let show = m.grandparentTitle ?? ""
                let number = m.parentIndex.flatMap { s in m.index.map { "S\(pad2(s))E\(pad2($0))" } } ?? ""
                let text = "\(show) \(number)".trimmingCharacters(in: .whitespacesAndNewlines)
                return text.isEmpty ? nil : text
            case "album", "track":
                return m.parentTitle ?? m.grandparentTitle
            default:
                return nil
            }
        }

        /// What identifies a title across servers: its Plex guid, or without one (local or
        /// legacy matches) the kind, title and year.
        public nonisolated static func groupKey(_ m: Metadata) -> String {
            if let guid = m.guid, !guid.isEmpty, !guid.hasPrefix("local://"), !guid.contains("agents.none") { return guid }
            func norm(_ s: String) -> String {
                let kept = s.unicodeScalars.filter { $0.properties.isAlphabetic || $0.properties.numericType != nil }
                return String(String.UnicodeScalarView(kept)).lowercased()
            }
            let kind = Metadata.kind(m)
            if kind == "episode" {
                return "episode|\(norm(m.grandparentTitle ?? ""))|\(m.parentIndex ?? -1)|\(m.index ?? -1)"
            }
            return "\(kind)|\(norm(Metadata.title(m)))|\(m.year ?? 0)"
        }

        /// A short description of a file: resolution, codec, container, bitrate and size.
        public nonisolated static func version(_ media: Media) -> Version {
            var resolution: String?
            if let r = media.videoResolution {
                if r == "4k" || r == "4K" {
                    resolution = "4K"
                } else if r == "sd" || r == "SD" {
                    resolution = "SD"
                } else if r.allSatisfy({ $0.isASCII && $0.isNumber }) {
                    resolution = "\(r)p"
                } else {
                    resolution = r.uppercased()
                }
            }
            let part = media.part.first
            let container = (media.container ?? part?.container)?.uppercased()
            let videoCodec = media.videoCodec?.uppercased()
            let hdr = part.flatMap(Part.videoStream).flatMap(MediaStream.hdrLabel)
            let size = part?.size
            var bits: [String] = []
            if let edition = media.editionTitle ?? media.title { bits.append(edition) }
            bits += [resolution, videoCodec, hdr, container].compactMap { $0 }
            if let bitrate = media.bitrate, bitrate > 0 {
                bits.append(bitrate >= 1000 ? "\(fixed(Double(bitrate) / 1000, 1)) Mbps" : "\(bitrate) kbps")
            }
            if let size, size > 0 { bits.append(sizeLabel(size)) }
            let label = bits.isEmpty ? "Unknown quality" : bits.joined(separator: " · ")
            return Version(
                label: label, resolution: resolution, videoCodec: videoCodec, container: container, bitrate: media.bitrate,
                size: size, hdr: hdr)
        }

        /// Rust's `{:.N}`: exact decimal rounding with ties to even, which `printf` does too.
        private nonisolated static func fixed(_ x: Double, _ digits: Int) -> String {
            String(format: "%.\(digits)f", x)
        }

        private nonisolated static func sizeLabel(_ bytes: Int) -> String {
            let GB = 1024.0 * 1024 * 1024
            let MB = 1024.0 * 1024
            return Double(bytes) >= GB ? "\(fixed(Double(bytes) / GB, 1)) GB" : "\(fixed(Double(bytes) / MB, 0)) MB"
        }

        /// Search one server; the current one when `target.current`.
        private static func searchOne(_ app: AppState, _ target: Target, _ query: String) async throws -> [Metadata] {
            let server = try await Servers.serverFor(app, target.current ? nil : target.id)
            let hubs = try await server.searchHubs(query, limit: HUB_LIMIT)
            var seen = Set<String>()
            var out: [Metadata] = []
            for m in hubs.flatMap(\.metadata) {
                if out.count >= PER_SERVER_ITEMS { break }
                if Metadata.kind(m) == "collection" { continue }
                if !seen.insert("\(Metadata.kind(m))\u{0}\(Metadata.ratingKey(m))").inserted { continue }
                out.append(m)
            }
            return out
        }

        private enum Outcome: Sendable {
            case items([Metadata])
            case error(String)
            case timeout
        }

        /// Search every server of the account. `onUpdate` sees the results after each server
        /// that finishes (and once at the start), so a front end can show them as they come.
        @discardableResult
        public static func searchAll(_ app: AppState, _ query: String, _ onUpdate: (Results) -> Void = { _ in }) async
            -> Results
        {
            await searchAllWithin(app, query, PER_SERVER, onUpdate)
        }

        /// `searchAll` with its own limit on how long one server may take.
        @discardableResult
        public static func searchAllWithin(
            _ app: AppState, _ query: String, _ perServer: Duration, _ onUpdate: (Results) -> Void = { _ in }
        ) async -> Results {
            let all = targets(app)
            var results = Results(query, all)
            onUpdate(results)
            if results.servers.allSatisfy({ $0.state != .pending }) {
                results.done = true
                onUpdate(results)
                return results
            }
            // The work never throws: each server's failure is an outcome.
            try? await Util.eachCompleted(all.filter(\.online), CONCURRENCY) { target -> Outcome in
                // `tokio::time::timeout`: the search itself is left to finish on its own.
                let outcome = await Util.within(perServer) { () -> Outcome in
                    do {
                        return .items(try await searchOne(app, target, query))
                    } catch {
                        return .error(errorText(error))
                    }
                }
                return outcome ?? .timeout
            } _: { outcome, target in
                switch outcome {
                case .items(let items):
                    results.add(target, items)
                case .error(let error):
                    log.debug("searching \(target.name) failed: \(error)")
                    if !target.current { Servers.forget(app, target.id) }
                    results.fail(target, .failed, error)
                case .timeout:
                    if !target.current { Servers.forget(app, target.id) }
                    results.fail(target, .timedOut, "Took too long to answer.")
                }
                onUpdate(results)
            }
            results.done = true
            onUpdate(results)
            return results
        }
    }
}
