// Port of desktop/core/state.ts.
// Application state shared by the front end, the session flow and the player.
//
// Changes are pushed to the front end as CoreEvents; it asks for a Snapshot once on startup
// and follows the events from then on.
//
// Conventions of the PatinaCore port (read this before adding a module):
//
// * Each TypeScript module is a caseless `enum` named after it in PascalCase, holding its
//   functions as statics: `downloads.list()` is `Downloads.list()`, `ops/util.ts` is
//   `Ops.Util`, `ops/server.ts` `Ops.Server`. Two exceptions: the functions of `session.ts`
//   are statics of the `Session` struct below (`Session.retry(app)`), which keeps its name,
//   and the helpers of this file (`eventName`, `errorText`, `sameServer`, `userView`, ...) are
//   free functions, as the shared vocabulary.
// * A module's types are nested in its namespace when the name is generic or would clash
//   (`Undo.Stack`, `Report.Op`, `Live.Pending`, `Offline.Progress`, `Failure.Report`), and
//   top level when it is distinctive and shared (`Settings`, `Scope`, `Checkpoint`,
//   `Download`, `PlaybackState`, `PlaybackFailure`).
// * Everything runs on the main actor (the package's default isolation), like the
//   single-threaded JavaScript. `app.spawn { ... }` is `app.spawn(...)`: a tracked `Task`
//   whose failure is logged; tests `await app.settled()`. `setInterval` is a `Task` loop
//   with `Task.sleep`, and an `AbortController` is cancelling that `Task`.
// * `throw new Error(text)` is `throw CoreError(text)`; `errorText(e)` is what the user sees
//   (a `PlexError`'s message, a `CoreError`'s text). Commands throw; the UI shows
//   `errorText`. Toasts: `app.toast(text)`, `app.toastUndo(text, id)`, `app.error(e)`.
// * Module-level `let`s of mutable state become `static var`s of the namespace (main-actor
//   isolated), or, when tests need each `AppState` to have its own, `app.local(T.self) { T() }`.
// * Files and secrets come from `app.environment` (`AppEnvironment`): `app.paths.config`
//   is the desktop's `configDir()`, `app.paths.data` its `dataDir()`.
// * Plex clients are made with `app.newTv(token)` and `app.newServer(url, token)` so they
//   use the environment's URL session (tests route it to `MockPMS`).

import Foundation
import PlexKit

nonisolated private let log = logger("state")

/// An error with a message for people (`new Error(text)`).
public nonisolated struct CoreError: Error, LocalizedError, Sendable, Equatable {
    public let message: String

    public init(_ message: String) {
        self.message = message
    }

    public var errorDescription: String? { message }
}

/// The text of a caught error (`{e:#}`).
public nonisolated func errorText(_ e: any Error) -> String {
    switch e {
    case let e as PlexError: e.message
    case let e as CoreError: e.message
    case is CancellationError: "cancelled"
    default: e.localizedDescription
    }
}

/// `anyhow::Context`: the outer message, then the cause ("outer: inner").
public nonisolated func context(_ e: any Error, _ message: String) -> CoreError {
    CoreError("\(message): \(errorText(e))")
}

/// Where the app is in the sign-in / connect flow.
public nonisolated enum Stage: Sendable, Equatable {
    case booting
    case login
    case pickServer(servers: [ServerView], user: UserView)
    case connecting(name: String)
    case ready
    case failed(message: String)

    /// The TypeScript `kind`.
    public var kind: String {
        switch self {
        case .booting: "booting"
        case .login: "login"
        case .pickServer: "pickServer"
        case .connecting: "connecting"
        case .ready: "ready"
        case .failed: "failed"
        }
    }
}

/// A connected server plus the account that reached it. Its functions are those of
/// `session.ts` (see Session.swift).
public nonisolated struct Session: Sendable, Equatable {
    /// plex.tv client; `nil` for manually configured servers.
    public var tv: PlexTv?
    public var server: PlexServer
    public var serverName: String
    public var serverId: String
    public var user: User?
    /// Whether the signed-in user owns (administers) this server.
    public var owned: Bool
    public var servers: [Resource]

    public init(
        tv: PlexTv?, server: PlexServer, serverName: String, serverId: String, user: User?, owned: Bool,
        servers: [Resource]
    ) {
        self.tv = tv
        self.server = server
        self.serverName = serverName
        self.serverId = serverId
        self.user = user
        self.owned = owned
        self.servers = servers
    }

    public static func == (a: Session, b: Session) -> Bool {
        a.tv === b.tv && a.server == b.server && a.serverName == b.serverName && a.serverId == b.serverId
            && a.user == b.user && a.owned == b.owned && a.servers == b.servers
    }
}

