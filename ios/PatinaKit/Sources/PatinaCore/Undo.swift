// Port of desktop/core/undo.ts.
// Undo for reversible actions: watch-state changes (single and bulk), removing a playlist
// entry and removing something from Continue Watching.
//
// Each action goes through a function here, which records what it needs to put things back
// *before* changing anything, does the change, and announces it with a toast carrying the
// undo entry's id (see `Toast.undo`). Front ends then offer "Undo" on that toast and run
// `undo` with the id, or with `nil` for the most recent entry (⌘Z).
//
// The stack belongs to one account and server: entries of another scope are dropped when the
// session changes (`Stack.retainScope`) and refused if one slips through.

import Foundation
import PlexKit

public enum Undo {
    /// How many undoable actions are remembered.
    public nonisolated static let DEPTH = 25
    /// Calls made at once when restoring many items.
    nonisolated static let CONCURRENCY = 6

    /// The account and server an undo entry belongs to.
    public static func scopeOf(_ session: Session) -> String {
        "\(Offline.account(session)):\(session.serverId)"
    }

    private static func currentScope(_ app: AppState) -> String? {
        app.session().map(scopeOf)
    }

    /// What an item's watch state was.
    public nonisolated struct WatchState: Sendable, Equatable {
        public var ratingKey: String
        public var watched: Bool
        /// Resume position in milliseconds.
        public var offset: Int

        public init(ratingKey: String, watched: Bool, offset: Int) {
            self.ratingKey = ratingKey
            self.watched = watched
            self.offset = offset
        }
    }

    private static func watchStateOf(_ item: Metadata) -> WatchState {
        WatchState(ratingKey: item.ratingKey ?? "", watched: (item.viewCount ?? 0) > 0, offset: max(item.viewOffset ?? 0, 0))
    }

    /// How to reverse one change.
    public nonisolated enum Action: Sendable, Equatable {
        /// Put these items back to their earlier watch state.
        case watch(server: String?, restore: [WatchState])
        /// Put a removed entry back into a playlist, after the entry it followed.
        case playlistEntry(playlist: String, ratingKey: String, after: Int?)
        /// Bring back a resume position (what removing from Continue Watching and clearing progress take away).
        case progress(server: String?, ratingKey: String, offset: Int)
    }

    public nonisolated struct Entry: Sendable, Equatable {
        public var id: Int
        public var scope: String
        /// What the action was, as in the toast ("Marked Heat watched").
        public var label: String
        public var action: Action
    }

    /// The undo history, newest last.
    public nonisolated struct Stack: Sendable, Equatable {
        public private(set) var entries: [Entry] = []
        private var next = 0

        public init() {}

        /// Remember an action; returns its id. Entries of other scopes are dropped first.
        public mutating func push(_ scope: String, _ label: String, _ action: Action) -> Int {
            retainScope(scope)
            next += 1
            entries.append(Entry(id: next, scope: scope, label: label, action: action))
            if entries.count > DEPTH { entries.removeFirst() }
            return next
        }

        /// Drop everything that does not belong to `scope`.
        public mutating func retainScope(_ scope: String) {
            entries.removeAll { $0.scope != scope }
        }

        public mutating func clear() {
            entries = []
        }

        /// The newest entry of `scope`.
        public func last(_ scope: String) -> Entry? {
            entries.last { $0.scope == scope }
        }

        /// Remove and return entry `id`, or the newest one of `scope`.
        public mutating func take(_ scope: String, _ id: Int? = nil) -> Entry? {
            retainScope(scope)
            let at: Int
            if let id {
                guard let found = entries.firstIndex(where: { $0.id == id }) else { return nil }
                at = found
            } else {
                guard !entries.isEmpty else { return nil }
                at = entries.count - 1
            }
            return entries.remove(at: at)
        }

        /// Put an entry that could not be undone back where it was (the newest slot).
        public mutating func restore(_ entry: Entry) {
            entries.append(entry)
        }

        public var count: Int { entries.count }
        public var isEmpty: Bool { entries.isEmpty }
    }

