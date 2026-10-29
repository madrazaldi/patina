// Port of desktop/core/smart.ts.
// Smart downloads: keep the next few unwatched episodes of a show, season or playlist
// downloaded, and clean up after watching.
//
// A `Policy` belongs to one account on one server. `reconcile` compares what each enabled
// policy wants with what is downloaded and queues or removes files to match. It only ever
// looks at the connected server and account, and it only removes downloads a policy fetched
// itself (`Download.policy`); downloads someone asked for by hand are never touched. An
// optional storage budget limits how much a policy may add.
//
// Reconciling runs on sign-in, whenever watch state may have changed (after playback, a
// scrobble, an offline sync or a live server update, through `AppState.bump`), every few
// minutes, and in the app's background refresh (`backgroundRefresh`).

import Foundation
import PlexKit

nonisolated private let log = logger("smart")

public enum Smart {
    /// What a policy keeps downloaded.
    public nonisolated enum Target: String, Codable, Sendable {
        case show, season, playlist
    }

    public nonisolated static func targetLabel(_ t: Target) -> String {
        t.rawValue
    }

    public nonisolated struct Policy: Codable, Sendable, Equatable {
        public var id: String
        /// Account and server the policy belongs to; it is inert anywhere else.
        public var account: String
        public var serverId: String
        public var serverName: String
        /// The show, season or playlist.
        public var ratingKey: String
        public var target: Target
        public var title: String
        /// How many unwatched items to keep downloaded.
        public var keep: Int
        /// Remove downloads this policy fetched once they are watched.
        public var deleteWatched: Bool
        public var enabled: Bool

        public init(
            id: String, account: String, serverId: String, serverName: String, ratingKey: String, target: Target,
            title: String, keep: Int, deleteWatched: Bool, enabled: Bool
        ) {
            self.id = id
            self.account = account
            self.serverId = serverId
            self.serverName = serverName
            self.ratingKey = ratingKey
            self.target = target
            self.title = title
            self.keep = keep
            self.deleteWatched = deleteWatched
            self.enabled = enabled
        }
    }

    public nonisolated static let MAX_KEEP = 50

    nonisolated struct Saved: Codable, Sendable {
        /// Most bytes all downloads together may use before policies stop adding.
        var budget: Int?
        var policies: [Policy] = []

        init() {}

        /// `#[serde(default)]` on the whole struct: missing fields take their defaults.
        init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            budget = (try? c.decodeIfPresent(Double.self, forKey: .budget)).flatMap { $0.map { Int($0) } }
            policies = try c.decodeIfPresent([Policy].self, forKey: .policies) ?? []
        }

        func encode(to encoder: Encoder) throws {
            var c = encoder.container(keyedBy: CodingKeys.self)
            try c.encode(budget, forKey: .budget)
            try c.encode(policies, forKey: .policies)
        }

