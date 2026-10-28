// STUB: the admin agent replaces this file with the port of desktop/core/ops/edit.ts.
// Only what Commands+ItemActions calls.

import Foundation

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
            throw CoreError("Editing is not ported yet.")
        }
    }
}