    /// Forget every entry (the session ended).
    public static func clear(_ app: AppState) {
        app.undo.clear()
    }

    /// Drop entries that belong to another account or server.
    public static func sessionChanged(_ app: AppState, _ session: Session?) {
        if let session { app.undo.retainScope(scopeOf(session)) } else { app.undo.clear() }
    }

    /// The id and description of what `undo` would reverse.
    public static func last(_ app: AppState) -> (id: Int, label: String)? {
        guard let scope = currentScope(app), let e = app.undo.last(scope) else { return nil }
        return (e.id, e.label)
    }

    /// Remember an action begun in `scope` (taken before its first request). Nothing is
    /// recorded if the account or server changed meanwhile: the action's rating keys and
    /// "current server" would then be read against the wrong one.
    private static func record(_ app: AppState, _ scope: String?, _ label: String, _ action: Action) -> Int? {
        guard let scope, currentScope(app) == scope else { return nil }
        return app.undo.push(scope, label, action)
    }

    /// Toast `text`, offering undo when the action was recorded.
    private static func announce(_ app: AppState, _ text: String, _ undoId: Int?) {
        if let undoId { app.toastUndo(text, undoId) } else { app.toast(text) }
    }

    // MARK: Watch state

    /// The items whose state `watched` would change, with their current watch state.
    public static func changing(_ items: some Sequence<Metadata>, _ watched: Bool) -> [WatchState] {
        items.map(watchStateOf).filter { watched ? !$0.watched : $0.watched || $0.offset > 0 }
    }

    /// The watch state of `keys` and, for shows, seasons, artists and albums, of everything inside.
    private static func leafStates(_ server: PlexServer, _ keys: [String]) async throws -> ([Metadata], [Metadata]) {
        var found: [Metadata] = []
        for start in stride(from: 0, to: keys.count, by: 40) {
            found += try await server.metadataMany(Array(keys[start..<min(start + 40, keys.count)]))
        }
        var leaves: [Metadata] = []
        var nested: [String] = []
        for item in found {
            if ["show", "season", "artist", "album"].contains(item.type ?? "") {
                nested.append(item.ratingKey ?? "")
            } else {
                leaves.append(item)
            }
        }
        let inside = try await Ops.Util.mapLimit(nested, CONCURRENCY) { rk, _ in try await server.allLeaves(rk).metadata }
        for list in inside { leaves += list }
        return (found, leaves)
    }

    public static func watchLabel(_ found: [Metadata], _ keys: Int, _ watched: Bool) -> String {
        let state = watched ? "watched" : "unwatched"
        if found.count == 1 && keys == 1 { return "Marked \(found[0].title ?? "") \(state)" }
        if keys == 1 { return "Marked as \(state)" }
        return "Marked \(keys) items \(state)"
    }

    /// Mark items (or whole shows, seasons, albums) watched or unwatched on `serverId` (`nil`
    /// for the current one), remembering how to put them back.
    public static func setWatched(_ app: AppState, _ serverId: String?, _ keys: [String], _ watched: Bool) async throws {
        let scope = currentScope(app)
        let foreign = serverId.flatMap { Servers.isCurrent(app, $0) ? nil : $0 }
        let server = try await Servers.serverFor(app, serverId)
        // The earlier state is best effort: failing to read it must not stop the change.
        let before = try? await leafStates(server, keys)
        for key in keys {
            if watched { try await server.scrobble(key) } else { try await server.unscrobble(key) }
        }
        var undoEntry: (String, Action)?
        if let (found, leaves) = before {
            let restore = changing(leaves, watched)
            if !restore.isEmpty {
                undoEntry = (watchLabel(found, keys.count, watched), .watch(server: foreign, restore: restore))
            }
        }
        let state = watched ? "watched" : "unwatched"
        let text = undoEntry?.0 ?? (keys.count == 1 ? "Marked as \(state)" : "Marked \(keys.count) items \(state)")
        let id = undoEntry.flatMap { record(app, scope, $0.0, $0.1) }
        announce(app, text, id)
        app.bumpWith(.everything())
    }

