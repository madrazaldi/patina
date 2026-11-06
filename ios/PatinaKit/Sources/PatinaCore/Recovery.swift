// Port of desktop/core/recovery.ts.
// What survives a connection failure or a restart: a local position per item (checkpoints)
// and the play queue with its position.
//
// Both are written to the data directory and scoped by account and server, so another
// profile or server never sees them. Plex stays the source of truth for watch state; a
// checkpoint only matters when the server did not hear about the last minutes of playback
// (the connection dropped), which is why one is ignored once the server has a newer view of
// the item.

import Foundation
import PlexKit

/// The account and server something belongs to.
public nonisolated struct Scope: Sendable, Hashable {
    public var account: String
    public var server: String

    public init(_ account: String, _ server: String) {
        self.account = account
        self.server = server
    }

    public static func of(_ session: Session) -> Scope {
        Scope(Offline.account(session), session.serverId)
    }

    /// `Option<Scope>` equality.
    public static func eq(_ a: Scope?, _ b: Scope?) -> Bool {
        a == b
    }
}

// Serialized with the Rust core's snake_case names.
public nonisolated struct Checkpoint: Codable, Sendable, Equatable {
    public var account: String
    public var server: String
    public var ratingKey: String
    public var position: Int
    public var duration: Int
    public var updated: Int

    enum CodingKeys: String, CodingKey {
        case account, server
        case ratingKey = "rating_key"
        case position, duration, updated
    }

    public init(account: String, server: String, ratingKey: String, position: Int, duration: Int, updated: Int) {
        self.account = account
        self.server = server
        self.ratingKey = ratingKey
        self.position = position
        self.duration = duration
        self.updated = updated
    }
}

/// The queue that was playing, to offer again after a restart.
public nonisolated struct SavedQueue: Codable, Sendable, Equatable {
    public var account: String
    public var server: String
    public var savedAt: Int
    /// `video` or `audio`.
    public var kind: String
    /// Slim copies of the queue items, in play order.
    public var items: [Metadata]
    public var current: Int
    public var positionMs: Int
    public var quality: String?

    enum CodingKeys: String, CodingKey {
        case account, server
        case savedAt = "saved_at"
        case kind, items, current
        case positionMs = "position_ms"
        case quality
    }

    public init(
        account: String, server: String, savedAt: Int, kind: String, items: [Metadata], current: Int, positionMs: Int,
        quality: String?
    ) {
        self.account = account
        self.server = server
        self.savedAt = savedAt
        self.kind = kind
        self.items = items
        self.current = current
        self.positionMs = positionMs
        self.quality = quality
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        account = try c.decode(String.self, forKey: .account)
        server = try c.decode(String.self, forKey: .server)
        savedAt = try c.decode(Int.self, forKey: .savedAt)
        kind = try c.decode(String.self, forKey: .kind)
        items = try c.decode([Metadata].self, forKey: .items)
        current = try c.decode(Int.self, forKey: .current)
        positionMs = try c.decode(Int.self, forKey: .positionMs)
        quality = try c.decodeIfPresent(String.self, forKey: .quality)
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(account, forKey: .account)
        try c.encode(server, forKey: .server)
        try c.encode(savedAt, forKey: .savedAt)
        try c.encode(kind, forKey: .kind)
        try c.encode(items, forKey: .items)
        try c.encode(current, forKey: .current)
        try c.encode(positionMs, forKey: .positionMs)
        // `Option` is written as null, as serde does.
        try c.encode(quality, forKey: .quality)
    }
}

/// What a front end shows in its "Resume" offer.
public nonisolated struct SavedQueueView: Sendable, Equatable {
    public var current: Metadata
    public var positionMs: Int
    public var index: Int
    public var count: Int
    public var savedAt: Int
}

public enum Recovery {
    static func checkpointsPath(_ paths: Paths) -> URL {
        paths.data.appendingPathComponent("playback-checkpoints.json")
    }

    static func queuesPath(_ paths: Paths) -> URL {
        paths.data.appendingPathComponent("saved-queues.json")
    }

    nonisolated static let MONTH = 30 * 24 * 3600
    /// A local checkpoint is only used to start playback again for this long; after that the
    /// server's position (or an explicit resume of a saved queue) decides.
    public nonisolated static let RECOVERY_WINDOW = 6 * 3600
    nonisolated static let MAX_CHECKPOINTS = 200
    nonisolated static let MAX_QUEUES = 16
    /// Longest queue kept; around the playing item when there are more.
    public nonisolated static let MAX_QUEUE_ITEMS = 300

    // MARK: Checkpoints

    private static func isCheckpoint(_ c: Checkpoint, _ scope: Scope, _ ratingKey: String) -> Bool {
        c.account == scope.account && c.server == scope.server && c.ratingKey == ratingKey
    }

