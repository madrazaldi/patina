// Port of desktop/core/controls.ts.
// Player controls beyond transport (volume, speed, chapters, repeat, a sleep timer) and the
// live diagnostics shown beside them (buffering, decoder, dropped frames, connection type).
//
// The controls act on the engine directly. What the front ends show arrives as one
// `PlaybackState` event, emitted when something changes (diagnostics at most once a second).
// The state is also kept here, for front ends that open while something is already playing.
//
// On the desktop mpv owned volume, speed and repeat and reported them as properties; here the
// `Tracker` keeps them, and repeat and the sleep timer are acted on by the player's own queue.
// The settings that carry over between playbacks (`Tuning`) are kept per `AppState`.

import Foundation
import PlexKit

public nonisolated struct PlaybackState: Sendable, Equatable {
    /// Percent (0 to 100).
    public var volume: Double
    public var muted: Bool
    /// The rate in effect (the engine may play less than was asked).
    public var speed: Double
    public var `repeat`: Controls.Repeat
    public var sleep: Controls.SleepState?
    public var chapters: [Controls.ChapterView]
    /// Index into `chapters` of the playing chapter.
    public var chapter: Int?
    public var diagnostics: Controls.Diagnostics
}

public enum Controls {
    /// What happens when the queue or item ends.
    public nonisolated enum Repeat: String, Sendable, Equatable {
        case off, one, all
    }

    /// Off, then the whole queue, then the item, then off again.
    public nonisolated static func nextRepeat(_ r: Repeat) -> Repeat {
        switch r {
        case .off: .all
        case .all: .one
        case .one: .off
        }
    }

    public nonisolated static func repeatLabel(_ r: Repeat) -> String {
        switch r {
        case .off: "off"
        case .one: "repeat one"
        case .all: "repeat all"
        }
    }

    /// Sleep timer presets cycled through by `cycleSleep`, in minutes.
    public nonisolated static let SLEEP_PRESETS = [15, 30, 45, 60, 90]

    /// A change to the running player.
    public nonisolated enum Tune: Sendable, Equatable {
        /// Volume in percent (0 to 100).
        case volume(Double)
        case volumeBy(Double)
        /// Mute, unmute, or toggle with `nil`.
        case mute(Bool?)
        /// Playback speed (0.25 to 4).
        case speed(Double)
        case speedBy(Double)
        case `repeat`(Repeat)
        case cycleRepeat
        /// Jump to a chapter by index.
        case chapter(Int)
        case chapterBy(Int)
        case sleepOff
        case sleepMinutes(Int)
        case sleepEndOfItem
        /// Off, then each of `SLEEP_PRESETS`, then the end of the item, then off.
        case cycleSleep
    }

    public nonisolated struct ChapterView: Sendable, Equatable {
        public var title: String
        public var timeMs: Int
    }

    /// How the server is reached.
    public nonisolated struct ConnectionInfo: Sendable, Equatable {
        /// `local`, `remote`, `relay` or `offline` (a downloaded file).
        public var kind: String
        public var address: String?
    }

    public nonisolated static func offlineConnection() -> ConnectionInfo {
        ConnectionInfo(kind: "offline", address: nil)
    }

    public nonisolated struct Diagnostics: Sendable, Equatable {
        /// Playback is stalled waiting for data.
        public var buffering = false
        /// How full the buffer is while stalled, in percent (mpv only; `nil` on iOS).
        public var bufferPercent: Double?
        /// Seconds of media ahead of the playhead that are already buffered.
        public var bufferedSecs: Double?
        /// The decoder in use ("VideoToolbox", "dav1d", ...); `nil` is unknown.
        public var hwdec: String?
        /// Frames the video output dropped.
        public var droppedFrames = 0
        /// Frames the decoder dropped (mpv only; 0 on iOS).
        public var decoderDroppedFrames = 0
        public var connection: ConnectionInfo?
        /// Where playback restarted from a local checkpoint after the connection failed, if it did.
        public var recoveredFromMs: Int?
        /// "SDR", "HDR10", "Dolby Vision", ... (iOS).
        public var videoFormat: String?
        /// How the engine plays the item: "Direct (AVPlayer)", "Remux", "Software", ... (iOS).
        public var route: String?
        /// "1920×1080" (iOS).
        public var videoSize: String?
    }

    public nonisolated struct SleepState: Sendable, Equatable {
        /// `minutes` or `endOfItem`.
        public var mode: String
        /// When the timer ends, in milliseconds since the epoch (`minutes` only).
        public var endsAtMs: Int?
    }

    // MARK: Settings that outlive one playback

    enum Sleep: Equatable {
        case minutes(id: Int, endsAtMs: Int)
        case endOfItem
    }

