// Port of desktop/core/ops/edit.ts.
// Editing items: fields, tags and collections.

import Foundation
import PlexKit

extension Ops {
    public enum Edit {
        public nonisolated struct TagEdit: Sendable, Equatable {
            /// `genre`, `label`, `collection`, `mood`, ...
            public var tag: String
            public var add: [String]
            public var remove: [String]

            public init(tag: String, add: [String], remove: [String]) {
                self.tag = tag
                self.add = add
                self.remove = remove
            }
        }

        /// Edit fields (locking them so refreshes keep the change) and tags.
        public static func editMetadata(_ app: AppState, _ ratingKey: String, _ fields: [(String, String)], _ tags: [TagEdit])
            async throws
        {
            let server = try app.server()
            guard let item = try await server.metadata(ratingKey) else { throw CoreError("Item not found.") }
            guard let kind = Util.mediaTypeId(item.type ?? "") else { throw CoreError("This item cannot be edited.") }
            if !fields.isEmpty {
                try await server.editMetadata(ratingKey, sectionId: item.librarySectionID, kind: kind, fields: fields, lock: true)
            }
            for t in tags where !t.add.isEmpty || !t.remove.isEmpty {
                try await server.editTags(ratingKey, kind: kind, tag: t.tag, add: t.add, remove: t.remove)
            }
        }

        /// Add an item to a collection, or to a new one called `title`.
        public static func addToCollection(_ app: AppState, _ ratingKey: String, _ collection: String?, _ title: String?)
            async throws
        {
            let server = try app.server()
            let uri = server.itemUri([ratingKey])
            if let collection {
                try await server.addToCollection(collection, uri: uri)
            } else if let title {
                guard let item = try await server.metadata(ratingKey) else { throw CoreError("Item not found.") }
                guard let section = item.librarySectionID else { throw CoreError("The item is not in a library.") }
                let kind = Util.mediaTypeId(item.type ?? "") ?? 1
                _ = try await server.createCollection(String(section), title: title, kind: kind, uri: uri, smart: false)
            } else {
                throw CoreError("No collection given.")
            }
        }
    }
}
