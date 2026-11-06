// Port of desktop/core/player.ts.
// Playback through the app's `PlaybackEngine`.
//
// A play request becomes a Plex play queue. On the desktop every queue item was appended to
// mpv's playlist and mpv owned next/previous, repeat and the end of an item. Here the queue is
// ours: one item at a time is loaded into the engine, and next/previous, continuing at the end
// of an item, repeat, "stop after this" and the sleep timer are decided below. We report
// timeline progress, mark items watched, mirror audio/subtitle choices between the engine and
// Plex, and offer intro/credits skipping from Plex markers (the screen draws the skip button
// from `NowPlaying.marker` instead of mpv's OSD).
//
// The queue can be edited while it plays (play next, remove, reorder, shuffle). Edits are
// mirrored to the Plex play queue where it has the items.
//
// The engine's events, commands, answers to background requests and two timers all feed one
// inbox that the playback loop drains one message at a time, so a handler runs to completion
// before the next message is looked at. The player's state is kept per `AppState`
// (module-level in TypeScript).

import Foundation
import PlexKit

nonisolated private let log = logger("player")

/// The intro or credits in view, which the screen offers to skip (`Commands.skipMarker`).
public nonisolated struct SkipMarker: Sendable, Equatable {
    /// `intro` or `credits`.
    public var kind: String
    public var startMs: Int
    public var endMs: Int
    /// Skipping moves on to the next item (the final credits with something queued after, or
    /// "stop after this"), rather than seeking past the marker.
    public var skipsToNext: Bool

    public init(kind: String, startMs: Int, endMs: Int, skipsToNext: Bool) {
        self.kind = kind
        self.startMs = startMs
        self.endMs = endMs
        self.skipsToNext = skipsToNext
    }
}

public enum Player {
    // MARK: Requests

    /// What to play.
    public nonisolated struct PlayRequest: Sendable, Equatable {
        public var item: Metadata
        /// Explicit bulk selection; disables automatic episode expansion.
        public var selection: [Metadata]
        /// Resume from the saved position (otherwise start from the beginning).
        public var resume: Bool
        public var shuffle: Bool
        /// Which version (media) of the starting item to play; `nil` lets the playback preferences pick.
        public var mediaIndex: Int?
        /// Play the item's radio station instead of the item (music).
        public var radio: Bool
        /// A tuned Live TV session; always transcoded, never scrobbled.
        public var live: Bool
        /// Quality preset id or label for this playback; the settings' otherwise.
        public var quality: String?

        public init(
            item: Metadata = Metadata(), selection: [Metadata] = [], resume: Bool = true, shuffle: Bool = false,
            mediaIndex: Int? = nil, radio: Bool = false, live: Bool = false, quality: String? = nil
        ) {
            self.item = item
            self.selection = selection
            self.resume = resume
            self.shuffle = shuffle
            self.mediaIndex = mediaIndex
            self.radio = radio
            self.live = live
            self.quality = quality
        }
    }

    /// Remote-control commands for the running player.
    public nonisolated enum Control: Sendable, Equatable {
        case togglePause
        case play
        case pause
        case next
        case previous
        case stop
        case seekRelative(seconds: Double)
        case seekTo(seconds: Double)
    }

    /// Edits to the running queue. Entries are addressed by the `uid` sent in `QueueView`, which
    /// stays stable while the queue is reordered.
    public nonisolated enum QueueCommand: Sendable, Equatable {
        /// Queue an item (or every item of a container) next or at the end.
        case add(item: Metadata, next: Bool)
        case remove(uid: Int)
        /// Move an entry before another one, or to the end.
        case move(uid: Int, before: Int?)
        case jump(uid: Int)
        /// Shuffle what comes after the current item, or restore its order.
        case shuffle(on: Bool)
        /// Quit when the current item ends instead of moving on.
        case stopAfter(on: Bool)
    }

    /// A track chosen in the player screen; ids are the engine's (`EngineTrack.id`).
    public nonisolated enum TrackChoice: Sendable, Equatable {
        case audio(Int)
        case subtitle(EngineSubtitle?)
    }

    // MARK: State

    /// Everything the playback loop waits for.
    nonisolated enum Msg: Sendable {
        /// An engine event, tagged with the load it belongs to.
        case event(EngineEvent, gen: Int)
        case control(Control)
        case queue(QueueCommand)
        case tune(Controls.Tune)
        case skipMarker
        case selectTrack(TrackChoice)
        case `internal`(Internal)
        case tick
        case stateTick
    }

    /// Results of requests made off the loop, so slow answers never hold up the engine's events.
    nonisolated enum Internal: Sendable {
        /// The full item (markers, streams, chapters) of a queue entry; `timedOut` when playback
        /// should not wait for it any longer.
        case detail(uid: Int, item: Metadata?, timedOut: Bool)
        /// The playable items behind something added to the queue.
        case leaves(source: Metadata, next: Bool, result: Result<[Metadata], CoreError>)
        /// Plex's play queue after items were added to it.
        case added(uids: [Int], queue: MediaContainer?)
        /// The items of the Plex play queue, to find what follows the end of the local one.
        case upcoming([Metadata])
        /// The engine could not start the load `gen` (and did not report `.failed`).
        case loadFailed(gen: Int, error: any Error)
    }

    /// A playback loop: the queue's (`Run`) or the downloads' (`FilesRun`).
    protocol Active: AnyObject {
        var inbox: Channel<Msg> { get }
        /// Closed when the loop is over.
        var ended: Channel<Void> { get }
        /// Queue edits go to this loop (not to a downloads playback).
        var acceptsQueue: Bool { get }
        /// A `.tick` / `.stateTick` from `ticker` is waiting in the inbox.
        var tickQueued: Bool { get set }
        var stateTickQueued: Bool { get set }
    }

    final class State {
        var engine: (any PlaybackEngine)?
        var active: (any Active)?
        var nextUid = 1
        /// Bumped for every load; engine events carry the value it had when they were sent.
        var engineGen = 0
        /// A position to start the next playback's first item from (retry, resuming a saved queue).
        var startOffset: (String, Int)?
    }

    static func state(_ app: AppState) -> State {
        app.local(State.self) { State() }
    }

    /// Give the core the app's engine (one for the app's lifetime). Its events and the lock
    /// screen's commands go to whatever is playing.
    public static func attach(_ app: AppState, _ engine: any PlaybackEngine) {
        let s = state(app)
        s.engine = engine
        engine.onEvent = { [weak s] event in
            guard let s else { return }
            s.active?.inbox.send(.event(event, gen: s.engineGen))
        }
        engine.onRemoteCommand = { [weak app] command in
            guard let app else { return }
            control(app, remoteControl(command))
        }
    }

    /// The `control` command a lock screen or headphone command stands for.
    nonisolated static func remoteControl(_ c: RemoteCommand) -> Control {
        switch c {
        case .play: .play
        case .pause: .pause
        case .togglePause: .togglePause
        case .next: .next
        case .previous: .previous
        case .stop: .stop
        case .seek(let to): .seekTo(seconds: to)
        case .skip(let by): .seekRelative(seconds: by)
        }
    }

    /// Whether a player is running.
    public static func isActive(_ app: AppState) -> Bool {
        state(app).active != nil
    }

    static func uid(_ app: AppState) -> Int {
        let s = state(app)
        defer { s.nextUid += 1 }
        return s.nextUid
    }

    // MARK: Commands

    public static func play(_ app: AppState, _ req: PlayRequest) {
        Failure.clear(app)
        // The account and server this playback belongs to, for a failure reported after a switch.
        let origin = app.session().map(Scope.of)
        app.spawn {
            do {
                try await run(app, req, origin)
            } catch is CancellationError {
                return
            } catch {
                log.error("playback failed: \(errorText(error))")
                app.setNowPlaying(nil)
                app.setQueue(nil)
                Controls.finished(app)
                // Nothing was playing yet (no server, no queue, no engine): offer to try again.
                let (kind, message) = Failure.explainError(error)
                Failure.record(
                    app,
                    Failure.Report(
                        item: req.item, error: message, kind: kind, phase: .start, positionMs: 0,
                        mediaIndex: req.mediaIndex ?? 0,
                        quality: req.quality ?? Config.playerQuality(app.settings().player).id,
                        remaining: max(0, req.selection.count - 1), request: req, scope: origin))
            }
        }
    }

