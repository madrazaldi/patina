// Port of desktop/core/offline.ts.
// Local resume points and an account/server-scoped outbox for offline watches.

import CryptoKit
import Foundation
import PlexKit

public enum Offline {
    /// One entry of `offline-progress.json` (snake_case on disk, like the Rust struct).
    public nonisolated struct Progress: Codable, Sendable, Equatable {
        public var id: String
        public var account: String
        public var server: String
        public var ratingKey: String
        public var position: Int
        public var duration: Int
        public var watched: Bool
        public var updated: Int
        public var revision: String
        public var pending: Bool

        enum CodingKeys: String, CodingKey {
            case id, account, server
            case ratingKey = "rating_key"
            case position, duration, watched, updated, revision, pending
        }

        public init(
            id: String, account: String, server: String, ratingKey: String, position: Int, duration: Int,
            watched: Bool, updated: Int, revision: String, pending: Bool
        ) {
            self.id = id
            self.account = account
            self.server = server
            self.ratingKey = ratingKey
            self.position = position
            self.duration = duration
            self.watched = watched
            self.updated = updated
            self.revision = revision
            self.pending = pending
        }
    }

    /// Unix time in seconds.
    public nonisolated static func now() -> Int {
        Int(Date().timeIntervalSince1970)
    }

    public nonisolated static func digest(_ value: String) -> String {
        SHA256.hash(data: Data(value.utf8)).map { String(format: "%02x", $0) }.joined()
    }

    public nonisolated static func account(_ session: Session) -> String {
        session.user?.uuid ?? digest(session.server.token)
    }

    private static func path(_ paths: Paths) -> URL {
        paths.config.appendingPathComponent("offline-progress.json")
    }

    /// The resume position of a download: the local one when there is one, else the server's.
    public static func position(_ paths: Paths, _ download: Download) -> Int {
        if let p = readList(paths).first(where: { $0.id == download.id }) { return p.watched ? 0 : p.position }
        return download.item.viewOffset ?? 0
    }

    private static func readList(_ paths: Paths) -> [Progress] {
        (try? Storage.readJson(path(paths)) { [Progress]() }) ?? []
    }

    /// Downloads finished offline whose watch isn't on the server yet.
    public static func watchedOffline(_ paths: Paths) -> Set<String> {
        Set(readList(paths).filter { $0.watched && $0.pending }.map(\.id))
    }

    public static func record(
        _ paths: Paths, _ download: Download, _ positionMs: Int, _ duration: Int, _ completed: Bool, _ threshold: Double
    ) throws {
        guard duration > 0 else { return }
        let watched = completed || Double(positionMs) / Double(duration) >= threshold
        let progress = Progress(
            id: download.id, account: download.account, server: download.serverId,
            ratingKey: Metadata.ratingKey(download.item), position: watched ? 0 : max(positionMs, 0),
            duration: duration, watched: watched, updated: now(), revision: UUID().uuidString.lowercased(),
            pending: download.account != "")
        try Storage.updateJson(path(paths), { [Progress]() }) { list in
            list.removeAll { $0.id == progress.id }
            list.append(progress)
        }
    }

    public static func belongs(_ p: Progress, _ server: String, _ account: String) -> Bool {
        p.pending && p.server == server && p.account == account
    }

    public static func acknowledge(_ list: inout [Progress], _ sent: Progress) {
        // A playback checkpoint written while the request was in flight stays pending.
        if let i = list.firstIndex(where: { $0.id == sent.id && $0.revision == sent.revision }) {
            list[i].pending = false
        }
    }

    public static func sync(_ app: AppState) async throws {
        guard let session = app.session() else { return }
        // Wait until local playback ends so partial checkpoints don't race its final watch state.
        if app.nowPlaying()?.local == true { return }
        guard let lease = Storage.tryLock(app.paths.config.appendingPathComponent("offline-sync")) else { return }
        defer { lease.release() }
        let file = path(app.paths)
        let acct = account(session)
        let list = try Storage.readJson(file) { [Progress]() }
        var firstError: (any Error)?
        func stillCurrent() -> Bool {
            guard let s = app.session() else { return false }
            return sameServer(s.server, session.server)
        }
        for p in list where belongs(p, session.serverId, acct) {
            if !stillCurrent() { break }
            let item: Metadata?
            do {
                item = try await session.server.metadata(p.ratingKey)
            } catch let e as PlexError where e.status == 404 || e.status == 410 {
                item = nil
            } catch {
                if firstError == nil { firstError = error }
                continue
            }
            guard let item else {
                // Deleted titles keep their local resume point, but must not block the outbox.
                try Storage.updateJson(file, { [Progress]() }) { acknowledge(&$0, p) }
                continue
            }
            // A newer online watch wins; replay must never rewind another device's newer progress.
            let serverIsNewer = (item.lastViewedAt ?? 0) > p.updated
            let serverPosition = item.viewOffset ?? 0
            let serverWatched = Metadata.isWatched(item)
            if !serverIsNewer {
                do {
                    if p.watched {
                        try await session.server.scrobble(p.ratingKey)
                    } else {
                        try await session.server.timeline(
                            TimelineUpdate(
                                ratingKey: p.ratingKey, key: item.key ?? "/library/metadata/\(p.ratingKey)",
                                state: .stopped, timeMs: p.position, durationMs: p.duration))
                    }
                } catch {
                    if firstError == nil { firstError = error }
                    continue
                }
            }
            try Storage.updateJson(file, { [Progress]() }) { l in
                acknowledge(&l, p)
                if serverIsNewer, let i = l.firstIndex(where: { $0.id == p.id && $0.revision == p.revision }) {
                    l[i].position = serverPosition
                    l[i].watched = serverWatched
                }
            }
            app.bump()
        }
        if let firstError { throw firstError }
    }
}
