// Port of desktop/core/ops/lists.ts.
// Watchlist, playlists and watch history.

import Foundation
import PlexKit

extension Ops {
    public enum Lists {
        /// The plex.tv watchlist, up to 1000 items.
        public static func watchlist(_ app: AppState) async throws -> [Metadata] {
            // Discover caps the page size at an undocumented value that has shrunk
            // before (even 50 was refused at times), so page and step down when refused.
            let tv = try app.plexTv()
            let sizes = [50, 20, 10]
            var at = 0
            var size = sizes[at]
            var items: [Metadata] = []
            while true {
                let page: MediaContainer
                do {
                    page = try await tv.watchlist(items.count, size: size)
                } catch {
                    if errorText(error).contains("container-size") && at + 1 < sizes.count {
                        at += 1
                        size = sizes[at]
                        continue
                    }
                    throw error
                }
                let total = page.totalSize
                let got = page.metadata.count
                items.append(contentsOf: page.metadata)
                if got < size || (total.map { items.count >= $0 } ?? false) || items.count >= 1000 { break }
            }
            return items
        }

        public static func watchlisted(_ app: AppState, _ discoverId: String) async throws -> Bool {
            try await app.plexTv().userState(discoverId)?.watchlistedAt != nil
        }

        public nonisolated struct PlaylistView: Sendable, Equatable {
            public var playlist: Metadata
            public var items: [Metadata]
            /// How many entries the server says the playlist has; more than `items.count` only
            /// when the safety limit cut loading short.
            public var total: Int
        }

        /// Entries requested per page of a playlist.
        public nonisolated static let PLAYLIST_PAGE = 1000
        /// A runaway guard: no front end loads more entries than this.
        public nonisolated static let PLAYLIST_LIMIT = 100_000

        public static func playlist(_ app: AppState, _ id: String) async throws -> PlaylistView? {
            let server = try app.server()
            guard let found = try await server.playlist(id) else { return nil }
            let (items, total) = try await playlistEntries(server, id)
            return PlaylistView(playlist: found, items: items, total: total)
        }

        /// Every entry of a playlist (paging through the server's limit) and the total the
        /// server reports.
        public static func playlistEntries(_ server: PlexServer, _ id: String) async throws -> ([Metadata], Int) {
            try await collectPages(PLAYLIST_PAGE, PLAYLIST_LIMIT) { start, size in
                let mc = try await server.playlistItems(id, start: start, size: size)
                return (mc.metadata, (mc.totalSize ?? mc.size).map { max($0, 0) })
            }
        }

        /// Fetch consecutive pages until the reported total, a short page, or `limit` entries.
        /// `fetch(start, size)` returns a page and the total the server reports (when it does).
        /// The returned total is what the server said, or what was loaded when it said nothing.
        public static func collectPages<T>(
            _ size: Int, _ limit: Int, _ fetch: (Int, Int) async throws -> ([T], Int?)
        ) async throws -> ([T], Int) {
            var items: [T] = []
            var total: Int?
            while true {
                let (page, reported) = try await fetch(items.count, size)
                total = reported ?? total
                let got = page.count
                items.append(contentsOf: page)
                // Without a total a short page means the end; with one keep going
                // until it is reached, since servers may cap pages below `size`.
                let done = got == 0 || (total.map { items.count >= $0 } ?? (got < size)) || items.count >= limit
                if done {
                    if items.count > limit { items.removeLast(items.count - limit) }
                    return (items, total ?? items.count)
                }
            }
        }

        /// Add an item to an existing playlist, or to a new one called `title` of `kind`.
        public static func addToPlaylist(
            _ app: AppState, _ ratingKey: String, _ playlist: String?, _ title: String?, _ kind: String = "video"
        ) async throws {
            let server = try app.server()
            let uri = server.itemUri([ratingKey])
            if let playlist {
                try await server.addToPlaylist(playlist, uri: uri)
            } else if let title {
                _ = try await server.createPlaylist(title, kind: kind, uri: uri, smart: false)
            } else {
                throw CoreError("No playlist given.")
            }
        }

        public nonisolated struct HistoryPage: Sendable, Equatable {
            public var items: [Metadata]
            public var hasMore: Bool
            /// Account names by id (owners only).
            public var accounts: [Int: String]
        }

        public nonisolated static let HISTORY_PAGE = 60

        /// Page `page` of the server's watch history, newest first.
        public static func history(_ app: AppState, _ page: Int) async throws -> HistoryPage {
            let server = try app.server()
            let owned = app.session()?.owned ?? false
            let mc = try await server.history(HistoryQuery(start: page * HISTORY_PAGE, size: HISTORY_PAGE))
            var accounts: [Int: String] = [:]
            if owned {
                for a in (try? await server.accounts()) ?? [] {
                    if let id = a.id, let name = a.name { accounts[id] = name }
                }
            }
            let hasMore = MediaContainer.hasMore(mc) || mc.metadata.count == HISTORY_PAGE
            return HistoryPage(items: mc.metadata, hasMore: hasMore, accounts: accounts)
        }

        /// Delete a history entry by its `historyKey`.
        public static func deleteHistory(_ app: AppState, _ key: String) async throws {
            try await app.server().deleteHistory(lastSegment(key))
        }

        /// The id at the end of a key like `/status/sessions/history/12`.
        public nonisolated static func lastSegment(_ key: String) -> String {
            key.lastIndex(of: "/").map { String(key[key.index(after: $0)...]) } ?? key
        }
    }
}
