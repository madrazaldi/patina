// Port of the "Undo" section of desktop/commands/index.ts.

import Foundation

extension Commands {
    public struct UndoLast: Sendable, Equatable {
        public var id: Int
        public var label: String
    }

    /// What ⌘Z would undo, if anything.
    public static func undoLast(_ app: AppState) -> UndoLast? {
        Undo.last(app).map { UndoLast(id: $0.id, label: $0.label) }
    }

    /// Undo entry `id` (a toast's Undo button), or the most recent one. Returns what was undone.
    @discardableResult
    public static func undoAction(_ app: AppState, id: Int? = nil) async throws -> String {
        try await Undo.undo(app, id)
    }
}