    /// Forget the resume position without changing the watched state.
    public static func clearProgress(_ app: AppState, _ ratingKey: String) async throws {
        let scope = currentScope(app)
        let server = try app.server()
        let before = (try? await server.metadata(ratingKey)) ?? nil
        try await server.setProgress(ratingKey, timeMs: 0, state: "stopped")
        let offset = before?.viewOffset ?? 0
        var id: Int?
        if offset > 0 {
            let label = "Cleared progress of \(before?.title ?? "the item")"
            id = record(app, scope, label, .progress(server: nil, ratingKey: ratingKey, offset: offset))
        }
        announce(app, "Progress cleared", id)
        app.bumpWith(.everything())
    }

    /// Hide an item from Continue Watching. Undo is offered when it had a resume position to bring back.
    public static func removeFromContinueWatching(_ app: AppState, _ ratingKey: String) async throws {
        let scope = currentScope(app)
        let server = try app.server()
        let before = (try? await server.metadata(ratingKey)) ?? nil
        try await server.removeFromContinueWatching(ratingKey)
        let offset = before?.viewOffset ?? 0
        var id: Int?
        if offset > 0 {
            let label = "Removed \(before?.title ?? "the item") from Continue Watching"
            id = record(app, scope, label, .progress(server: nil, ratingKey: ratingKey, offset: offset))
        }
        announce(app, "Removed from Continue Watching", id)
        app.bumpWith(.everything())
    }

    // MARK: Playlists

    /// A playlist entry as far as position matters.
    public nonisolated struct Slot: Sendable, Equatable {
        public var entryId: Int
        public var ratingKey: String
        public var title: String

        public init(entryId: Int, ratingKey: String, title: String) {
            self.entryId = entryId
            self.ratingKey = ratingKey
            self.title = title
        }
    }

    private static func playlistSlots(_ server: PlexServer, _ id: String) async throws -> [Slot] {
        let PAGE = 500
        var out: [Slot] = []
        while true {
            let mc = try await server.playlistItems(id, start: out.count, size: PAGE)
            let got = mc.metadata.count
            let total = mc.totalSize.map { max($0, 0) }
            for m in mc.metadata {
                guard let entry = m.playlistItemID else { continue }
                out.append(Slot(entryId: entry, ratingKey: m.ratingKey ?? "", title: m.title ?? ""))
            }
            if got < PAGE || total.map({ out.count >= $0 }) == true || out.count >= 50_000 { break }
        }
        return out
    }

    /// The slot of `entryId` and the entry id it follows.
    public static func locate(_ slots: [Slot], _ entryId: Int) -> (Slot, Int?)? {
        guard let at = slots.firstIndex(where: { $0.entryId == entryId }) else { return nil }
        return (slots[at], at > 0 ? slots[at - 1].entryId : nil)
    }

    /// Remove one entry from a playlist, remembering where it was.
    public static func removeFromPlaylist(_ app: AppState, _ playlist: String, _ entryId: Int) async throws {
        let scope = currentScope(app)
        let server = try app.server()
        let slots = try? await playlistSlots(server, playlist)
        let found = slots.flatMap { locate($0, entryId) }
        try await server.removeFromPlaylist(playlist, playlistItemId: entryId)
        let text: String
        var id: Int?
        if let (slot, after) = found {
            let label = "Removed \(slot.title) from the playlist"
            id = record(app, scope, label, .playlistEntry(playlist: playlist, ratingKey: slot.ratingKey, after: after))
            text = label
        } else {
            text = "Removed from the playlist"
        }
        announce(app, text, id)
        app.bumpWith(.everything())
    }