    /// Play, starting the first item at `offsetMs` instead of the position the server remembers.
    public static func playFrom(_ app: AppState, _ req: PlayRequest, _ offsetMs: Int) {
        if offsetMs > 0 { state(app).startOffset = (Metadata.ratingKey(req.item), offsetMs) }
        play(app, req)
    }

    /// Drive the running player; nothing happens when nothing plays.
    public static func control(_ app: AppState, _ cmd: Control) {
        state(app).active?.inbox.send(.control(cmd))
    }

    /// The running queue's loop, if it is still going.
    static func liveCommands(_ app: AppState) -> Channel<Msg>? {
        guard let a = state(app).active, a.acceptsQueue, !a.inbox.closed else { return nil }
        return a.inbox
    }

    /// Enqueue a selection together, including when no player is running yet.
    public static func queueItems(_ app: AppState, _ items: [Metadata]) throws {
        guard let first = items.first else { return }
        guard items.allSatisfy({ queueKind($0) == queueKind(first) }) else {
            throw CoreError("Select only video or only music for a queue.")
        }
        if let tx = liveCommands(app) {
            for item in items where !tx.send(.queue(.add(item: item, next: false))) {
                throw CoreError("Playback ended; try again.")
            }
        } else {
            play(app, PlayRequest(item: first, selection: items))
        }
    }

    /// Edit the running queue. Adding with nothing playing starts playback.
    public static func queue(_ app: AppState, _ cmd: QueueCommand) {
        if let tx = liveCommands(app) {
            tx.send(.queue(cmd))
        } else if case .add(let item, _) = cmd {
            play(app, PlayRequest(item: item))
        }
    }

    /// Change volume, speed, chapter, repeat or the sleep timer of the running player.
    public static func tune(_ app: AppState, _ cmd: Controls.Tune) {
        state(app).active?.inbox.send(.tune(cmd))
    }

    /// Skip the intro or credits in view (`NowPlaying.marker`).
    public static func skipMarker(_ app: AppState) {
        state(app).active?.inbox.send(.skipMarker)
    }

    /// Switch the audio or subtitle track, and save the choice to Plex when streams are synced.
    public static func selectTrack(_ app: AppState, _ choice: TrackChoice) {
        state(app).active?.inbox.send(.selectTrack(choice))
    }

    /// Stop playback when Patina quits or moves away for good, and wait (a few seconds at most)
    /// for the last reports to reach Plex.
    public static func shutdown(_ app: AppState) async {
        if let a = state(app).active {
            a.inbox.send(.control(.stop))
            let ended = Task { await a.ended.recv() }
            let timer = Task {
                try? await Task.sleep(for: .seconds(3))
                ended.cancel()
            }
            _ = await ended.value
            timer.cancel()
        }
        await Report.flush(5000)
    }

    // MARK: Queues

    struct Queue {
        var items: [Metadata]
        var start: Int
        var kind: String
        var id: Int?
    }

    public nonisolated static func queueKind(_ item: Metadata) -> String {
        switch Metadata.kind(item) {
        case "track", "album", "artist": "audio"
        case "photo", "photoalbum": "photo"
        case "playlist":
            switch item.playlistType {
            case "audio": "audio"
            case "photo": "photo"
            default: "video"
            }
        default: "video"
        }
    }

    static func buildQueue(_ server: PlexServer, _ req: PlayRequest, _ settings: PlayerSettings) async throws -> Queue {
        let item = req.item
        if req.live { return Queue(items: [item], start: 0, kind: "video", id: nil) }
        let rk = Metadata.ratingKey(item)
        let kind = queueKind(item)
        var pq = PlayQueueRequest(kind: kind, shuffle: req.shuffle)
        if !req.selection.isEmpty {
            pq.uri = server.itemUri(req.selection.map(Metadata.ratingKey))
        } else if req.radio {
            if let station = (try? await server.station(rk))?.key {
                pq.uri = server.libraryUri(station)
            } else {
                // No sonic analysis: shuffle the artist instead.
                let artist: String? =
                    switch Metadata.kind(item) {
                    case "track": item.grandparentRatingKey
                    case "album": item.parentRatingKey
                    default: rk
                    }
                pq.uri = server.itemUri([artist ?? rk])
                pq.shuffle = true
            }
            pq.kind = "audio"
        } else {
            switch Metadata.kind(item) {
            case "playlist":
                pq.playlistId = rk
            case "collection":
                let children = try await server.collectionItems(rk).metadata
                pq.uri = server.itemUri(children.map(Metadata.ratingKey))
            case "show", "season", "artist", "album":
                pq.uri = server.itemUri([rk])
                if !req.shuffle { pq.key = Metadata.onDeckItem(item)?.key }
            default:
                pq.uri = server.itemUri([rk])
                pq.key = item.key
                pq.continuous = Metadata.kind(item) == "episode" && settings.continuous
            }
        }

        var mc: MediaContainer?
        var err: (any Error)?
        do {
            mc = try await server.createPlayQueue(pq)
        } catch {
            err = error
        }
        if let mc, !mc.metadata.isEmpty {
            // An explicit selection starts at the requested item; otherwise Plex says where.
            var start =
                !req.selection.isEmpty && item.ratingKey != nil
                ? mc.metadata.firstIndex { $0.ratingKey == item.ratingKey } : nil
            if start == nil {
                start = mc.metadata.firstIndex {
                    $0.playQueueItemID != nil && $0.playQueueItemID == mc.playQueueSelectedItemID
                }
            }
            return Queue(items: mc.metadata, start: start ?? 0, kind: kind, id: mc.playQueueID)
        }
        if !req.selection.isEmpty {
            var items: [Metadata] = []
            for it in req.selection { items += try await leaves(server, it) }
            if items.isEmpty { throw CoreError("nothing to play") }
            return Queue(items: items, start: 0, kind: kind, id: nil)
        }
        if Metadata.isPlayable(item) { return Queue(items: [item], start: 0, kind: kind, id: nil) }
        guard let err else { throw CoreError("nothing to play") }
        throw chained(err, "creating play queue")
    }

    /// The playable items of an item: itself, or everything below a container.
    static func leaves(_ server: PlexServer, _ item: Metadata) async throws -> [Metadata] {
        if Metadata.isPlayable(item) { return [item] }
        let rk = Metadata.ratingKey(item)
        let children: [Metadata]
        switch Metadata.kind(item) {
        case "playlist":
            children = try await server.playlistItems(rk, start: 0, size: 5000).metadata
        case "collection":
            children = try await server.collectionItems(rk).metadata
        default:
            return try await server.allLeaves(rk).metadata
        }
        var out: [Metadata] = []
        for child in children {
            if Metadata.isPlayable(child) {
                out.append(child)
            } else {
                out += try await server.allLeaves(Metadata.ratingKey(child)).metadata
            }
        }
        return out
    }

    nonisolated static func partOf(_ item: Metadata, _ mediaIndex: Int) -> Part? {
        (item.media.indices.contains(mediaIndex) ? item.media[mediaIndex] : item.media.first)?.part.first
    }

    /// Whether `item` plays through the transcoder rather than from its file.
    nonisolated static func transcoded(_ item: Metadata, _ quality: Quality, _ live: Bool) -> Bool {
        live || (quality.kbps != nil && ["movie", "episode", "clip", "trailer"].contains(Metadata.kind(item)))
    }

    struct Loaded {
        var url: URL
        var transcodeSession: String?
    }

    /// Where the engine reads `item`: its file (direct play, the token in the headers), or an
    /// HLS stream from Plex's transcoder.
    static func mediaUrl(_ server: PlexServer, _ item: Metadata, _ mediaIndex: Int, _ quality: Quality, _ live: Bool)
        throws -> Loaded
    {
        if transcoded(item, quality, live) {
            let session = UUID().uuidString.lowercased()
            guard let key = item.key else { throw CoreError("item has no key") }
            let url = try server.transcodeUrl(
                key,
                options: TranscodeOptions(
                    session: session, mediaIndex: mediaIndex, maxVideoBitrate: quality.kbps,
                    videoResolution: quality.kbps != nil ? quality.resolution : nil, subtitles: "burn",
                    protocol: "hls"))
            return Loaded(url: url, transcodeSession: session)
        }
        guard let part = partOf(item, mediaIndex) else {
            throw CoreError("\"\(Metadata.title(item))\" has no playable file")
        }
        guard let key = part.key else { throw CoreError("media part has no key") }
        return Loaded(url: try server.url(key), transcodeSession: nil)
    }