        enum CodingKeys: String, CodingKey {
            case budget, policies
        }
    }

    nonisolated static func path(_ paths: Paths) -> URL {
        paths.config.appendingPathComponent("smart-downloads.json")
    }

    static func load(_ paths: Paths) -> Saved {
        do {
            return try Storage.readJson(path(paths)) { Saved() }
        } catch {
            log.error("smart downloads: \(errorText(error))")
            return Saved()
        }
    }

    static func update<R>(_ paths: Paths, _ f: (inout Saved) throws -> R) throws -> R {
        try Storage.updateJson(path(paths), { Saved() }, f)
    }

    /// The saved policies as text, to notice them changing.
    public nonisolated static func fingerprint(_ paths: Paths) -> String {
        (try? String(contentsOf: path(paths), encoding: .utf8)) ?? ""
    }

    static func notify(_ app: AppState) {
        app.emit(.downloads(Downloads.list(app.paths)))
    }

    // MARK: Evaluation

    /// One item a policy could keep, in the order it would be watched.
    public nonisolated struct Candidate: Sendable, Equatable {
        public var ratingKey: String
        /// Id of its download.
        public var id: String
        public var watched: Bool
        public var size: Int

        public init(ratingKey: String, id: String, watched: Bool, size: Int) {
            self.ratingKey = ratingKey
            self.id = id
            self.watched = watched
            self.size = size
        }
    }

    /// A policy together with its items, as fetched from the server.
    public nonisolated struct Fetched: Sendable {
        public var policy: Policy
        public var candidates: [Candidate]

        public init(policy: Policy, candidates: [Candidate]) {
            self.policy = policy
            self.candidates = candidates
        }
    }

    /// A download of the connected account and server.
    public nonisolated struct Held: Sendable, Equatable {
        public var id: String
        public var policy: String?
        public var size: Int

        public init(id: String, policy: String?, size: Int) {
            self.id = id
            self.policy = policy
            self.size = size
        }
    }

    /// What to change.
    public nonisolated struct Plan: Sendable, Equatable {
        /// `(policy id, rating key)` to download, in order.
        public var add: [Pair] = []
        /// Download ids to remove.
        public var remove: [String] = []
        /// Downloads whose owner is gone: adopted by a policy that wants them, or released.
        public var assign: [Assignment] = []
        /// Items left out because the budget is full.
        public var overBudget = 0

        public init() {}

        public struct Pair: Sendable, Equatable {
            public var policy: String
            public var ratingKey: String
        }

        public struct Assignment: Sendable, Equatable {
            public var id: String
            public var policy: String?
        }
    }

    public nonisolated static func emptyPlan() -> Plan {
        Plan()
    }

    /// Work out what the policies need.
    ///
    /// - Each policy wants the first `keep` unwatched items.
    /// - A download a policy fetched is removed when no policy wants it any more and it is
    ///   not a watched item that policy is told to keep. Downloads of policies that are
    ///   disabled, or whose items could not be fetched, are left alone.
    /// - New items are added in order until `budget` (counting `outsideBytes`, the downloads
    ///   of other servers and accounts, and removals just freed) is reached.
    public nonisolated static func evaluate(
        _ fetched: [Fetched], _ held: [Held], _ known: Set<String>, _ outsideBytes: Int, _ budget: Int?
    ) -> Plan {
        var plan = Plan()
        let evaluated = Set(fetched.map(\.policy.id))

        var wanted: [String: String] = [:]
        var keepWatched = Set<String>()
        var windows: [[Candidate]] = []
        for f in fetched {
            let window = Array(f.candidates.filter { !$0.watched }.prefix(max(f.policy.keep, 0)))
            for c in window where wanted[c.id] == nil { wanted[c.id] = f.policy.id }
            if !f.policy.deleteWatched {
                for c in f.candidates where c.watched { keepWatched.insert(c.id) }
            }
            windows.append(window)
        }

        var used = outsideBytes
        var present = Set<String>()
        for h in held {
            let size = max(h.size, 0)
            if let owner = h.policy, evaluated.contains(owner), wanted[h.id] == nil, !keepWatched.contains(h.id) {
                plan.remove.append(h.id)
                continue
            }
            if let owner = h.policy, !known.contains(owner) {
                plan.assign.append(Plan.Assignment(id: h.id, policy: wanted[h.id]))
            }
            used += size
            present.insert(h.id)
        }

        for (f, window) in zip(fetched, windows) {
            for (at, c) in window.enumerated() {
                if present.contains(c.id) { continue }
                let size = max(c.size, 0)
                if let budget, used + size > budget {
                    // Later items must not jump the queue.
                    plan.overBudget += window[at...].filter { !present.contains($0.id) }.count
                    break
                }
                used += size
                present.insert(c.id)
                plan.add.append(Plan.Pair(policy: f.policy.id, ratingKey: c.ratingKey))
            }
        }
        return plan
    }

    /// Put the items of a policy in the order they are watched: a show's episodes by season
    /// and number (specials excluded), a season's by number, a playlist as it is.
    public nonisolated static func watchOrder(_ target: Target, _ items: [Metadata]) -> [Metadata] {
        // Stable sorts, like `sort_by_key`.
        func stable(_ items: [Metadata], _ less: (Metadata, Metadata) -> Bool) -> [Metadata] {
            items.enumerated().sorted { a, b in
                less(a.element, b.element) || (!less(b.element, a.element) && a.offset < b.offset)
            }.map(\.element)
        }
        switch target {
        case .show:
            return stable(items.filter { $0.parentIndex != 0 }) { a, b in
                (a.parentIndex ?? 0, a.index ?? 0) < (b.parentIndex ?? 0, b.index ?? 0)
            }
        case .season:
            return stable(items) { ($0.index ?? 0) < ($1.index ?? 0) }
        case .playlist:
            return items
        }
    }

    nonisolated static func partSize(_ m: Metadata) -> Int {
        m.media.first?.part.first?.size ?? 0
    }

    public nonisolated static func candidates(_ items: [Metadata], _ scope: Scope, _ offlineWatched: Set<String>)
        -> [Candidate]
    {
        items.filter { Metadata.isPlayable($0) && !$0.media.isEmpty }.map { m in
            let id = Downloads.idFor(scope.server, scope.account, Metadata.ratingKey(m))
            return Candidate(
                ratingKey: Metadata.ratingKey(m), id: id, watched: Metadata.isWatched(m) || offlineWatched.contains(id),
                size: partSize(m))
        }
    }

    /// Downloads of this account and server, and the bytes of all the others.
    public nonisolated static func split(_ all: [Download], _ scope: Scope) -> ([Held], Int) {
        var held: [Held] = []
        var outside = 0
        for d in all {
            if d.serverId == scope.server && d.account == scope.account {
                held.append(Held(id: d.id, policy: d.policy, size: d.size))
            } else {
                outside += max(d.size, 0)
            }
        }
        return (held, outside)
    }

    public nonisolated static func inScope(_ p: Policy, _ scope: Scope) -> Bool {
        p.account == scope.account && p.serverId == scope.server
    }

    // MARK: Reconciling

    public nonisolated struct Report: Sendable, Equatable {
        public var added = 0
        public var removed = 0
        /// Items not queued because the storage budget is full.
        public var overBudget = 0
        public var errors: [String] = []
    }

    /// The module's `running`, `pending` and `last`, per app.
    final class Run {
        var running = false
        var pending = false
        var last: ContinuousClock.Instant?
    }

    private static func run(_ app: AppState) -> Run {
        app.local(Run.self) { Run() }
    }

    private static func fetchItems(_ server: PlexServer, _ p: Policy) async throws -> [Metadata] {
        switch p.target {
        case .show: try await server.allLeaves(p.ratingKey).metadata
        case .season: try await server.children(p.ratingKey).metadata
        case .playlist: try await server.playlistItems(p.ratingKey, start: 0, size: 5000).metadata
        }
    }

    /// Make the downloads match the policies of the connected account and server.
    @discardableResult
    public static func reconcile(_ app: AppState) async throws -> Report {
        var report = Report()
        guard let session = app.session() else { return report }
        let r = run(app)
        if r.running { return report }
        r.running = true
        defer { r.running = false }
        r.last = .now
        let paths = app.paths

        let scope = Scope.of(session)
        let saved = load(paths)
        let mine = saved.policies.filter { inScope($0, scope) }
        let known = Set(mine.map(\.id))
        let offlineWatched = Offline.watchedOffline(paths)

        var items: [(Policy, [Metadata])] = []
        for p in mine where p.enabled {
            do {
                items.append((p, watchOrder(p.target, try await fetchItems(session.server, p))))
            } catch {
                // Never remove anything on a guess: this policy sits the round out.
                report.errors.append("\(p.title): \(errorText(error))")
            }
        }
        // The session may have changed while the server was asked.
        guard let now = app.session(), sameServer(now.server, session.server) else { return report }

        let all = Downloads.list(paths)
        let (held, outside) = split(all, scope)
        let fetched = items.map { Fetched(policy: $0.0, candidates: candidates($0.1, scope, offlineWatched)) }
        let plan = evaluate(fetched, held, known, outside, saved.budget)
        report.overBudget = plan.overBudget

        if !plan.assign.isEmpty { Downloads.assign(paths, plan.assign.map { ($0.id, $0.policy) }) }
        let np = app.nowPlaying()
        let playing = np.flatMap { $0.local ? $0.item.ratingKey : nil }
        for id in plan.remove {
            // A file being played is cleaned up on a later round.
            if all.contains(where: { $0.id == id && playing == $0.item.ratingKey }) { continue }
            Downloads.remove(app, id)
            report.removed += 1
        }
        var byPolicy: [(String, [Metadata])] = []
        let everything = items.flatMap(\.1)
        for pair in plan.add {
            guard let item = everything.first(where: { Metadata.ratingKey($0) == pair.ratingKey }) else { continue }
            if let i = byPolicy.firstIndex(where: { $0.0 == pair.policy }) {
                byPolicy[i].1.append(item)
            } else {
                byPolicy.append((pair.policy, [item]))
            }
        }
        for (policy, list) in byPolicy {
            report.added += Downloads.add(app, session, list, policy)
        }
        if report.added + report.removed + plan.assign.count > 0 { notify(app) }
        return report
    }

    /// Reconcile shortly, coalescing requests; no more than once every 20 seconds.
    public static func reconcileSoon(_ app: AppState) {
        let r = run(app)
        if r.pending { return }
        r.pending = true
        let since = r.last.map { ContinuousClock.now - $0 } ?? .seconds(3600)
        let wait = max(max(.seconds(20) - since, .zero), .seconds(2))
        app.spawn {
            try await Task.sleep(for: wait)
            r.pending = false
            do {
                let report = try await reconcile(app)
                for e in report.errors { log.debug("smart downloads: \(e)") }
            } catch {
                log.debug("smart downloads: \(errorText(error))")
            }
        }
    }

    /// For the app's `BGAppRefreshTask` (`io.github.madrazaldi.patina.smart-downloads`): wait
    /// for the launch to connect, reconcile, and return once the queued downloads are handed to
    /// the background session (or there is nothing to fetch). The app schedules the next refresh.
    public static func backgroundRefresh(_ app: AppState) async {
        let started = ContinuousClock.now
        func waiting(_ limit: Duration) -> Bool { ContinuousClock.now - started < limit }
        // A launch into the background is still restoring the session.
        while app.session() == nil && waiting(.seconds(15)) {
            switch app.stage() {
            case .booting, .connecting: try? await Task.sleep(for: .milliseconds(250))
            default: return
            }
        }
        guard app.session() != nil else { return }
        do {
            let report = try await reconcile(app)
            for e in report.errors { log.debug("smart downloads: \(e)") }
        } catch {
            log.debug("smart downloads: \(errorText(error))")
        }
        Downloads.resume(app)
        let w = Downloads.worker(app)
        while w.running && !w.handedOff && waiting(.seconds(25)) {
            try? await Task.sleep(for: .milliseconds(250))
        }
    }

    // MARK: Managing policies

    /// A policy as front ends show it.
    @dynamicMemberLookup
    public nonisolated struct PolicyView: Sendable, Equatable {
        public var policy: Policy
        /// Belongs to the connected account and server (others wait until you switch to them).
        public var active: Bool
        /// Files this policy fetched, and their size.
        public var downloads: Int
        public var bytes: Int

        public subscript<T>(dynamicMember key: KeyPath<Policy, T>) -> T {
            policy[keyPath: key]
        }
    }

    public nonisolated struct Overview: Sendable, Equatable {
        public var policies: [PolicyView]
        /// Storage budget in bytes; `nil` when unlimited.
        public var budget: Int?
        /// Bytes of every download on this device.
        public var used: Int
    }

    public static func overview(_ app: AppState) -> Overview {
        let saved = load(app.paths)
        let all = Downloads.list(app.paths)
        let scope = app.session().map(Scope.of)
        let policies = saved.policies.map { policy in
            var n = 0
            var bytes = 0
            for d in all where d.policy == policy.id {
                n += 1
                bytes += d.size
            }
            return PolicyView(policy: policy, active: scope.map { inScope(policy, $0) } ?? false, downloads: n, bytes: bytes)
        }
        return Overview(policies: policies, budget: saved.budget, used: all.reduce(0) { $0 + max($1.size, 0) })
    }

    static func targetOf(_ item: Metadata) throws -> Target {
        switch Metadata.kind(item) {
        case "show": .show
        case "season": .season
        case "playlist": .playlist
        default: throw CoreError("Smart downloads work on shows, seasons and playlists.")
        }
    }

    static func titleOf(_ item: Metadata) -> String {
        Metadata.kind(item) == "season" ? "\(item.parentTitle ?? "Show") · \(Metadata.title(item))" : Metadata.title(item)
    }

    static func checkKeep(_ keep: Int) throws {
        if keep < 1 || keep > MAX_KEEP { throw CoreError("Keep between 1 and \(MAX_KEEP) items.") }
    }

    /// Keep the next `keep` unwatched items of a show, season or playlist downloaded.
    /// Replaces the settings of an existing policy for it.
    public static func setPolicy(_ app: AppState, _ ratingKey: String, _ keep: Int, _ deleteWatched: Bool) async throws
        -> Policy
    {
        try checkKeep(keep)
        guard let session = app.session() else { throw CoreError("Not connected.") }
        guard let item = try await session.server.metadata(ratingKey) else { throw CoreError("Item not found.") }
        let target = try targetOf(item)
        let scope = Scope.of(session)
        let fresh = Policy(
            id: UUID().uuidString.lowercased(), account: scope.account, serverId: scope.server,
            serverName: session.serverName, ratingKey: ratingKey, target: target, title: titleOf(item), keep: keep,
            deleteWatched: deleteWatched, enabled: true)
        let policy = try update(app.paths) { saved -> Policy in
            if let i = saved.policies.firstIndex(where: { inScope($0, scope) && $0.ratingKey == ratingKey }) {
                saved.policies[i].keep = keep
                saved.policies[i].deleteWatched = deleteWatched
                saved.policies[i].enabled = true
                saved.policies[i].title = fresh.title
                return saved.policies[i]
            }
            saved.policies.append(fresh)
            return fresh
        }
        notify(app)
        reconcileSoon(app)
        return policy
    }

    /// What to change in a policy; unset fields stay.
    public nonisolated struct Patch: Sendable, Equatable {
        public var keep: Int?
        public var deleteWatched: Bool?
        public var enabled: Bool?

        public init(keep: Int? = nil, deleteWatched: Bool? = nil, enabled: Bool? = nil) {
            self.keep = keep
            self.deleteWatched = deleteWatched
            self.enabled = enabled
        }
    }

    public static func updatePolicy(_ app: AppState, _ id: String, _ patch: Patch) throws {
        if let keep = patch.keep { try checkKeep(keep) }
        let found = try update(app.paths) { saved -> Bool in
            guard let i = saved.policies.firstIndex(where: { $0.id == id }) else { return false }
            saved.policies[i].keep = patch.keep ?? saved.policies[i].keep
            saved.policies[i].deleteWatched = patch.deleteWatched ?? saved.policies[i].deleteWatched
            saved.policies[i].enabled = patch.enabled ?? saved.policies[i].enabled
            return true
        }
        if !found { throw CoreError("That policy no longer exists.") }
        notify(app)
        reconcileSoon(app)
    }

    /// Delete a policy. Its downloads stay as ordinary downloads, or are removed too with
    /// `deleteDownloads`.
    public static func removePolicy(_ app: AppState, _ id: String, _ deleteDownloads: Bool) throws {
        try update(app.paths) { saved in saved.policies.removeAll { $0.id == id } }
        let owned = Downloads.list(app.paths).filter { $0.policy == id }.map(\.id)
        if deleteDownloads {
            for download in owned { Downloads.remove(app, download) }
        } else {
            Downloads.assign(app.paths, owned.map { ($0, nil) })
        }
        notify(app)
    }

    /// Limit all downloads together to `bytes`; `nil` removes the limit.
    public static func setBudget(_ app: AppState, _ bytes: Int?) throws {
        try update(app.paths) { saved in saved.budget = bytes.flatMap { $0 > 0 ? $0 : nil } }
        notify(app)
        reconcileSoon(app)
    }
}