    final class Tuning {
        var volume: Double?
        var muted = false
        var `repeat` = Repeat.off
        var sleep: Sleep?
        /// Mirrors the player, which owns "stop after this".
        var stopAfter = false
        var state: PlaybackState?
        var nextTimer = 1
        var timer: Task<Void, Never>?
    }

    static func tuning(_ app: AppState) -> Tuning {
        app.local(Tuning.self) { Tuning() }
    }

    static func nowMs() -> Int {
        Int(Date().timeIntervalSince1970 * 1000)
    }

    /// The last published state; `nil` when nothing is playing.
    public static func playbackState(_ app: AppState) -> PlaybackState? {
        tuning(app).state
    }

    public static func `repeat`(_ app: AppState) -> Repeat {
        tuning(app).repeat
    }

    /// The player toggled "stop after this"; repeat-one must not keep the item from ending.
    static func setStopAfter(_ app: AppState, _ on: Bool) {
        let t = tuning(app)
        t.stopAfter = on
        if !on, t.sleep == .endOfItem { t.sleep = nil }
    }

    /// Whether the sleep timer is "end of this item".
    public static func sleepAtItemEnd(_ app: AppState) -> Bool {
        tuning(app).sleep == .endOfItem
    }

    /// Make the engine (and the tracker of a new playback) honour the volume and mute choices
    /// made earlier. Repeat needs nothing: the player reads it when an item ends.
    static func applySaved(_ app: AppState, _ engine: any PlaybackEngine, _ tracker: Tracker) {
        let t = tuning(app)
        let volume = t.volume ?? 100
        engine.setVolume(volume / 100)
        engine.setMuted(t.muted)
        tracker.volume = volume
        tracker.muted = t.muted
    }

    /// Playback ended: drop what only applies to it.
    static func finished(_ app: AppState) {
        let t = tuning(app)
        t.sleep = nil
        t.timer?.cancel()
        t.timer = nil
        t.stopAfter = false
        t.state = nil
        app.emit(.playbackState(nil))
    }

    /// Apply a `Tune` to the running player.
    static func apply(_ app: AppState, _ engine: any PlaybackEngine, _ tracker: Tracker, _ cmd: Tune) async throws {
        let t = tuning(app)
        switch cmd {
        case .volume(let v):
            setVolume(app, engine, tracker, v)
        case .volumeBy(let delta):
            setVolume(app, engine, tracker, tracker.volume + delta)
        case .mute(let on):
            let muted = on ?? !tracker.muted
            engine.setMuted(muted)
            tracker.muted = muted
            t.muted = muted
        case .speed(let v):
            setSpeed(engine, tracker, v)
        case .speedBy(let delta):
            setSpeed(engine, tracker, tracker.speed + delta)
        case .repeat(let mode):
            t.repeat = mode
        case .cycleRepeat:
            t.repeat = nextRepeat(t.repeat)
        case .chapter(let index):
            try await jumpChapter(app, engine, index)
        case .chapterBy(let delta):
            guard let state = playbackState(app) else { throw CoreError("Nothing is playing") }
            let current = state.chapter ?? (delta > 0 ? -1 : state.chapters.count)
            try await jumpChapter(app, engine, current + delta)
        case .sleepOff:
            setSleep(app, nil)
        case .sleepMinutes(let minutes):
            setSleep(app, minutesTimer(app, minutes))
        case .sleepEndOfItem:
            setSleep(app, .endOfItem)
        case .cycleSleep:
            let next: Sleep?
            switch t.sleep {
            case nil:
                next = minutesTimer(app, SLEEP_PRESETS[0])
            case .minutes(_, let endsAtMs):
                // The preset at or above what is left, then the one after it.
                let left = Int((Double(max(0, endsAtMs - nowMs())) / 60_000).rounded(.up))
                if let i = SLEEP_PRESETS.firstIndex(where: { $0 >= left }), i + 1 < SLEEP_PRESETS.count {
                    next = minutesTimer(app, SLEEP_PRESETS[i + 1])
                } else {
                    next = .endOfItem
                }
            case .endOfItem:
                next = nil
            }
            setSleep(app, next)
        }
        refresh(app)
    }

    static func minutesTimer(_ app: AppState, _ minutes: Int) -> Sleep {
        let t = tuning(app)
        defer { t.nextTimer += 1 }
        return .minutes(id: t.nextTimer, endsAtMs: nowMs() + minutes * 60_000)
    }

    static func setVolume(_ app: AppState, _ engine: any PlaybackEngine, _ tracker: Tracker, _ value: Double) {
        let v = min(100, max(0, value)).rounded()
        engine.setVolume(v / 100)
        tracker.volume = v
        tuning(app).volume = v
    }