    /// What the lock screen shows for `item`.
    static func card(_ server: PlexServer?, _ item: Metadata) -> NowPlayingCard {
        let subtitle: String? =
            switch Metadata.kind(item) {
            case "episode": "\(item.grandparentTitle ?? "") · \(Metadata.episodeCode(item))"
            case "track": [item.grandparentTitle, item.parentTitle].compactMap { $0 }.joined(separator: " — ")
            default: item.year.map(String.init)
            }
        let artwork = Metadata.poster(item).flatMap { try? server?.imageUrl($0, width: 600, height: 600) }
        return NowPlayingCard(
            title: Metadata.title(item), subtitle: subtitle, artwork: artwork,
            durationSeconds: item.duration.map { Double($0) / 1000 })
    }

    // MARK: The queue's playback loop

    /// One entry of the queue, in play order.
    struct Entry {
        var uid: Int
        var item: Metadata
        var mediaIndex: Int
        var url: URL
        var transcodeSession: String?
    }

    /// Per-item playback state.
    final class Current {
        let uid: Int
        var item: Metadata
        let mediaIndex: Int
        let url: URL
        let transcodeSession: String?
        /// Where the engine is told to start.
        let startMs: Int
        var timeMs = 0
        var durationMs: Int
        var paused = false
        var reachedEnd = false
        /// Ignore track changes until this instant (we are applying Plex's choice).
        var applyingUntil = ContinuousClock.now + .seconds(3)
        /// `EngineLoad.externalSubtitles` index -> Plex stream id.
        var sidecars: [Int: Int] = [:]
        var activeMarker: Marker?
        var promptedMarker: Int?
        /// In the closing credits or last seconds, with something queued after.
        var ending = false
        var lastUi = ContinuousClock.now - .seconds(5)
        /// The engine load this item is (`State.engineGen`), once it was asked for.
        var gen: Int?
        var loadStarted = false
        /// The engine opened the item.
        var loaded = false
        /// The full item (markers, streams, chapters) is still being fetched.
        var detailPending: Bool
        /// The engine opened the item before the full item arrived: apply Plex's streams once it does.
        var streamsPending = false
        /// The tracks the engine plays, as last seen (to notice changes worth saving to Plex).
        var audio: Int?
        var subtitle: EngineSubtitle?
        /// When the position was last saved locally.
        var checkpointAt = ContinuousClock.now

        /// The state of an item as it starts: from the queue entry alone, since the full item is
        /// fetched afterwards.
        init(_ e: Entry, startMs: Int) {
            uid = e.uid
            item = e.item
            mediaIndex = e.mediaIndex
            url = e.url
            transcodeSession = e.transcodeSession
            self.startMs = startMs
            timeMs = startMs
            durationMs = e.item.duration ?? 0
            detailPending = e.item.ratingKey != nil && Metadata.kind(e.item) != "photo"
        }
    }

    final class Run: Active {
        let app: AppState
        let server: PlexServer
        /// Machine id of `server` when it is not the session's own; queue items are tagged with it.
        let serverTag: String?
        let scope: Scope
        /// The account and connected server when playback started (`scope` is the item's server).
        let origin: Scope?
        let settings: PlayerSettings
        let quality: Quality
        let engine: any PlaybackEngine
        let kind: String
        let live: Bool
        let playQueueId: Int?
        let inbox = Channel<Msg>()
        let ended = Channel<Void>()
        var acceptsQueue: Bool { true }
        var entries: [Entry] = []
        var cur: Current?
        /// Entry order before shuffling, to restore it.
        var unshuffled: [Int]?
        var stopAfter = false
        /// "Stop after this" was turned on by the sleep timer's "end of item".
        var sleepItem = false
        var playedAny = false
        /// Volume, speed, chapters and diagnostics, published as they change.
        let tracker: Controls.Tracker
        /// The last item ended with nothing after it.
        var finishedQueue = false
        /// Items that failed one after the other.
        var failuresInARow = 0
        /// Playback of this rating key restarted from a local checkpoint at this position.
        var recovered: (String, Int)?
        /// Where the first item was told to start, for a failure before it loaded.
        var initialStart: (String, Int)?
        /// More of the Plex queue is being fetched.
        var extending = false
        /// Playback is over (mpv's `quit`): the loop ends after the message being handled.
        var quitting = false
        /// A timer message is waiting in the inbox.
        var tickQueued = false
        var stateTickQueued = false

        init(
            app: AppState, server: PlexServer, serverTag: String?, scope: Scope, origin: Scope?,
            settings: PlayerSettings, quality: Quality, engine: any PlaybackEngine, kind: String, live: Bool,
            playQueueId: Int?, tracker: Controls.Tracker
        ) {
            self.app = app
            self.server = server
            self.serverTag = serverTag
            self.scope = scope
            self.origin = origin
            self.settings = settings
            self.quality = quality
            self.engine = engine
            self.kind = kind
            self.live = live
            self.playQueueId = playQueueId
            self.tracker = tracker
        }

        private func post(_ msg: Internal) {
            inbox.send(.internal(msg))
        }

        var reports: Bool {
            kind != "photo" && !live
        }

        func indexOf(_ uid: Int) -> Int? {
            entries.firstIndex { $0.uid == uid }
        }

        var curIndex: Int? {
            cur.flatMap { indexOf($0.uid) }
        }

        var hasNext: Bool {
            guard let i = curIndex else { return false }
            return i + 1 < entries.count
        }

        func hasNextAfter(_ uid: Int) -> Bool {
            guard let i = indexOf(uid) else { return false }
            return i + 1 < entries.count
        }

        /// Add an item to the queue at `index` (the end when `nil`).
        @discardableResult
        func insert(_ itemIn: Metadata, _ mediaIndex: Int, _ index: Int?) throws -> Int {
            var item = itemIn
            Metadata.setServerId(&item, serverTag)
            let loaded = try mediaUrl(server, item, mediaIndex, quality, live)
            let entry = Entry(
                uid: uid(app), item: item, mediaIndex: mediaIndex, url: loaded.url,
                transcodeSession: loaded.transcodeSession)
            if let index, index < entries.count {
                entries.insert(entry, at: index)
            } else {
                entries.append(entry)
            }
            return entry.uid
        }

        /// Save the automatic track choice of queued items that will be transcoded, in the
        /// background: the transcoder plays the server's selection when the item gets its turn.
        func saveTracksLater(_ itemsIn: [(Metadata, Int)]) {
            let items = itemsIn.filter { !live && transcoded($0.0, quality, false) }
            guard !items.isEmpty else { return }
            let (scope, server, paths) = (scope, server, app.paths)
            app.spawn {
                // `for_each_concurrent(4, ..)`
                try await Ops.Util.eachCompleted(
                    items.map { Pair(item: $0.0, mediaIndex: $0.1) }, 4,
                    { p in
                        do {
                            try await Prefs.saveAuto(paths, scope, server, p.item, p.mediaIndex)
                        } catch {
                            log.debug("could not save automatic track choice: \(errorText(error))")
                        }
                    }, { _, _ in })
            }
        }

        func emitQueue() {
            app.setQueue(
                QueueView(
                    items: entries.map { QueueItem(uid: $0.uid, item: $0.item) }, current: cur?.uid,
                    shuffled: unshuffled != nil, stopAfter: stopAfter))
            let i = curIndex
            let wraps = Controls.repeat(app) == .all && !entries.isEmpty
            engine.setRemoteQueue(
                hasPrevious: (i ?? 0) > 0 || wraps, hasNext: i.map { $0 + 1 < entries.count } ?? false || wraps)
        }

        func marker(_ c: Current) -> SkipMarker? {
            guard let m = c.activeMarker, let start = m.startTimeOffset, let end = m.endTimeOffset else { return nil }
            let kind = m.type ?? "intro"
            return SkipMarker(
                kind: kind, startMs: start, endMs: end, skipsToNext: kind == "credits" && m.final && (hasNext || stopAfter))
        }