/// A plex.tv account as front ends see it (no tokens).
public nonisolated struct UserView: Sendable, Equatable {
    public var uuid: String?
    public var name: String
    public var email: String?
    public var thumb: String?
    public var plexPass: Bool
}

public nonisolated func userView(_ u: User) -> UserView {
    UserView(
        uuid: u.uuid, name: User.displayName(u), email: u.email, thumb: u.thumb,
        plexPass: u.subscription?.active ?? false)
}

/// A server resource as front ends see it (no tokens or connections).
public nonisolated struct ServerView: Sendable, Equatable {
    public var id: String
    public var name: String
    public var owned: Bool
    public var sourceTitle: String?
    public var platform: String?
    public var version: String?
    public var presence: Bool
}

public nonisolated func serverView(_ r: Resource) -> ServerView {
    ServerView(
        id: Resource.id(r), name: Resource.name(r), owned: r.owned, sourceTitle: r.sourceTitle, platform: r.platform,
        version: r.productVersion, presence: r.presence)
}

public nonisolated struct SessionView: Sendable, Equatable {
    public var accountId: String
    public var serverName: String
    public var serverId: String
    public var machineIdentifier: String?
    public var baseUrl: String
    public var owned: Bool
    public var hasTv: Bool
    public var user: UserView?
    public var servers: [ServerView]
}

public nonisolated func sessionView(_ s: Session) -> SessionView {
    SessionView(
        accountId: Offline.account(s), serverName: s.serverName, serverId: s.serverId,
        machineIdentifier: s.server.machineIdentifier, baseUrl: s.server.baseUrl.absoluteString, owned: s.owned,
        hasTv: s.tv != nil, user: s.user.map(userView), servers: s.servers.map(serverView))
}

public nonisolated struct NowPlaying: Sendable, Equatable {
    public var item: Metadata
    public var positionMs: Int
    public var durationMs: Int
    public var paused: Bool
    public var transcoding: Bool
    public var queueLen: Int
    public var queuePos: Int
    /// Queue entry id of the item (see QueueView).
    public var uid: Int
    /// In the closing credits or last seconds with something queued after: show "Up next".
    public var ending: Bool
    /// Playback stops when this item ends.
    public var stopAfter: Bool
    public var live: Bool
    /// A downloaded file played offline.
    public var local: Bool

    public init(
        item: Metadata, positionMs: Int, durationMs: Int, paused: Bool, transcoding: Bool, queueLen: Int,
        queuePos: Int, uid: Int, ending: Bool, stopAfter: Bool, live: Bool, local: Bool
    ) {
        self.item = item
        self.positionMs = positionMs
        self.durationMs = durationMs
        self.paused = paused
        self.transcoding = transcoding
        self.queueLen = queueLen
        self.queuePos = queuePos
        self.uid = uid
        self.ending = ending
        self.stopAfter = stopAfter
        self.live = live
        self.local = local
    }
}

public nonisolated struct QueueItem: Sendable, Equatable {
    public var uid: Int
    public var item: Metadata

    public init(uid: Int, item: Metadata) {
        self.uid = uid
        self.item = item
    }
}

/// The running play queue, in play order.
public nonisolated struct QueueView: Sendable, Equatable {
    public var items: [QueueItem]
    public var current: Int?
    public var shuffled: Bool
    public var stopAfter: Bool

    public init(items: [QueueItem], current: Int?, shuffled: Bool, stopAfter: Bool) {
        self.items = items
        self.current = current
        self.shuffled = shuffled
        self.stopAfter = stopAfter
    }
}

/// A background task on the server (scan, analysis, ...), from live notifications.
public nonisolated struct ServerActivity: Sendable, Equatable {
    public var uuid: String
    public var kind: String
    public var title: String
    public var subtitle: String?
    public var progress: Int
    public var cancellable: Bool
    public var sectionId: String?

    public init(
        uuid: String, kind: String, title: String, subtitle: String?, progress: Int, cancellable: Bool,
        sectionId: String?
    ) {
        self.uuid = uuid
        self.kind = kind
        self.title = title
        self.subtitle = subtitle
        self.progress = progress
        self.cancellable = cancellable
        self.sectionId = sectionId
    }
}