    /// Where to put a restored entry, given the playlist as it is now (with the restored entry
    /// last): after `after` if that entry still exists, at the top when it was first, else it
    /// stays at the end. The outer `nil` is "stay at the end"; `.some(after)` carries the
    /// target (itself `nil` for the top).
    public static func place(_ slots: [Slot], _ after: Int?) -> Int?? {
        guard let after else { return .some(nil) }
        // `slots.iter().rev().skip(1)`: everything but the last (the restored entry itself).
        if slots.dropLast().contains(where: { $0.entryId == after }) { return .some(after) }
        return nil
    }

    private static func restorePlaylistEntry(_ server: PlexServer, _ playlist: String, _ ratingKey: String, _ after: Int?)
        async throws
    {
        try await server.addToPlaylist(playlist, uri: server.itemUri([ratingKey]))
        let slots = try await playlistSlots(server, playlist)
        guard let added = slots.last else { throw CoreError("The playlist is empty.") }
        if added.ratingKey == ratingKey, case .some(let target) = place(slots, after) {
            try await server.movePlaylistItem(playlist, playlistItemId: added.entryId, after: target)
        }
    }

    // MARK: Undoing

    /// Restore calls for watch states: scrobble or unscrobble, then the resume position.
    public nonisolated enum Call: Sendable, Equatable {
        case scrobble(ratingKey: String)
        case unscrobble(ratingKey: String)
        case progress(ratingKey: String, ms: Int)

        var ratingKey: String {
            switch self {
            case .scrobble(let rk), .unscrobble(let rk), .progress(let rk, _): rk
            }
        }
    }

    public static func callsFor(_ restore: [WatchState]) -> [Call] {
        var out: [Call] = []
        for s in restore {
            out.append(s.watched ? .scrobble(ratingKey: s.ratingKey) : .unscrobble(ratingKey: s.ratingKey))
            if !s.watched && s.offset > 0 { out.append(.progress(ratingKey: s.ratingKey, ms: s.offset)) }
        }
        return out
    }

    private static func runCalls(_ server: PlexServer, _ calls: [Call]) async throws {
        // One item's calls stay in order; different items go in parallel.
        var order: [String] = []
        var perItem: [String: [Call]] = [:]
        for c in calls {
            if perItem[c.ratingKey] == nil { order.append(c.ratingKey) }
            perItem[c.ratingKey, default: []].append(c)
        }
        _ = try await Ops.Util.mapLimit(order.map { perItem[$0]! }, CONCURRENCY) { list, _ in
            for c in list {
                switch c {
                case .scrobble(let rk): try await server.scrobble(rk)
                case .unscrobble(let rk): try await server.unscrobble(rk)
                case .progress(let rk, let ms): try await server.setProgress(rk, timeMs: ms, state: "stopped")
                }
            }
        }
    }

    /// Reverse an action. `id` names an entry (an Undo button), `nil` the most recent one.
    /// Returns what was undone; the front ends are told with a toast and a revision.
    @discardableResult
    public static func undo(_ app: AppState, _ id: Int? = nil) async throws -> String {
        guard let scope = currentScope(app) else { throw CoreError("Not connected to a server.") }
        guard let entry = app.undo.take(scope, id) else { throw CoreError("Nothing to undo.") }
        do {
            switch entry.action {
            case .watch(let server, let restore):
                try await runCalls(try await Servers.serverFor(app, server), callsFor(restore))
            case .playlistEntry(let playlist, let ratingKey, let after):
                try await restorePlaylistEntry(try app.server(), playlist, ratingKey, after)
            case .progress(let server, let ratingKey, let offset):
                try await Servers.serverFor(app, server).setProgress(ratingKey, timeMs: offset, state: "stopped")
            }
        } catch {
            // Keep it so a retry is possible once the server is reachable.
            if currentScope(app) == entry.scope { app.undo.restore(entry) }
            throw error
        }
        app.toast("Undid: \(entry.label)")
        app.bumpWith(.everything())
        return entry.label
    }
}