        func publish() {
            guard let c = cur else { return }
            c.lastUi = .now
            app.setNowPlaying(
                NowPlaying(
                    item: c.item, positionMs: c.timeMs, durationMs: c.durationMs, paused: c.paused,
                    transcoding: c.transcodeSession != nil, queueLen: entries.count, queuePos: curIndex ?? 0,
                    uid: c.uid, ending: c.ending, stopAfter: stopAfter, live: live, local: false, marker: marker(c)))
        }

        /// Tell Plex about the current state of an item. Never waits for the answer.
        func report(_ c: Current, _ state: TimelineState) {
            if let op = timelineAt(c, state, c.timeMs) { Report.reporter().send(server, op) }
        }

        /// Views refetch watch state once Plex has the last reports.
        func refreshAfterReports() {
            let app = app
            app.spawn {
                await Report.flush(20_000)
                app.bump()
            }
        }

        /// Save the local position of the playing item and the queue around it.
        func checkpoint() {
            guard reports, let c = cur else { return }
            c.checkpointAt = .now
            if c.timeMs > 0, let rk = c.item.ratingKey {
                do {
                    try Recovery.saveCheckpoint(app.paths, scope, rk, c.timeMs, c.durationMs)
                } catch {
                    log.debug("could not save the playback position: \(errorText(error))")
                }
            }
        }

        func persistQueue() {
            guard reports, let index = curIndex, let c = cur else { return }
            let (range, current) = Recovery.window(entries.count, index)
            let queue = SavedQueue(
                account: scope.account, server: scope.server, savedAt: Offline.now(), kind: kind,
                items: entries[range].map { Recovery.slim($0.item) }, current: current, positionMs: c.timeMs,
                quality: quality.id)
            do {
                try Recovery.saveQueue(app.paths, scope, queue)
            } catch {
                log.debug("could not save the queue: \(errorText(error))")
            }
        }

        // MARK: Moving through the queue

        /// Play the entry at `index` from `startMs`; `nil` (or an index past the end) only
        /// leaves the current item. Also restarts the current entry (repeat one).
        func go(_ index: Int?, startMs: Int = 0) {
            let old = cur
            cur = nil
            if let old { finish(old, false) }
            if stopAfter { setStopAfter(false) }
            guard let index, entries.indices.contains(index) else { return }
            let c = Current(entries[index], startMs: startMs)
            cur = c
            beginItemState()
            publish()
            emitQueue()
            if reports { report(c, .playing) }
            persistQueue()
            if index + 1 == entries.count { extendFromPlex() }
            if c.detailPending {
                // Queue entries lack markers and sometimes streams; fetch the full item, but
                // never hold up playback for long.
                let (server, key, uid) = (server, Metadata.ratingKey(c.item), c.uid)
                let inbox = inbox
                Task {
                    let item = try? await server.metadata(key, opts: .playback())
                    inbox.send(.internal(.detail(uid: uid, item: item, timedOut: false)))
                }
                Task {
                    try? await Task.sleep(for: .seconds(4))
                    inbox.send(.internal(.detail(uid: uid, item: nil, timedOut: true)))
                }
            } else {
                startLoad(c)
            }
        }

        /// Ask the engine to play `c`, with Plex's audio and subtitle choice (and the automatic
        /// playback preferences) unless it is transcoded.
        func startLoad(_ c: Current) {
            guard !c.loadStarted else { return }
            c.loadStarted = true
            c.applyingUntil = .now + .seconds(3)
            let info = server.clientInfo
            var request = EngineLoad(
                url: c.url, startSeconds: Double(c.startMs) / 1000,
                httpHeaders: [
                    "X-Plex-Token": server.token, "X-Plex-Client-Identifier": info.identifier,
                    "X-Plex-Product": info.product,
                ], audioOnly: kind == "audio", live: live, nowPlaying: card(server, c.item))
            if settings.syncStreams && c.transcodeSession == nil {
                Prefs.applyAuto(app, scope, server, &c.item, c.mediaIndex)
                applyPlexStreams(server, &request, c)
            }
            let s = state(app)
            let engine = engine
            let inbox = inbox
            Task { [weak self] in
                // Nothing else ran since `go`, unless the queue moved on: then this item is not loaded.
                guard let self, self.cur === c, !self.quitting else { return }
                s.engineGen += 1
                let gen = s.engineGen
                c.gen = gen
                do {
                    try await engine.load(request)
                } catch is CancellationError {
                    return
                } catch {
                    inbox.send(.internal(.loadFailed(gen: gen, error: error)))
                }
            }
        }

        /// The playing item's per-file state for the front ends: chapters from Plex and a
        /// recovered position.
        func beginItemState() {
            tracker.newItem(cur?.item.chapter ?? [])
            let c = cur
            let recovered = c.flatMap { c in
                self.recovered.flatMap { $0.0 == Metadata.ratingKey(c.item) ? $0.1 : nil }
            }
            tracker.recoveredFrom(recovered)
            tracker.publish(app, true)
        }

        /// The item played to its end or failed: what mpv's playlist would do next.
        func onEnd(_ c: Current, completed: Bool, failed: (String, EngineFailureKind?)?) {
            cur = nil
            c.reachedEnd = c.reachedEnd || completed
            let rep = Controls.repeat(app)
            if completed && !hasNextAfter(c.uid) && rep == .off { finishedQueue = true }
            if let failed { onFailure(c, failed.0, failed.1) }
            finish(c, completed)
            if quitting { return }
            if completed && (Controls.sleepAtItemEnd(app) || stopAfter) {
                quitting = true
                return
            }
            guard let i = indexOf(c.uid) else {
                quitting = true
                return
            }
            if completed && rep == .one {
                go(i)
            } else if i + 1 < entries.count {
                go(i + 1)
            } else if rep == .all {
                go(0)
            } else {
                // Playback is over once the queue is done, or after every item failed.
                quitting = true
            }
        }

        /// The engine gave up on an item: tell the front end why, and what can be done.
        func onFailure(_ c: Current, _ error: String?, _ engineKind: EngineFailureKind?) {
            failuresInARow += 1
            let upcoming = indexOf(c.uid).map { entries[$0...].map(\.item) } ?? []
            let remaining = max(0, upcoming.count - 1)
            // Where trying again picks up: the playhead, or for a file that never started, where it was meant to start.
            let position =
                c.loaded && c.timeMs > 0
                ? c.timeMs : initialStart.flatMap { $0.0 == Metadata.ratingKey(c.item) ? $0.1 : nil } ?? c.timeMs
            let request = PlayRequest(
                item: upcoming.first ?? c.item, selection: upcoming.count > 1 ? upcoming : [], resume: true,
                mediaIndex: c.mediaIndex, live: live, quality: quality.id)
            Failure.record(
                app,
                Failure.Report(
                    item: c.item, error: error, engineKind: engineKind ?? .other, phase: c.loaded ? .playing : .start,
                    positionMs: position, mediaIndex: c.mediaIndex, quality: quality.id, remaining: remaining,
                    request: request, scope: origin))
            // Everything failing in turn (or one item failing over and over under repeat-all) ends playback.
            if failuresInARow >= max(entries.count, 1) { quitting = true }
        }

        // MARK: Engine events

        func onEvent(_ ev: EngineEvent, _ gen: Int) async {
            // Events of an item the queue has moved on from.
            guard let c = cur, c.gen == gen else { return }
            switch ev {
            case .loaded(let duration, _, let chapters):
                playedAny = true
                failuresInARow = 0
                if let failed = Failure.current(app), failed.item.ratingKey != c.item.ratingKey {
                    // Playback moved on to something else that works.
                    Failure.clear(app)
                }
                c.loaded = true
                if let duration, duration > 0 { c.durationMs = Int(duration * 1000) }
                tracker.onChapters(chapters)
                // The engine starts each item at normal speed; the speed chosen carries over.
                if tracker.speed != 1 {
                    engine.setRate(tracker.speed)
                    tracker.speed = min(tracker.speed, max(engine.maxRate, 0.25))
                }
                c.applyingUntil = .now + .milliseconds(1500)
                if c.streamsPending {
                    c.streamsPending = false
                    await applyLateStreams(c)
                }
                tracker.publish(app, true)
                publish()
            case .time(let seconds):
                await onTime(Int(seconds * 1000))
            case .paused(let paused):
                guard paused != c.paused else { return }
                c.paused = paused
                publish()
                if reports { report(c, paused ? .paused : .playing) }
                checkpoint()
                persistQueue()
            case .diagnostics(let d):
                tracker.publish(app, tracker.onDiagnostics(d))
            case .audioTrack(let id):
                guard id != c.audio else { return }
                c.audio = id
                if let id, settings.syncStreams, c.transcodeSession == nil, ContinuousClock.now > c.applyingUntil {
                    syncStreamChoice(c, .audio(id))
                }
            case .subtitleTrack(let sub):
                guard sub != c.subtitle else { return }
                c.subtitle = sub
                if settings.syncStreams, c.transcodeSession == nil, ContinuousClock.now > c.applyingUntil {
                    syncStreamChoice(c, .subtitle(sub))
                }
            case .ended:
                onEnd(c, completed: true, failed: nil)
            case .failed(let message, let kind):
                onEnd(c, completed: false, failed: (message, kind))
            }
        }

