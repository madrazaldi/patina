// Port of desktop/core/ops/library.ts.
// Library browsing beyond plain listings: filters, folders, related hubs.

import Foundation
import PlexKit

extension Ops {
    public enum Library {
        public nonisolated struct FilterView: Sendable, Equatable {
            public var filter: String
            public var title: String
            /// `string`, `integer`, `tag` or `boolean`.
            public var kind: String
        }

        /// Filters for a section and media type (genre, year, resolution, unwatched, ...).
        public static func sectionFilters(_ app: AppState, _ id: String, _ kind: Int? = nil) async throws -> [FilterView] {
            let server = try app.server()
            func views(_ list: [Directory]) -> [FilterView] {
                list.compactMap { d in d.filter.map { FilterView(filter: $0, title: d.title ?? "", kind: d.filterType ?? "string") } }
            }
            if let meta = try await server.sectionMeta(id) {
                let found = meta.type.first { t in t.type.flatMap(Util.mediaTypeId) == kind } ?? meta.type.first
                if let found { return views(found.filter) }
            }
            return views(try await server.sectionFilters(id))
        }

        public nonisolated struct FolderView: Sendable, Equatable {
            public var title: String?
            public var directories: [Directory]
            public var items: [Metadata]
        }

        /// A section's folder tree: the root, or the folder at `key`.
        public static func sectionFolders(_ app: AppState, _ id: String, _ key: String? = nil) async throws -> FolderView {
            let mc = try await app.server().sectionFolders(id, parent: key)
            return FolderView(title: mc.title2 ?? mc.title1, directories: mc.directory, items: mc.metadata)
        }

        /// Extra hubs for a detail page, plus sonically similar music.
        public static func itemHubs(_ app: AppState, _ ratingKey: String, _ kind: String) async throws -> [Hub] {
            try await itemHubsOn(app.server(), ratingKey, kind)
        }

        /// `itemHubs` on any server (an item found on another server of the account).
        public static func itemHubsOn(_ server: PlexServer, _ ratingKey: String, _ kind: String) async -> [Hub] {
            var hubs = (try? await server.metadataHubs(ratingKey, count: 12)) ?? []
            var similar: (String, [Metadata])?
            if kind == "track" || kind == "album" {
                similar = (try? await server.nearest(ratingKey)).map { ("Sonically similar", $0) }
            } else if kind == "artist" {
                similar = (try? await server.similar(ratingKey)).map { ("Similar artists", $0) }
            }
            if let (title, items) = similar, !items.isEmpty {
                hubs.append(
                    Hub(title: title, hubIdentifier: "patina.\(kind).similar", metadata: items.filter { $0.ratingKey != ratingKey }))
            }
            return hubs
        }
    }
}
