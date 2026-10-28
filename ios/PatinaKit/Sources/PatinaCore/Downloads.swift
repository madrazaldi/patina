// STUB: the downloads agent replaces this file with the port of desktop/core/downloads.ts.
// Only what the foundation calls: the `Download` type (all its fields), `list` and `resume`.

import Foundation
import PlexKit

public nonisolated struct Download: Sendable, Equatable {
    public var id: String
    public var serverId: String
    public var serverName: String
    public var item: Metadata
    public var file: String?
    public var poster: String?
    public var size: Int
    public var received: Int
    public var state: Downloads.State
    public var error: String?
    public var addedAt: Int
    /// Account that queued this file. Legacy downloads remain playable but never sync to an inferred user.
    public var account: String
    public var validator: String?
    public var sourceKey: String?
    /// Smart-download policy that fetched this file and may remove it again (see Smart.swift);
    /// `nil` for downloads someone asked for by hand.
    public var policy: String?

    public init(
        id: String, serverId: String, serverName: String, item: Metadata, file: String? = nil, poster: String? = nil,
        size: Int = 0, received: Int = 0, state: Downloads.State = .queued, error: String? = nil, addedAt: Int = 0,
        account: String = "", validator: String? = nil, sourceKey: String? = nil, policy: String? = nil
    ) {
        self.id = id
        self.serverId = serverId
        self.serverName = serverName
        self.item = item
        self.file = file
        self.poster = poster
        self.size = size
        self.received = received
        self.state = state
        self.error = error
        self.addedAt = addedAt
        self.account = account
        self.validator = validator
        self.sourceKey = sourceKey
        self.policy = policy
    }
}

public enum Downloads {
    public nonisolated enum State: String, Sendable, Codable {
        case queued, downloading, paused, done, failed
    }

    /// The manifest (`downloads.json` in the config dir).
    public static func list(_ paths: Paths) -> [Download] {
        []
    }

    /// Start (or continue) the transfer worker.
    public static func resume(_ app: AppState) {}
}