public nonisolated struct Toast: Sendable, Equatable {
    public var text: String
    public var error: Bool
    /// Id of an undo entry (see Undo.swift) that reverses what the toast reports.
    public var undo: Int?

    public init(text: String, error: Bool, undo: Int? = nil) {
        self.text = text
        self.error = error
        self.undo = undo
    }
}

/// Everything a front end needs to render its first frame.
public nonisolated struct Snapshot: Sendable, Equatable {
    public var connected: Bool
    public var stage: Stage
    public var session: SessionView?
    public var sections: [Directory]
    public var nowPlaying: NowPlaying?
    public var queue: QueueView?
    public var activities: [ServerActivity]
    public var revision: Int
}

/// What the next "revision" event covers, so views can refetch only what changed.
/// `all` means the change could not be narrowed down (watch state, unknown sections).
public nonisolated struct Changes: Sendable, Equatable {
    public var all = false
    /// Library section ids that changed.
    public var sections: [String] = []
    /// Rating keys that changed, when the server said which.
    public var items: [String] = []

    public init(all: Bool = false, sections: [String] = [], items: [String] = []) {
        self.all = all
        self.sections = sections
        self.items = items
    }

    public static func everything() -> Changes {
        Changes(all: true)
    }

    /// A change in one library section, optionally to one item.
    public static func section(_ section: String?, _ item: String? = nil) -> Changes {
        var c = Changes()
        c.add(section, item)
        return c
    }

    /// Fold another change in. Without a section the change could be anywhere.
    public mutating func add(_ section: String?, _ item: String?) {
        if let section {
            if !sections.contains(section) { sections.append(section) }
        } else {
            all = true
        }
        if let item, !items.contains(item) { items.append(item) }
    }
}

/// A state change for the front end.
public nonisolated enum CoreEvent: Sendable, Equatable {
    case connection(online: Bool)
    case settingsChanged
    case stage(Stage)
    case session(SessionView?, sections: [Directory])
    case nowPlaying(NowPlaying?)
    case queue(QueueView?)
    case activities([ServerActivity])
    /// Watch state may have changed; views refetch.
    case revision(Int)
    /// What the revision that follows covers (sent just before it). A revision with no
    /// preceding `changes` may affect anything.
    case changes(Changes)
    case toast(Toast)
    case downloads([Download])
    /// Playback sessions on the server changed.
    case sessionsChanged
    /// Volume, speed, chapters, repeat, sleep timer and diagnostics of the playing item; `nil` when nothing plays.
    case playbackState(PlaybackState?)
    /// Playback failed and can be retried, changed or dismissed; `nil` clears it.
    case playbackFailure(PlaybackFailure?)
}

/// The event name the desktop's webview listens for (`CoreEvent::name`).
public nonisolated func eventName(_ e: CoreEvent) -> String {
    switch e {
    case .connection: "connection"
    case .settingsChanged: "settings-changed"
    case .stage: "stage"
    case .session: "session"
    case .nowPlaying: "now-playing"
    case .queue: "queue"
    case .activities: "activities"
    case .revision: "revision"
    case .changes: "library-changed"
    case .toast: "toast"
    case .downloads: "downloads"
    case .sessionsChanged: "sessions-changed"
    case .playbackState: "playback-state"
    case .playbackFailure: "playback-failure"
    }
}

/// Receives every CoreEvent.
public protocol EventSink: AnyObject {
    func emit(_ event: CoreEvent)
}

/// A sink that drops every event (tests).
public final class NullSink: EventSink {
    public init() {}
    public func emit(_ event: CoreEvent) {}
}

/// A sink that calls a closure (`{ emit: (e) => seen.push(e) }`).
public final class CallbackSink: EventSink {
    private let f: (CoreEvent) -> Void

    public init(_ f: @escaping (CoreEvent) -> Void) {
        self.f = f
    }

    public func emit(_ event: CoreEvent) {
        f(event)
    }
}

/// Servers offered on the "choose a server" screen.
public struct PendingServers {
    public var tv: PlexTv
    public var user: User
    public var servers: [Resource]
}

/// The running notification listener (see Live.swift).
public final class LiveTask {
    /// Cancelled when the listener is replaced or stopped (the TypeScript `abort`).
    var task: Task<Void?, Never>?
    /// Set when the task ended on its own.
    public internal(set) var finished = false

    func abort() {
        task?.cancel()
    }
}

/// Whether two server clients point at the same base URL with the same token (`PlexServer: PartialEq`).
public nonisolated func sameServer(_ a: PlexServer, _ b: PlexServer) -> Bool {
    a === b || a == b
}