        func onTime(_ t: Int) async {
            tracker.onTime(t)
            guard let c = cur else { return }
            let nextTitle = curIndex.flatMap { i in i + 1 < entries.count ? Metadata.fullTitle(entries[i + 1].item) : nil }
            let video = kind == "video" && !live
            c.timeMs = t
            let inCredits = c.item.marker.contains {
                $0.type == "credits" && $0.final && ($0.startTimeOffset.map { $0 <= t } ?? false)
            }
            let nearEnd = c.durationMs > 0 && c.durationMs - t < 30_000
            let ending = nextTitle != nil && video && (inCredits || nearEnd)
            let changed = ending != c.ending
            c.ending = ending
            let checkpointDue = ContinuousClock.now - c.checkpointAt >= .seconds(5)
            let markerChanged = await handleMarkers()
            let stale = cur.map { ContinuousClock.now - $0.lastUi >= .milliseconds(900) } ?? false
            if changed || markerChanged || stale { publish() }
            if checkpointDue { checkpoint() }
        }

        func tick() {
            guard let c = cur else { return }
            if reports { report(c, c.paused ? .paused : .playing) }
            if let session = c.transcodeSession { Report.reporter().send(server, .pingTranscode(session: session)) }
            persistQueue()
        }

        /// An item ended (or was left). Never waits for Plex: the reports go to the worker.
        func finish(_ c: Current, _ completed: Bool) {
            guard reports else { return }
            let progress = c.durationMs > 0 ? Double(c.timeMs) / Double(c.durationMs) : 0
            let watched = completed || c.reachedEnd || progress >= settings.watchedThreshold
            let timeMs = watched && c.durationMs > 0 ? c.durationMs : c.timeMs
            if let op = timelineAt(c, .stopped, timeMs) { Report.reporter().send(server, op) }
            if watched, let rk = c.item.ratingKey { Report.reporter().send(server, .scrobble(ratingKey: rk)) }
            if let rk = c.item.ratingKey {
                do {
                    if watched {
                        try Recovery.clearCheckpoint(app.paths, scope, rk)
                    } else {
                        try Recovery.saveCheckpoint(app.paths, scope, rk, c.timeMs, c.durationMs)
                    }
                } catch {
                    log.debug("could not update the playback position: \(errorText(error))")
                }
            }
            refreshAfterReports()
        }

        /// Follow the markers: which one is in view, and skip it when the settings say so.
        /// Returns whether the marker in view changed.
        func handleMarkers() async -> Bool {
            guard let c = cur else { return false }
            let t = c.timeMs
            let marker = c.item.marker.first {
                ($0.type == "intro" || $0.type == "credits") && ($0.startTimeOffset ?? Int.max) <= t
                    && t < ($0.endTimeOffset ?? 0) - 1000
            }
            let changed = marker?.id != c.activeMarker?.id || (marker == nil) != (c.activeMarker == nil)
            c.activeMarker = marker
            guard let marker, c.promptedMarker != marker.id else { return changed }
            c.promptedMarker = marker.id
            let auto = marker.type == "intro" ? settings.autoSkipIntro : settings.autoSkipCredits
            if auto { await skipMarker(marker) }
            return true
        }

        func skipMarker(_ m: Marker) async {
            let isCredits = m.type == "credits"
            guard let c = cur else { return }
            c.activeMarker = nil
            if isCredits && m.final && (hasNext || stopAfter) {
                c.reachedEnd = true
                if stopAfter {
                    quitting = true
                } else if let i = curIndex {
                    go(i + 1)
                }
                return
            }
            if let end = m.endTimeOffset {
                await engine.seek(toSeconds: Double(end) / 1000)
                publish()
            }
        }

        func setStopAfter(_ on: Bool) {
            stopAfter = on
            Controls.setStopAfter(app, on)
            if !on { sleepItem = false }
            publish()
            tracker.publish(app, true)
        }

        /// Big and endless queues (radio) arrive in windows; fetch what follows.
        func extendFromPlex() {
            guard let id = playQueueId, !extending else { return }
            extending = true
            let (server, inbox) = (server, inbox)
            Task {
                let items = (try? await server.playQueue(id, window: 50).metadata) ?? []
                inbox.send(.internal(.upcoming(items)))
            }
        }

        // MARK: Tracks

        /// Put Plex's selected streams into the load request: the audio track, the subtitle (an
        /// embedded stream or a sidecar file) and every sidecar subtitle.
        func applyPlexStreams(_ server: PlexServer, _ request: inout EngineLoad, _ c: Current) {
            guard let part = partOf(c.item, c.mediaIndex) else { return }
            if let audio = Part.audioStreams(part).first(where: \.selected), let index = audio.index {
                request.audioStreamIndex = index
            }
            let subs = Part.subtitleStreams(part)
            let selectedSub = subs.first(where: \.selected)
            for s in subs where MediaStream.isExternal(s) {
                guard let key = s.key, let id = s.id, let url = try? server.url(key) else { continue }
                request.externalSubtitles.append(
                    ExternalSubtitle(
                        url: url, title: MediaStream.shortLabel(s), language: s.languageCode, forced: s.forced,
                        hearingImpaired: s.hearingImpaired, format: s.codec ?? s.format))
                let at = request.externalSubtitles.count - 1
                c.sidecars[at] = id
                if selectedSub?.id == id { request.subtitle = .external(at) }
            }
            if let selectedSub, !MediaStream.isExternal(selectedSub), let index = selectedSub.index {
                request.subtitle = .stream(index)
            }
            c.audio = request.audioStreamIndex
            c.subtitle = request.subtitle
        }

        /// The full item arrived after the engine opened a stripped-down one: switch to the
        /// tracks Plex (and the playback preferences) chose. Sidecar files can't be added late.
        func applyLateStreams(_ c: Current) async {
            Prefs.applyAuto(app, scope, server, &c.item, c.mediaIndex)
            var request = EngineLoad(url: c.url)
            let sidecars = c.sidecars
            let (audio, subtitle) = (c.audio, c.subtitle)
            applyPlexStreams(server, &request, c)
            c.sidecars = sidecars
            c.applyingUntil = .now + .milliseconds(1500)
            if let index = request.audioStreamIndex, index != audio { await engine.selectAudio(streamIndex: index) }
            switch request.subtitle {
            case .stream, nil:
                if request.subtitle != subtitle { engine.selectSubtitle(request.subtitle) }
            case .external:
                c.subtitle = subtitle
            }
        }

        /// Switch a track because the user asked for it, and save the choice.
        func selectTrack(_ choice: TrackChoice) async {
            guard let c = cur else { return }
            switch choice {
            case .audio(let id):
                c.audio = id
                await engine.selectAudio(streamIndex: id)
            case .subtitle(let sub):
                c.subtitle = sub
                engine.selectSubtitle(sub)
            }
            if settings.syncStreams && c.transcodeSession == nil { syncStreamChoice(c, choice) }
        }