    static func setSpeed(_ engine: any PlaybackEngine, _ tracker: Tracker, _ value: Double) {
        let v = (min(4, max(0.25, value)) * 100).rounded() / 100
        engine.setRate(v)
        // The engine clamps to what the item supports; show what plays.
        tracker.speed = min(v, max(engine.maxRate, 0.25))
    }

    static func jumpChapter(_ app: AppState, _ engine: any PlaybackEngine, _ index: Int) async throws {
        guard let state = playbackState(app) else { throw CoreError("Nothing is playing") }
        guard state.chapters.indices.contains(index) else { throw CoreError("No such chapter") }
        await engine.seek(toSeconds: Double(state.chapters[index].timeMs) / 1000)
    }

    static func setSleep(_ app: AppState, _ sleep: Sleep?) {
        let t = tuning(app)
        t.sleep = sleep
        t.timer?.cancel()
        t.timer = nil
        guard case .minutes(let id, let endsAtMs) = sleep else { return }
        let wait = max(0, endsAtMs - nowMs())
        t.timer = Task { [weak app] in
            try? await Task.sleep(for: .milliseconds(wait))
            guard !Task.isCancelled, let app else { return }
            let t = tuning(app)
            if case .minutes(id, _) = t.sleep {
                t.sleep = nil
                app.toast("Sleep timer: playback stopped")
                Player.control(app, .stop)
                refresh(app)
            }
        }
    }

    /// Publish the shared state again after a control changed the app-wide parts of it.
    static func refresh(_ app: AppState) {
        let t = tuning(app)
        guard var state = t.state else { return }
        (state.repeat, state.sleep) = globalParts(app)
        t.state = state
        app.emit(.playbackState(state))
    }

    static func globalParts(_ app: AppState) -> (Repeat, SleepState?) {
        let t = tuning(app)
        let sleep: SleepState? =
            switch t.sleep {
            case nil: nil
            case .minutes(_, let endsAtMs): SleepState(mode: "minutes", endsAtMs: endsAtMs)
            case .endOfItem: SleepState(mode: "endOfItem", endsAtMs: nil)
            }
        return (t.repeat, sleep)
    }

    // MARK: Following the engine

    nonisolated static func chapterTitle(_ title: String?, _ i: Int) -> String {
        if let title, !title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { return title }
        return "Chapter \(i + 1)"
    }

    /// Keeps the `PlaybackState` of one player up to date from the engine's events and the
    /// controls applied to it.
    public final class Tracker {
        var volume = 100.0
        var muted = false
        var speed = 1.0
        private var engineChapters: [ChapterView] = []
        private var plexChapters: [ChapterView] = []
        private var positionMs = 0
        private var diag: Diagnostics
        private var emitted: PlaybackState?
        private var lastEmit = ContinuousClock.now - .seconds(5)

        public init(_ connection: ConnectionInfo) {
            diag = Diagnostics(connection: connection)
        }

        /// Take in the engine's diagnostics; true when they should be published right away
        /// (buffering or the decoder changed; buffer levels and frame counts can wait).
        @discardableResult
        public func onDiagnostics(_ d: EngineDiagnostics) -> Bool {
            let urgent = d.buffering != diag.buffering || d.decoder != diag.hwdec || d.route != diag.route
                || d.videoFormat != diag.videoFormat
            diag.buffering = d.buffering
            diag.bufferedSecs = d.bufferedSeconds.map { ($0 * 10).rounded() / 10 }
            diag.hwdec = d.decoder.flatMap { $0.isEmpty ? nil : $0 }
            diag.droppedFrames = d.droppedFrames ?? 0
            diag.videoFormat = d.videoFormat
            diag.route = d.route
            diag.videoSize = d.videoSize
            return urgent
        }

        /// The chapters of the file, as the engine found them.
        public func onChapters(_ chapters: [EngineChapter]) {
            engineChapters = chapters.enumerated().map { i, c in
                ChapterView(title: chapterTitle(c.title, i), timeMs: Int(c.startSeconds * 1000))
            }
        }

        public func onTime(_ ms: Int) {
            positionMs = ms
        }

        /// A new file: per-file figures start over.
        public func newItem(_ plex: [Chapter]) {
            engineChapters = []
            diag.buffering = false
            diag.bufferPercent = nil
            diag.bufferedSecs = nil
            diag.droppedFrames = 0
            diag.decoderDroppedFrames = 0
            diag.recoveredFromMs = nil
            diag.hwdec = nil
            diag.videoFormat = nil
            diag.route = nil
            diag.videoSize = nil
            setPlexChapters(plex)
        }