    /// Remember where an item is being played.
    public static func saveCheckpoint(_ paths: Paths, _ scope: Scope, _ ratingKey: String, _ position: Int, _ duration: Int)
        throws
    {
        let now = Offline.now()
        try Storage.updateJson(checkpointsPath(paths), { [Checkpoint]() }) { list in
            list.removeAll { isCheckpoint($0, scope, ratingKey) || now - $0.updated >= MONTH }
            list.append(
                Checkpoint(
                    account: scope.account, server: scope.server, ratingKey: ratingKey, position: max(0, position),
                    duration: duration, updated: now))
            if list.count > MAX_CHECKPOINTS { list.removeFirst(list.count - MAX_CHECKPOINTS) }
        }
    }

    /// Forget an item's checkpoint (it was finished).
    public static func clearCheckpoint(_ paths: Paths, _ scope: Scope, _ ratingKey: String) throws {
        try Storage.updateJson(checkpointsPath(paths), { [Checkpoint]() }) { list in
            list.removeAll { isCheckpoint($0, scope, ratingKey) }
        }
    }

    public static func checkpoint(_ paths: Paths, _ scope: Scope, _ ratingKey: String) -> Checkpoint? {
        let list = (try? Storage.readJson(checkpointsPath(paths)) { [Checkpoint]() }) ?? []
        return list.first { isCheckpoint($0, scope, ratingKey) }
    }

    /// The position to resume `item` from, if a local checkpoint is newer than what the server knows.
    public static func resumePosition(_ paths: Paths, _ scope: Scope, _ item: Metadata) -> Int? {
        guard let rk = item.ratingKey, let cp = checkpoint(paths, scope, rk) else { return nil }
        return usable(cp, item.lastViewedAt, Offline.now())
    }

    public nonisolated static func usable(_ cp: Checkpoint, _ lastViewedAt: Int?, _ now: Int) -> Int? {
        let finished = cp.duration > 0 && Double(cp.position) >= Double(cp.duration) * 0.95
        // The server saw the item after the checkpoint (here or on another device): it knows better.
        let serverNewer = lastViewedAt.map { $0 > cp.updated } ?? false
        return cp.position > 0 && !finished && !serverNewer && now - cp.updated < RECOVERY_WINDOW ? cp.position : nil
    }

    // MARK: Saved queue

    private static func isQueueOf(_ q: SavedQueue, _ scope: Scope) -> Bool {
        q.account == scope.account && q.server == scope.server
    }

    public nonisolated static func savedQueueView(_ q: SavedQueue) -> SavedQueueView? {
        guard q.items.indices.contains(q.current) else { return nil }
        return SavedQueueView(
            current: q.items[q.current], positionMs: q.positionMs, index: q.current, count: q.items.count,
            savedAt: q.savedAt)
    }

    /// An item reduced to what is needed to list and re-queue it.
    public nonisolated static func slim(_ m: Metadata) -> Metadata {
        Metadata(
            ratingKey: m.ratingKey, key: m.key, type: m.type, title: m.title, year: m.year, index: m.index,
            parentIndex: m.parentIndex, lastViewedAt: m.lastViewedAt, duration: m.duration, thumb: m.thumb,
            parentRatingKey: m.parentRatingKey, parentTitle: m.parentTitle, parentThumb: m.parentThumb,
            grandparentRatingKey: m.grandparentRatingKey, grandparentTitle: m.grandparentTitle,
            grandparentThumb: m.grandparentThumb, librarySectionID: m.librarySectionID)
    }

    /// At most `MAX_QUEUE_ITEMS` items keeping `current` in view; returns the window and the
    /// new index of `current`.
    public nonisolated static func window(_ len: Int, _ current: Int) -> (Range<Int>, Int) {
        if len <= MAX_QUEUE_ITEMS { return (0..<len, current) }
        let start = min(max(0, current - MAX_QUEUE_ITEMS / 10), len - MAX_QUEUE_ITEMS)
        return (start..<(start + MAX_QUEUE_ITEMS), current - start)
    }

    public static func saveQueue(_ paths: Paths, _ scope: Scope, _ queue: SavedQueue) throws {
        let now = Offline.now()
        try Storage.updateJson(queuesPath(paths), { [SavedQueue]() }) { list in
            list.removeAll { isQueueOf($0, scope) || now - $0.savedAt >= MONTH }
            list.append(queue)
            if list.count > MAX_QUEUES { list.removeFirst(list.count - MAX_QUEUES) }
        }
    }

    public static func loadQueue(_ paths: Paths, _ scope: Scope) -> SavedQueue? {
        let now = Offline.now()
        guard let list = try? Storage.readJson(queuesPath(paths), { [SavedQueue]() }) else { return nil }
        return list.first { isQueueOf($0, scope) && now - $0.savedAt < MONTH && $0.current < $0.items.count }
    }

    public static func clearQueue(_ paths: Paths, _ scope: Scope) throws {
        try Storage.updateJson(queuesPath(paths), { [SavedQueue]() }) { list in
            list.removeAll { isQueueOf($0, scope) }
        }
    }
}