        /// Save a track choice made in the player back to Plex so other apps agree.
        func syncStreamChoice(_ c: Current, _ choice: TrackChoice) {
            guard let part = partOf(c.item, c.mediaIndex), let partId = part.id else { return }
            let streamId: Int?
            switch choice {
            case .audio(let index):
                streamId = part.stream.first { $0.streamType == StreamType.audio && $0.index == index }?.id
            case .subtitle(nil):
                streamId = 0
            case .subtitle(.stream(let index)):
                streamId = part.stream.first { $0.streamType == StreamType.subtitle && $0.index == index }?.id
            case .subtitle(.external(let i)):
                streamId = c.sidecars[i]
            }
            guard let streamId else { return }
            // Keep the item's own selection in step, for a reload (repeat, retry) of the same entry.
            let isAudio = if case .audio = choice { true } else { false }
            let pick =
                isAudio
                ? Prefs.Choice(audio: streamId) : Prefs.Choice(subtitle: streamId == 0 ? .off : .track(streamId))
            select(&c.item, c.mediaIndex, pick)
            if let i = indexOf(c.uid) { select(&entries[i].item, entries[i].mediaIndex, pick) }
            Report.reporter().send(
                server, .streams(partId: partId, audio: isAudio ? streamId : nil, subtitle: isAudio ? nil : streamId))
        }

        private func select(_ item: inout Metadata, _ mediaIndex: Int, _ choice: Prefs.Choice) {
            let mi = item.media.indices.contains(mediaIndex) ? mediaIndex : 0
            guard item.media.indices.contains(mi), !item.media[mi].part.isEmpty else { return }
            _ = Prefs.applyChoice(&item.media[mi].part[0], choice)
        }

        // MARK: Answers and commands

        func onInternal(_ msg: Internal) async {
            switch msg {
            case .detail(let uid, let full, let timedOut):
                guard let c = cur, c.uid == uid else { return }
                if timedOut {
                    // Playback starts with what the queue entry has; the details follow if they arrive.
                    if c.detailPending && !c.loadStarted { startLoad(c) }
                    return
                }
                guard c.detailPending else { return }
                c.detailPending = false
                if var full {
                    full.playQueueItemID = c.item.playQueueItemID
                    // The server's answer carries no Patina server tag; keep the one the entry had.
                    Metadata.setServerId(&full, serverTag)
                    if c.durationMs == 0 { c.durationMs = full.duration ?? 0 }
                    tracker.setPlexChapters(full.chapter)
                    c.item = full
                    if c.loadStarted && settings.syncStreams && c.transcodeSession == nil {
                        if c.loaded {
                            await applyLateStreams(c)
                        } else {
                            c.streamsPending = true
                        }
                    }
                }
                startLoad(c)
                publish()
            case .leaves(let source, let next, let result):
                var failed: String?
                switch result {
                case .success(let items):
                    do {
                        try addItems(source, items, next)
                    } catch {
                        failed = errorText(error)
                    }
                case .failure(let e):
                    failed = e.message
                }
                if let failed { app.error("Could not change the queue: \(failed)") }
                emitQueue()
                persistQueue()
            case .added(let uids, let mc):
                guard let mc else { return }
                var known = entries.compactMap(\.item.playQueueItemID)
                for u in uids {
                    guard let i = indexOf(u) else { continue }
                    let rk = entries[i].item.ratingKey
                    if let found = mc.metadata.first(where: {
                        $0.ratingKey == rk && $0.playQueueItemID != nil && !known.contains($0.playQueueItemID!)
                    }), let id = found.playQueueItemID {
                        entries[i].item.playQueueItemID = id
                        known.append(id)
                    }
                }
            case .upcoming(let items):
                extending = false
                let known = entries.compactMap(\.item.playQueueItemID)
                guard let last = known.last, let after = items.firstIndex(where: { $0.playQueueItemID == last }) else {
                    return
                }
                let fresh = items[(after + 1)...].filter { m in
                    m.playQueueItemID.map { !known.contains($0) } ?? false
                }
                var later: [(Metadata, Int)] = []
                for item in fresh {
                    let mediaIndex = Prefs.mediaIndex(app.paths, scope, item, nil)
                    later.append((item, mediaIndex))
                    do {
                        try insert(item, mediaIndex, nil)
                    } catch {
                        log.debug("could not extend queue: \(errorText(error))")
                    }
                }
                saveTracksLater(later)
                emitQueue()
            case .loadFailed(let gen, let error):
                guard let c = cur, c.gen == gen else { return }
                let (kind, _) = Failure.explainError(error)
                onEnd(c, completed: false, failed: (errorText(error), kind == .network ? .network : .other))
            }
        }

        func onControl(_ cmd: Control) async {
            switch cmd {
            case .togglePause:
                engine.isPaused ? engine.play() : engine.pause()
            case .play:
                engine.play()
            case .pause:
                engine.pause()
            case .next:
                // mpv's `playlist-next`: nothing after the last item, unless the queue repeats.
                guard let i = curIndex else { return }
                if i + 1 < entries.count {
                    go(i + 1)
                } else if Controls.repeat(app) == .all {
                    go(0)
                }
            case .previous:
                guard let i = curIndex else { return }
                if i > 0 {
                    go(i - 1)
                } else if Controls.repeat(app) == .all {
                    go(entries.count - 1)
                }
            case .stop:
                quitting = true
            case .seekRelative(let seconds):
                await engine.seek(toSeconds: max(0, engine.positionSeconds + seconds))
            case .seekTo(let seconds):
                await engine.seek(toSeconds: seconds)
            }
        }

        func onTune(_ cmd: Controls.Tune) async {
            do {
                try await Controls.apply(app, engine, tracker, cmd)
            } catch {
                app.error(error)
            }
            // The sleep timer's "end of item" is "stop after this".
            let atEnd = Controls.sleepAtItemEnd(app)
            if atEnd && !stopAfter {
                setStopAfter(true)
            } else if !atEnd && sleepItem && stopAfter {
                setStopAfter(false)
            }
            sleepItem = atEnd
        }

        func onQueueCommand(_ cmd: QueueCommand) throws {
            switch cmd {
            case .add(let item, let next):
                // Tags are relative to the connected server, which may have changed since the
                // player started: compare where the item and the player actually are.
                guard Scope.eq(Prefs.scopeFor(app, item), scope) else {
                    throw CoreError("A queue plays from one server; “\(Metadata.title(item))” is on another.")
                }
                if Metadata.isPlayable(item) {
                    try addItems(item, [item], next)
                } else {
                    // A container's items come from the server; wait for them off the loop.
                    let (server, inbox) = (server, inbox)
                    Task {
                        let result: Result<[Metadata], CoreError>
                        do {
                            result = .success(try await leaves(server, item))
                        } catch {
                            result = .failure(CoreError(errorText(error)))
                        }
                        inbox.send(.internal(.leaves(source: item, next: next, result: result)))
                    }
                }
            case .remove(let uid):
                guard let i = indexOf(uid) else { return }
                let e = entries.remove(at: i)
                if let session = e.transcodeSession {
                    Report.reporter().send(server, .stopTranscode(session: session))
                }
                if let queue = playQueueId, let item = e.item.playQueueItemID {
                    Report.reporter().send(server, .playQueue(.remove(queue: queue, item: item)))
                }
                unshuffled?.removeAll { $0 == uid }
                if let c = cur, c.uid == uid {
                    // Like removing mpv's current playlist entry: the next one plays.
                    cur = nil
                    finish(c, false)
                    if i < entries.count {
                        go(i)
                    } else {
                        quitting = true
                    }
                }
            case .move(let uid, let before):
                guard let from = indexOf(uid) else { return }
                let to = before.flatMap(indexOf) ?? entries.count
                moveEntry(from, to)
                let nowAt = indexOf(uid) ?? 0
                let after = nowAt > 0 ? entries[nowAt - 1].item.playQueueItemID : nil
                if let queue = playQueueId, let item = entries[nowAt].item.playQueueItemID {
                    Report.reporter().send(server, .playQueue(.move(queue: queue, item: item, after: after)))
                }
            case .jump(let uid):
                if let i = indexOf(uid) { go(i) }
            case .shuffle(let on):
                shuffle(on)
            case .stopAfter(let on):
                setStopAfter(on)
            }
        }