        /// Chapters Plex knows, for streams (transcodes) that carry none of their own.
        public func setPlexChapters(_ plex: [Chapter]) {
            plexChapters = plex.enumerated().map { i, c in
                ChapterView(title: chapterTitle(c.tag, i), timeMs: c.startTimeOffset ?? 0)
            }
        }

        public func recoveredFrom(_ ms: Int?) {
            diag.recoveredFromMs = ms
        }

        public func view(_ app: AppState) -> PlaybackState {
            let chapters = engineChapters.isEmpty ? plexChapters : engineChapters
            let chapter = chapters.lastIndex { $0.timeMs <= positionMs }
            let (rep, sleep) = globalParts(app)
            return PlaybackState(
                volume: volume, muted: muted, speed: speed, repeat: rep, sleep: sleep, chapters: chapters,
                chapter: chapter, diagnostics: diag)
        }

        /// Emit the state if it changed. Quiet changes (buffer levels, frame counts) wait for
        /// the next second; `urgent` ones go out now.
        public func publish(_ app: AppState, _ urgent: Bool) {
            if !urgent && ContinuousClock.now - lastEmit < .seconds(1) { return }
            let state = view(app)
            if emitted == state { return }
            lastEmit = .now
            emitted = state
            tuning(app).state = state
            app.emit(.playbackState(state))
        }
    }

    // MARK: Connection

    /// How the session's server is reached: from Plex's connection list when it names the
    /// address in use, else from the address itself.
    public nonisolated static func connectionInfo(_ session: Session) -> ConnectionInfo {
        connectionInfoOf(session, session.server, session.serverId)
    }

    /// `connectionInfo` for `server`, one of the account's servers with id `serverId`.
    public nonisolated static func connectionInfoOf(_ session: Session, _ server: PlexServer, _ serverId: String)
        -> ConnectionInfo
    {
        let base = server.baseUrl
        let rawHost = base.host(percentEncoded: false) ?? ""
        let host = rawHost.contains(":") ? "[\(rawHost)]" : rawHost
        let address = base.port.map { "\(host):\($0)" } ?? host
        let target = trimSlashes(base.absoluteString)
        let connections = session.servers.filter { Resource.id($0) == serverId }.flatMap(\.connections)
        let known =
            connections.first { $0.uri.map(trimSlashes) == target }
            ?? connections.first { $0.address == rawHost && $0.port == base.port }
        let kind: String =
            if let known { known.relay ? "relay" : known.local ? "local" : "remote" } else {
                isLocalHost(host) ? "local" : "remote"
            }
        return ConnectionInfo(kind: kind, address: address)
    }

    nonisolated static func trimSlashes(_ s: String) -> String {
        var s = Substring(s)
        while s.hasSuffix("/") { s = s.dropLast() }
        return String(s)
    }

    /// The 16 bytes of an IPv6 address (a zone suffix is ignored).
    nonisolated static func ipv6Bytes(_ host: String) -> [UInt8]? {
        let h = host.split(separator: "%", maxSplits: 1).first.map(String.init) ?? host
        var addr = in6_addr()
        guard inet_pton(AF_INET6, h, &addr) == 1 else { return nil }
        return withUnsafeBytes(of: &addr) { Array($0) }
    }

    nonisolated static func ipv4Bytes(_ host: String) -> [UInt8]? {
        // `inet_pton` only takes the dotted quad, like `isIPv4`.
        var addr = in_addr()
        guard inet_pton(AF_INET, host, &addr) == 1 else { return nil }
        return withUnsafeBytes(of: &addr) { Array($0) }
    }

    /// Whether an address is on this device or its network.
    public nonisolated static func isLocalHost(_ hostIn: String) -> Bool {
        var host = Prefs.asciiLower(hostIn.trimmingCharacters(in: CharacterSet(charactersIn: "[]")))
        if host.hasSuffix(".plex.direct") {
            // `192-168-1-5.<id>.plex.direct` names the address in its first label.
            let label = host.dropLast(".plex.direct".count).split(separator: ".", omittingEmptySubsequences: false).first
            host = (label.map(String.init) ?? "").replacingOccurrences(of: "-", with: ".")
        }
        if host == "localhost" || host.hasSuffix(".local") { return true }
        if let b = ipv4Bytes(host) {
            // private, loopback, link-local
            return b[0] == 10 || (b[0] == 172 && (16...31).contains(b[1])) || (b[0] == 192 && b[1] == 168)
                || b[0] == 127 || (b[0] == 169 && b[1] == 254)
        }
        if let b = ipv6Bytes(host) {
            let first = Int(b[0]) << 8 | Int(b[1])
            let loopback = b[0..<15].allSatisfy { $0 == 0 } && b[15] == 1
            return loopback || first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80
        }
        // A bare machine name only resolves on the local network.
        return !host.contains(".")
    }
}