/// Shared application state; a single instance per process.
public final class AppState {
    private let sink: any EventSink
    public let environment: AppEnvironment
    private var _settings: Settings
    private var _stage: Stage = .booting
    private var _session: Session?
    private var _sections: [Directory] = []
    private var _nowPlaying: NowPlaying?
    private var _queue: QueueView?
    private var _activities: [ServerActivity] = []
    private var _revision = 0
    private var _connected = true
    private var ticker: Task<Void, Never>?
    private var tasks: [Int: Task<Void, Never>] = [:]
    private var nextTask = 0
    private var locals: [ObjectIdentifier: AnyObject] = [:]

    /// Bumped whenever the session changes, which stops the previous server's notification listener.
    public var liveGeneration = 0
    /// The running notification listener, cancelled when it is replaced or stopped.
    public var liveTask: LiveTask?
    /// Bumped to abandon an in-flight PIN sign-in.
    public var loginAttempt = 0
    /// Servers offered on the "choose a server" screen.
    public var pending: PendingServers?
    /// Episodes loaded by the bulk audio/subtitle dialog, keyed by show.
    public var tracks: (String, [Metadata])?
    /// Reversible actions of the current account and server (see Undo.swift).
    public var undo = Undo.Stack()

    /// Load the settings and start the background poller (downloads, settings, offline sync).
    public init(sink: any EventSink, environment: AppEnvironment) {
        self.sink = sink
        self.environment = environment
        _settings = Config.loadSettings(environment)
        startTicker()
    }

    public var paths: Paths { environment.paths }

    private func startTicker() {
        guard let interval = environment.tickInterval else { return }
        ticker = Task { [weak self] in
            var downloadsSeen = ""
            var settingsSeen: [JSONValue] = []
            var ticks = 0
            while !Task.isCancelled {
                try? await Task.sleep(for: interval)
                guard let self, !Task.isCancelled else { return }
                let list = Downloads.list(paths)
                let next = String(describing: list) + Smart.fingerprint(paths)
                if next != downloadsSeen {
                    downloadsSeen = next
                    emit(.downloads(list))
                }
                if list.contains(where: { $0.state == .queued || $0.state == .downloading }) { Downloads.resume(self) }
                let s = settings()
                let nextSettings = [s.player.json, s.ui.json, s.tui.json]
                if nextSettings != settingsSeen {
                    settingsSeen = nextSettings
                    emit(.settingsChanged)
                }
                ticks += 1
                // Smart downloads follow watch state; check now and then even when nothing announces a change.
                if ticks % 150 == 5 { Smart.reconcileSoon(self) }
                if ticks % 15 == 1 {
                    spawn {
                        do {
                            try await Offline.sync(self)
                        } catch {
                            log.debug("offline progress remains queued: \(errorText(error))")
                        }
                    }
                }
            }
        }
    }

    /// Stop the background poller (the Rust version stops when the state is dropped).
    public func dispose() {
        ticker?.cancel()
        ticker = nil
    }

    public func emit(_ event: CoreEvent) {
        sink.emit(event)
    }

    /// Run a detached task: it is tracked, and its failure is logged instead of lost.
    /// Cancelling the returned task cancels the work.
    @discardableResult
    public func spawn<T: Sendable>(_ op: @escaping @MainActor () async throws -> T) -> Task<T?, Never> {
        let task = Task<T?, Never> {
            do {
                return try await op()
            } catch is CancellationError {
                return nil
            } catch {
                log.warn("background task failed: \(errorText(error))")
                return nil
            }
        }
        nextTask += 1
        let id = nextTask
        tasks[id] = Task { [weak self] in
            _ = await task.value
            self?.tasks[id] = nil
        }
        return task
    }

    /// Returns when every task spawned so far has finished (tests).
    public func settled() async {
        while let (_, task) = tasks.first {
            await task.value
        }
    }

    /// Per-app state of a module (a module-level `let` that tests need per `AppState`):
    /// made by `make` on first use.
    public func local<T: AnyObject>(_ type: T.Type, _ make: () -> T) -> T {
        if let found = locals[ObjectIdentifier(type)] as? T { return found }
        let made = make()
        locals[ObjectIdentifier(type)] = made
        return made
    }

    public func snapshot() -> Snapshot {
        Snapshot(
            connected: _connected, stage: _stage, session: _session.map(sessionView), sections: _sections,
            nowPlaying: _nowPlaying, queue: _queue, activities: _activities, revision: _revision)
    }