        /// Queue `items` (what `source` stands for), and mirror them to the Plex queue in the background.
        func addItems(_ source: Metadata, _ items: [Metadata], _ next: Bool) throws {
            if items.isEmpty { throw CoreError("nothing playable in “\(Metadata.title(source))”") }
            let first = next ? curIndex.map { $0 + 1 } : nil
            var added: [Int] = []
            var later: [(Metadata, Int)] = []
            for (n, it) in items.enumerated() {
                let mediaIndex = Prefs.mediaIndex(app.paths, scope, it, nil)
                later.append((it, mediaIndex))
                do {
                    added.append(try insert(it, mediaIndex, first.map { $0 + n }))
                } catch {
                    log.warn("could not queue item: \(errorText(error))")
                }
            }
            saveTracksLater(later)
            // Keep the Plex queue in step so other apps (and later edits) agree; its answer
            // tells us the new entries' ids, which arrives as an `added` message.
            if let id = playQueueId {
                let uri =
                    Metadata.kind(source) == "playlist"
                    ? server.libraryUri("/playlists/\(Metadata.ratingKey(source))/items")
                    : server.itemUri([Metadata.ratingKey(source)])
                let (server, inbox, uids) = (server, inbox, added)
                Task {
                    let queue = try? await server.addToPlayQueue(id, uri: uri, next: next)
                    inbox.send(.internal(.added(uids: uids, queue: queue)))
                }
            }
            unshuffled?.append(contentsOf: added)
            let what = added.count == 1 ? "1 item" : "\(added.count) items"
            app.toast(next ? "Playing \(what) next" : "Added \(what) to the queue")
        }

        /// Move entry `from` before entry `to` (or to the end).
        func moveEntry(_ from: Int, _ to: Int) {
            if from == to || from + 1 == to { return }
            let e = entries.remove(at: from)
            let dest = from < to ? to - 1 : to
            entries.insert(e, at: min(dest, entries.count))
        }

        /// Put the entries after the current one into `order`.
        func reorderUpcoming(_ order: [Int]) {
            let base = curIndex.map { $0 + 1 } ?? 0
            for (k, u) in order.enumerated() {
                guard let i = indexOf(u) else { continue }
                if i >= base && i != base + k { moveEntry(i, base + k) }
            }
        }

        func shuffle(_ on: Bool) {
            let base = min(curIndex.map { $0 + 1 } ?? 0, entries.count)
            if on {
                if unshuffled == nil { unshuffled = entries.map(\.uid) }
                reorderUpcoming(entries[base...].map(\.uid).shuffled())
                if let queue = playQueueId { Report.reporter().send(server, .playQueue(.shuffle(queue: queue))) }
            } else if let order = unshuffled {
                unshuffled = nil
                let upcoming = Set(entries[base...].map(\.uid))
                reorderUpcoming(order.filter(upcoming.contains))
                if let queue = playQueueId { Report.reporter().send(server, .playQueue(.unshuffle(queue: queue))) }
            }
        }

        /// One message of the playback loop.
        func handle(_ msg: Msg) async {
            switch msg {
            case .event(let event, let gen):
                await onEvent(event, gen)
            case .control(let cmd):
                await onControl(cmd)
                emitQueue()
            case .queue(let cmd):
                do {
                    try onQueueCommand(cmd)
                } catch {
                    app.error("Could not change the queue: \(errorText(error))")
                }
                tracker.publish(app, true)
                emitQueue()
                persistQueue()
            case .tune(let cmd):
                await onTune(cmd)
                tracker.publish(app, true)
                emitQueue()
                persistQueue()
            case .skipMarker:
                if let m = cur?.activeMarker { await skipMarker(m) }
            case .selectTrack(let choice):
                await selectTrack(choice)
            case .internal(let msg):
                await onInternal(msg)
            case .tick:
                tickQueued = false
                tick()
            case .stateTick:
                stateTickQueued = false
                tracker.publish(app, false)
                // A position the 900 ms throttle held back (no time events while paused).
                if let c = cur, app.nowPlaying()?.positionMs != c.timeMs { publish() }
            }
        }
    }

    static func timelineAt(_ c: Current, _ state: TimelineState, _ timeMs: Int) -> Report.Op? {
        guard let rk = c.item.ratingKey, let key = c.item.key else { return nil }
        return .timeline(
            TimelineUpdate(
                ratingKey: rk, key: key, state: state, timeMs: timeMs, durationMs: c.durationMs,
                playQueueItemId: c.item.playQueueItemID, session: c.transcodeSession))
    }

    /// Send `.tick` (or `.stateTick`) to the loop every `period`, unless one is still waiting
    /// in its inbox (`setInterval` in TypeScript).
    static func ticker(_ p: any Active, _ period: Duration, state: Bool) -> Task<Void, Never> {
        Task { [weak p] in
            while !Task.isCancelled {
                try? await Task.sleep(for: period)
                guard let p, !Task.isCancelled, !p.inbox.closed else { return }
                if state {
                    if !p.stateTickQueued {
                        p.stateTickQueued = true
                        p.inbox.send(.stateTick)
                    }
                } else if !p.tickQueued {
                    p.tickQueued = true
                    p.inbox.send(.tick)
                }
            }
        }
    }

    static func run(_ app: AppState, _ req: PlayRequest, _ origin: Scope?) async throws {
        guard let session = app.session() else { throw CoreError("Not connected to a server.") }
        let s = state(app)
        guard let engine = s.engine else { throw CoreError("The player is not ready.") }
        // An item found on another server of the account plays, and reports, through that server.
        let server = try await Servers.serverOf(app, req.item)
        let tagId = Metadata.serverId(req.item)
        let serverTag = tagId != nil && !Servers.isCurrent(app, tagId) ? tagId : nil
        // Checkpoints and saved queues are kept per server, so the other server's rating keys never mix with ours.
        var scope = Scope.of(session)
        if let serverTag { scope.server = serverTag }
        let settings = app.settings().player
        let forced = s.startOffset
        s.startOffset = nil
        let queue = try await buildQueue(server, req, settings)

        await replaceActive(app)
        let p = Run(
            app: app, server: server, serverTag: serverTag, scope: scope, origin: origin, settings: settings,
            quality: req.quality.flatMap(Config.quality) ?? Config.playerQuality(settings), engine: engine,
            kind: queue.kind, live: req.live, playQueueId: queue.id,
            tracker: Controls.Tracker(Controls.connectionInfoOf(session, server, scope.server)))
        s.active = p
        Controls.applySaved(app, engine, p.tracker)

        // Resolve stream URLs up front; items that cannot be played are dropped.
        var start = 0
        var startMs = 0
        var later: [(Metadata, Int)] = []
        for (i, item) in queue.items.enumerated() {
            let mediaIndex = Prefs.mediaIndex(app.paths, scope, item, i == queue.start ? req.mediaIndex : nil)
            if !p.live && transcoded(item, p.quality, false) {
                if i == queue.start {
                    // The transcoder starts as soon as the engine opens the stream.
                    do {
                        try await Prefs.saveAuto(app.paths, scope, server, item, mediaIndex)
                    } catch {
                        log.debug("could not save automatic track choice: \(errorText(error))")
                    }
                } else {
                    later.append((item, mediaIndex))
                }
            }
            if i == queue.start {
                p.initialStart = (Metadata.ratingKey(item), 0)
                startMs = 0
                if req.resume {
                    // A position asked for, else a local checkpoint newer than what the server knows
                    // (a connection that dropped before it was told), else the server's.
                    let asked = forced.flatMap { $0.0 == Metadata.ratingKey(item) ? $0.1 : nil }
                    let local = Recovery.resumePosition(app.paths, scope, item)
                    if let chosen = asked ?? local ?? item.viewOffset, chosen > 0 {
                        p.initialStart = (Metadata.ratingKey(item), chosen)
                        startMs = chosen
                        if let own = asked ?? local, item.viewOffset.map({ abs(own - $0) > 2000 }) ?? true {
                            p.recovered = (Metadata.ratingKey(item), chosen)
                        }
                    }
                }
                start = p.entries.count
            }
            do {
                try p.insert(item, mediaIndex, nil)
            } catch {
                log.warn("skipping queue item: \(errorText(error))")
            }
        }
        if p.entries.isEmpty {
            if s.active === p { s.active = nil }
            p.inbox.close()
            p.ended.close()
            throw CoreError("nothing in the queue could be played")
        }
        p.saveTracksLater(later)
        p.go(start, startMs: startMs)

        let ticks = ticker(p, .seconds(10), state: false)
        let stateTicks = ticker(p, .seconds(1), state: true)
        defer {
            ticks.cancel()
            stateTicks.cancel()
        }

        while !p.quitting, let msg = await p.inbox.recv() {
            await p.handle(msg)
        }

        // A queue played to its end is done; anything else can be offered again after a restart.
        let atEnd = p.finishedQueue || (p.cur.map { $0.reachedEnd && !p.hasNext } ?? false)
        if atEnd && Controls.repeat(app) == .off {
            try? Recovery.clearQueue(app.paths, scope)
        } else {
            p.persistQueue()
        }
        let last = p.cur
        p.cur = nil
        if let last { p.finish(last, last.reachedEnd) }
        for e in p.entries {
            if let session = e.transcodeSession { Report.reporter().send(server, .stopTranscode(session: session)) }
        }
        // No more commands reach this loop.
        p.inbox.close()
        if s.active === p {
            s.active = nil
            engine.stop()
            app.setNowPlaying(nil)
            app.setQueue(nil)
            Controls.finished(app)
        }
        p.refreshAfterReports()
        p.ended.close()
    }

