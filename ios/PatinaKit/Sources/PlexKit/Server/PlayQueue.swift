// Port of desktop/plex/server/playqueue.ts.
// Play queues (port of crates/plex/src/server/playqueue.rs).

import Foundation

/// Parameters for creating a play queue. Every field is optional (`kind` defaults to the empty
/// string, as in Rust).
public struct PlayQueueRequest: Sendable, Hashable {
    /// Library URI of the source (item, show, season, album, section filter...).
    public var uri: String?
    /// Build from a playlist instead of a URI.
    public var playlistId: String?
    /// `video`, `audio` or `photo`.
    public var kind: String
    /// Item to start at (a `/library/metadata/<id>` key).
    public var key: String?
    public var shuffle: Bool
    public var `repeat`: Bool
    /// Continue with the following items (next episodes) after `key`.
    public var continuous: Bool
    public var includeChapters: Bool
    public var includeMarkers: Bool

    public init(
        uri: String? = nil, playlistId: String? = nil, kind: String = "", key: String? = nil, shuffle: Bool = false,
        repeat: Bool = false, continuous: Bool = false, includeChapters: Bool = false, includeMarkers: Bool = false
    ) {
        self.uri = uri
        self.playlistId = playlistId
        self.kind = kind
        self.key = key
        self.shuffle = shuffle
        self.repeat = `repeat`
        self.continuous = continuous
        self.includeChapters = includeChapters
        self.includeMarkers = includeMarkers
    }

    public static func video(_ uri: String) -> PlayQueueRequest {
        PlayQueueRequest(uri: uri, kind: "video")
    }
}

extension PlexServer {
    public func createPlayQueue(_ req: PlayQueueRequest) async throws -> MediaContainer {
        let p = Params()
            .set("type", req.kind)
            .opt("uri", req.uri)
            .opt("playlistID", req.playlistId)
            .opt("key", req.key)
            .set("shuffle", req.shuffle ? 1 : 0)
            .set("repeat", req.repeat ? 1 : 0)
            .set("continuous", req.continuous ? 1 : 0)
            .flag("includeChapters", req.includeChapters)
            .flag("includeMarkers", req.includeMarkers)
            .set("own", 1)
        return try await container("POST", "/playQueues", p)
    }

    /// Fetch a play queue; `window` limits items around the selected one.
    public func playQueue(_ id: Int, window: Int? = nil) async throws -> MediaContainer {
        let p = Params().opt("window", window).set("includeMarkers", 1).set("includeChapters", 1)
        return try await get("/playQueues/\(id)", p)
    }

    /// Add items; `next` inserts after the current item ("Play Next").
    public func addToPlayQueue(_ id: Int, uri: String, next: Bool) async throws -> MediaContainer {
        try await container("PUT", "/playQueues/\(id)", Params().set("uri", uri).flag("next", next))
    }

    public func removeFromPlayQueue(_ id: Int, itemId: Int) async throws -> MediaContainer {
        try await container("DELETE", "/playQueues/\(id)/items/\(itemId)", Params())
    }

    public func movePlayQueueItem(_ id: Int, itemId: Int, after: Int? = nil) async throws -> MediaContainer {
        try await container("PUT", "/playQueues/\(id)/items/\(itemId)/move", Params().opt("after", after))
    }

    public func shufflePlayQueue(_ id: Int) async throws -> MediaContainer {
        try await container("PUT", "/playQueues/\(id)/shuffle", Params())
    }

    public func unshufflePlayQueue(_ id: Int) async throws -> MediaContainer {
        try await container("PUT", "/playQueues/\(id)/unshuffle", Params())
    }

    public func clearPlayQueue(_ id: Int) async throws -> MediaContainer {
        try await container("DELETE", "/playQueues/\(id)/items", Params())
    }

    /// Reset the queue position to the first item.
    public func resetPlayQueue(_ id: Int) async throws -> MediaContainer {
        try await container("PUT", "/playQueues/\(id)/reset", Params())
    }
}
