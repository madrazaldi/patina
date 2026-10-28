// Port of the smart-downloads half of desktop/commands/auto.ts (the playback preferences half
// is Commands+Auto.swift). Policies are scoped by account and server inside the core.

import Foundation
import PlexKit

extension Commands {
    public static func smartDownloads(_ app: AppState) -> Smart.Overview {
        Smart.overview(app)
    }

    /// Keep the next `keep` unwatched items of a show, season or playlist downloaded.
    public static func setSmartDownload(_ app: AppState, ratingKey: String, keep: Int, deleteWatched: Bool) async throws
        -> Smart.Policy
    {
        try await Smart.setPolicy(app, ratingKey, keep, deleteWatched)
    }

    public static func updateSmartDownload(_ app: AppState, id: String, patch: Smart.Patch) throws {
        try Smart.updatePolicy(app, id, patch)
    }

    public static func removeSmartDownload(_ app: AppState, id: String, deleteDownloads: Bool) throws {
        try Smart.removePolicy(app, id, deleteDownloads)
    }

    /// Limit all downloads together to this many bytes; `nil` removes the limit.
    public static func setDownloadBudget(_ app: AppState, bytes: Int? = nil) throws {
        try Smart.setBudget(app, bytes)
    }

    /// Bring downloads in line with the policies now.
    @discardableResult
    public static func runSmartDownloads(_ app: AppState) async throws -> Smart.Report {
        try await Smart.reconcile(app)
    }
}