    public func clientInfo() -> ClientInfo {
        ClientInfo(
            identifier: _settings.clientId, product: Config.PRODUCT, version: Config.VERSION,
            platform: environment.platform, device: environment.device, deviceName: environment.deviceName,
            platformVersion: environment.platformVersion)
    }

    /// The HTTP plumbing for a new Plex client: this app's headers and URL session.
    public func http() -> Http {
        Http(info: clientInfo(), session: environment.urlSession)
    }

    /// `new PlexTv(app.clientInfo(), token)`.
    public func newTv(_ token: String?) -> PlexTv {
        PlexTv(info: clientInfo(), token: token, http: http())
    }

    /// `new PlexServer(app.clientInfo(), url, token)`.
    public func newServer(_ url: String, _ token: String) throws -> PlexServer {
        try PlexServer(http: http(), baseUrl: url, token: token)
    }

    /// The settings as saved now.
    public func settings() -> Settings {
        Config.reloadSettings(environment, _settings)
    }

    /// Change and save the settings, showing an error toast when that fails.
    public func updateSettings(_ f: (inout Settings) -> Void) {
        do {
            try tryUpdateSettings(f)
        } catch {
            self.error("Could not save settings: \(errorText(error))")
        }
    }

    public func tryUpdateSettings(_ f: (inout Settings) throws -> Void) throws {
        _settings = try Config.updateSettings(environment, _settings, f)
    }

    public func session() -> Session? {
        _session
    }

    /// The connected server.
    public func server() throws -> PlexServer {
        guard let s = _session else { throw CoreError("Not connected to a server.") }
        return s.server
    }

    public func tv() -> PlexTv? {
        _session?.tv
    }

    /// The plex.tv client, or an error when not signed in to plex.tv.
    public func plexTv() throws -> PlexTv {
        guard let tv = tv() else { throw CoreError("Not signed in to plex.tv.") }
        return tv
    }

    public func connected() -> Bool {
        _connected
    }

    public func setConnected(_ online: Bool) {
        let was = _connected
        _connected = online
        if was != online {
            emit(.connection(online: online))
            if online {
                bump()
                Failure.onReconnect(self)
            }
        }
    }

    public func stage() -> Stage {
        _stage
    }

    public func setStage(_ stage: Stage) {
        _stage = stage
        emit(.stage(stage))
    }

    public func setSession(_ session: Session?, _ sections: [Directory]) {
        let view = session.map(sessionView)
        Undo.sessionChanged(self, session)
        Failure.sessionChanged(self, session)
        _session = session
        _sections = sections
        emit(.session(view, sections: sections))
    }

    public func sections() -> [Directory] {
        _sections
    }

    public func setSections(_ sections: [Directory]) {
        let session = _session.map(sessionView)
        _sections = sections
        emit(.session(session, sections: sections))
    }

    public func nowPlaying() -> NowPlaying? {
        _nowPlaying
    }

    public func setNowPlaying(_ np: NowPlaying?) {
        _nowPlaying = np
        emit(.nowPlaying(np))
    }

    public func queue() -> QueueView? {
        _queue
    }

    public func setQueue(_ queue: QueueView?) {
        _queue = queue
        emit(.queue(queue))
    }

    public func activities() -> [ServerActivity] {
        _activities
    }

    public func setActivities(_ list: [ServerActivity]) {
        _activities = list
        emit(.activities(list))
    }

    public func updateActivities(_ f: (inout [ServerActivity]) -> Void) {
        f(&_activities)
        emit(.activities(_activities))
    }

    public func revision() -> Int {
        _revision
    }

    /// Watch state may have changed; views refetch.
    public func bump() {
        _revision += 1
        emit(.revision(_revision))
        // Watch state may have changed: smart downloads catch up.
        Smart.reconcileSoon(self)
    }

    /// Like bump, telling front ends which sections and items changed.
    public func bumpWith(_ changes: Changes) {
        emit(.changes(changes))
        bump()
    }

    public func toast(_ text: String) {
        emit(.toast(Toast(text: text, error: false)))
    }

    /// A toast that offers to undo what it reports.
    public func toastUndo(_ text: String, _ undoId: Int) {
        emit(.toast(Toast(text: text, error: false, undo: undoId)))
    }

    public func error(_ text: String) {
        emit(.toast(Toast(text: text, error: true)))
    }

    public func error(_ e: any Error) {
        error(errorText(e))
    }
}