    /// Stop the running playback (it is being replaced) and wait for it to wrap up.
    static func replaceActive(_ app: AppState) async {
        let s = state(app)
        guard let old = s.active else { return }
        s.active = nil
        old.inbox.send(.control(.stop))
        _ = await old.ended.recv()
    }

    // MARK: Offline playback

    /// Play downloaded files with local checkpoints; the outbox reconciles them when connected.
    public static func playFiles(_ app: AppState, _ files: [Download]) {
        app.spawn {
            do {
                try await runFiles(app, files)
            } catch {
                app.error("Playback failed: \(errorText(error))")
                app.setNowPlaying(nil)
            }
        }
    }

    final class FilesRun: Active {
        let app: AppState
        let engine: any PlaybackEngine
        let files: [Download]
        let threshold: Double
        let tracker = Controls.Tracker(Controls.offlineConnection())
        let inbox = Channel<Msg>()
        let ended = Channel<Void>()
        var acceptsQueue: Bool { false }
        var np: NowPlaying?
        var gen: Int?
        var lastUi = ContinuousClock.now
        var checkpointAt = ContinuousClock.now
        var quitting = false
        var tickQueued = false
        var stateTickQueued = false

        init(app: AppState, engine: any PlaybackEngine, files: [Download], threshold: Double) {
            self.app = app
            self.engine = engine
            self.files = files
            self.threshold = threshold
        }

        func save(_ completed: Bool) {
            guard let n = np, files.indices.contains(n.queuePos) else { return }
            do {
                try Offline.record(app.paths, files[n.queuePos], n.positionMs, n.durationMs, completed, threshold)
            } catch {
                app.error("Could not save offline progress: \(errorText(error))")
            }
        }

        func publish() {
            lastUi = .now
            app.setNowPlaying(np)
        }

        func go(_ pos: Int) {
            save(false)
            np = nil
            gen = nil
            guard files.indices.contains(pos), let file = files[pos].file else {
                quitting = true
                return
            }
            let d = files[pos]
            let position = Offline.position(app.paths, d)
            np = NowPlaying(
                item: d.item, positionMs: position, durationMs: d.item.duration ?? 0, paused: false, transcoding: false,
                queueLen: files.count, queuePos: pos, uid: 0, ending: false, stopAfter: false, live: false,
                local: true)
            tracker.newItem([])
            tracker.publish(app, true)
            publish()
            engine.setRemoteQueue(hasPrevious: pos > 0, hasNext: pos + 1 < files.count)
            let request = EngineLoad(
                url: URL(fileURLWithPath: file), startSeconds: Double(position) / 1000,
                audioOnly: queueKind(d.item) == "audio", nowPlaying: card(nil, d.item))
            let s = state(app)
            let (engine, inbox) = (engine, inbox)
            Task { [weak self] in
                guard let self, self.np?.queuePos == pos, !self.quitting else { return }
                s.engineGen += 1
                let gen = s.engineGen
                self.gen = gen
                do {
                    try await engine.load(request)
                } catch is CancellationError {
                    return
                } catch {
                    inbox.send(.event(.failed(message: errorText(error), kind: .other), gen: gen))
                }
            }
        }

        func handle(_ msg: Msg) async {
            switch msg {
            case .event(let ev, let g):
                guard g == gen, var n = np else { return }
                switch ev {
                case .loaded(let duration, _, let chapters):
                    if let duration, duration > 0 { n.durationMs = Int(duration * 1000) }
                    tracker.onChapters(chapters)
                    if tracker.speed != 1 { engine.setRate(tracker.speed) }
                    np = n
                    tracker.publish(app, true)
                    publish()
                case .time(let seconds):
                    n.positionMs = Int(seconds * 1000)
                    np = n
                    tracker.onTime(n.positionMs)
                    tracker.publish(app, false)
                    if ContinuousClock.now - checkpointAt >= .seconds(5) {
                        save(false)
                        checkpointAt = .now
                    }
                    if ContinuousClock.now - lastUi >= .milliseconds(900) { publish() }
                case .paused(let paused):
                    n.paused = paused
                    np = n
                    save(false)
                    checkpointAt = .now
                    publish()
                case .diagnostics(let d):
                    tracker.publish(app, tracker.onDiagnostics(d))
                case .audioTrack, .subtitleTrack:
                    break
                case .ended:
                    save(true)
                    np = nil
                    if Controls.sleepAtItemEnd(app) {
                        quitting = true
                    } else {
                        go(n.queuePos + 1)
                    }
                case .failed(let message, _):
                    app.error("Couldn't play \(Metadata.fullTitle(n.item)): \(message)")
                    go(n.queuePos + 1)
                }
            case .control(let cmd):
                let pos = np?.queuePos
                switch cmd {
                case .togglePause: engine.isPaused ? engine.play() : engine.pause()
                case .play: engine.play()
                case .pause: engine.pause()
                case .next: if let pos, pos + 1 < files.count { go(pos + 1) }
                case .previous: if let pos, pos > 0 { go(pos - 1) }
                case .stop: quitting = true
                case .seekRelative(let s): await engine.seek(toSeconds: max(0, engine.positionSeconds + s))
                case .seekTo(let s): await engine.seek(toSeconds: s)
                }
            case .tune(let cmd):
                // Playing downloads: the controls apply directly.
                do {
                    try await Controls.apply(app, engine, tracker, cmd)
                } catch {
                    app.error(error)
                }
                tracker.publish(app, true)
            case .stateTick:
                stateTickQueued = false
                tracker.publish(app, false)
                if let n = np, app.nowPlaying()?.positionMs != n.positionMs { publish() }
            case .queue, .skipMarker, .selectTrack, .internal, .tick:
                break
            }
        }
    }

    static func runFiles(_ app: AppState, _ files: [Download]) async throws {
        let s = state(app)
        guard let engine = s.engine else { throw CoreError("The player is not ready.") }
        for d in files where d.file == nil { throw CoreError("Downloaded file is missing") }
        await replaceActive(app)
        let p = FilesRun(app: app, engine: engine, files: files, threshold: app.settings().player.watchedThreshold)
        s.active = p
        Controls.applySaved(app, engine, p.tracker)
        p.go(0)
        let stateTicks = ticker(p, .seconds(1), state: true)
        defer { stateTicks.cancel() }
        while !p.quitting, let msg = await p.inbox.recv() {
            await p.handle(msg)
        }
        p.save(false)
        p.inbox.close()
        if s.active === p {
            s.active = nil
            engine.stop()
            app.setNowPlaying(nil)
            Controls.finished(app)
        }
        p.ended.close()
    }
}

/// `{ item, mediaIndex }` for the concurrent workers (tuples are not `Sendable`).
nonisolated struct Pair: Sendable {
    var item: Metadata
    var mediaIndex: Int
}

/// An unbounded queue with one reader (the TypeScript `Channel`): `send` never waits, `recv`
/// waits for a value and returns `nil` once the channel is closed and drained.
final class Channel<T: Sendable> {
    private var buffer: [T] = []
    private var waiter: CheckedContinuation<T?, Never>?
    private(set) var closed = false

    /// False when the channel is closed.
    @discardableResult
    func send(_ value: T) -> Bool {
        guard !closed else { return false }
        if let w = waiter {
            waiter = nil
            w.resume(returning: value)
        } else {
            buffer.append(value)
        }
        return true
    }

    func recv() async -> T? {
        if !buffer.isEmpty { return buffer.removeFirst() }
        if closed { return nil }
        return await withCheckedContinuation { waiter = $0 }
    }

    func close() {
        closed = true
        if let w = waiter {
            waiter = nil
            w.resume(returning: nil)
        }
    }
}
