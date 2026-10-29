// Port of desktop/plex/server/status.ts.
// Sessions and history (port of crates/plex/src/server/status.rs).

import Foundation

/// Filters for watch history. Every field is optional.
public struct HistoryQuery: Sendable, Hashable {
    public var accountId: Int?
    public var sectionId: String?
    public var ratingKey: String?
    /// Only entries viewed at or after this unix timestamp.
    public var since: Int?
    public var start: Int
    public var size: Int

    public init(
        accountId: Int? = nil, sectionId: String? = nil, ratingKey: String? = nil, since: Int? = nil, start: Int = 0,
        size: Int = 0
    ) {
        self.accountId = accountId
        self.sectionId = sectionId
        self.ratingKey = ratingKey
        self.since = since
        self.start = start
        self.size = size
    }
}

extension PlexServer {
    /// Everything currently playing on the server, with user, player and transcode info.
    public func sessions() async throws -> [Metadata] {
        try await get("/status/sessions").metadata
    }

    /// Watch history (newest first).
    public func history(_ q: HistoryQuery) async throws -> MediaContainer {
        let p = Params()
            .set("sort", "viewedAt:desc")
            .opt("accountID", q.accountId)
            .opt("librarySectionID", q.sectionId)
            .opt("metadataItemID", q.ratingKey)
            .opt("viewedAt>", q.since)
            .set("X-Plex-Container-Start", q.start)
            .set("X-Plex-Container-Size", q.size == 0 ? 100 : q.size)
        return try await get("/status/sessions/history/all", p)
    }

    public func deleteHistory(_ historyId: String) async throws {
        try await delete("/status/sessions/history/\(historyId)")
    }

    /// Stop someone's stream with a message (Plex Pass).
    public func terminateSession(_ sessionId: String, reason: String) async throws {
        try await empty(
            "GET", "/status/sessions/terminate", Params().set("sessionId", sessionId).set("reason", reason))
    }
}
