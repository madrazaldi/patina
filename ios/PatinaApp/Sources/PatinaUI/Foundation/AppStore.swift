// Port of web/src/lib/state.svelte.ts: the front end's state. It receives every CoreEvent
// (it is the `EventSink` of the `AppState` it owns), mirrors what the screens read, and
// holds the UI-only state: navigation, the open sheet, the confirmation, toasts.

import Observation
import PatinaCore
import PatinaPlayer
import PlexKit
import SwiftUI

/// SwiftUI has a `Commands` protocol too; in this module `Commands` is the core's API.
typealias Commands = PatinaCore.Commands

public struct ToastItem: Identifiable, Equatable {
    public var id: Int
    public var text: String
    public var error: Bool
    /// Id of the undo entry that reverses what the toast reports.
    public var undo: Int?
}

/// An open "Are you sure?" (see `AppStore.confirm`).
public struct ConfirmRequest: Identifiable {
    public let id = UUID()
    public var title: String
    public var text: String?
    /// Label of the confirming button.
    public var action: String
    public var danger: Bool
    let resolve: (Bool) -> Void
}

/// The dialogs any screen can open on an item (`app.playlistTarget`, `app.editTarget`, ...).
public enum ItemSheet: Identifiable {
    /// `items` is a selection added together (bulk actions); empty for `item` alone.
    case playlist(Metadata, items: [Metadata])
    case collection(Metadata, items: [Metadata])
    case edit(Metadata)
    case artwork(Metadata, kind: PatinaCore.Commands.ArtworkKind)
    case match(Metadata)
    case subtitle(Metadata)
    case switchUser
    case customizeHome
    /// Keep the next unwatched items of a show, season or playlist downloaded.
    case keepDownloaded(Metadata)

    public var id: String {
        switch self {
        case .playlist(let m, let items): "playlist:\(m.ratingKey ?? ""):\(items.count)"
        case .collection(let m, let items): "collection:\(m.ratingKey ?? ""):\(items.count)"
        case .edit(let m): "edit:\(m.ratingKey ?? "")"
        case .artwork(let m, let kind): "artwork:\(m.ratingKey ?? ""):\(kind.rawValue)"
        case .match(let m): "match:\(m.ratingKey ?? "")"
        case .subtitle(let m): "subtitle:\(m.ratingKey ?? "")"
        case .switchUser: "switchUser"
        case .customizeHome: "customizeHome"
        case .keepDownloaded(let m): "keep:\(m.ratingKey ?? "")"
        }
    }
}

/// What the changes reported before a revision covered (`library-changed`).
struct ChangeLog {
    var revision: Int
    var changes: Changes
}

@MainActor
@Observable
public final class AppStore: EventSink {
    @ObservationIgnored public private(set) var app: AppState!
    @ObservationIgnored public let host = AppHost()
    /// The app's player (one for its lifetime), which the core drives; nil in previews or when
    /// it could not start.
    @ObservationIgnored public private(set) var engine: AetherPlaybackEngine?

    // Mirrors of the core (the CoreEvents).
    public internal(set) var connected = true
    public internal(set) var stage: Stage = .booting
    public internal(set) var session: SessionView?
    public internal(set) var sections: [Directory] = []
    public internal(set) var nowPlaying: NowPlaying?
    public internal(set) var queue: QueueView?
    /// Volume, speed, repeat, sleep timer, chapters and diagnostics of the running player.
    public internal(set) var playback: PlaybackState?
    /// Why playback last failed, until it is retried or dismissed.
    public internal(set) var failure: PlaybackFailure?
    /// The queue that was playing when Patina last stopped, offered as a Resume (never auto-played).
    public var savedQueue: PendingCommands.SavedQueueView?
    /// Server background tasks, live.
    public internal(set) var activities: [ServerActivity] = []
    public internal(set) var downloads: [Download] = []
    /// Bumped whenever watch state may have changed; views refetch.
    public internal(set) var revision = 0
    /// Bumped when playback sessions on the server change.
    public internal(set) var sessionsTick = 0
    public internal(set) var settings: PatinaCore.Commands.AppSettings?
    public var toasts: [ToastItem] = []
    /// Whether the server has a DVR (Live TV in the sidebar).
    public internal(set) var liveTv = false
    /// Whether a Seerr instance is set up (Requests in the sidebar).
    public internal(set) var seerr = false

    // Navigation: each destination keeps its own stack of routes.
    public var selection: Destination = .home
    public var paths: [Destination: [Route]] = [:]
    /// Set by the shell: a tab bar (iPhone) rather than a sidebar.
    public var compact = false

    // Overlays.
    /// The full-screen player is up (the mini player shows otherwise).
    public var playerOpen = false
    public var sheet: ItemSheet?
    public var confirming: ConfirmRequest?
    /// The command palette (⌘K).
    public var palette = false
    public var searchText = ""
    /// Bumped to put the cursor in the search field (⌘F).
    public var searchFocusRequest = 0

    @ObservationIgnored private var toastId = 0
    @ObservationIgnored private var pending: Changes?
    @ObservationIgnored private var log: [ChangeLog] = []
    @ObservationIgnored private var debugRouteDone = false
    @ObservationIgnored private var debugSignInDone = false
    @ObservationIgnored private var debugPlayDone = false
    /// Library pages, filters and scroll kept across visits (`libraryMemory`).
    @ObservationIgnored var libraryMemory: [(String, LibraryModel)] = []

    /// The app: the live environment (Keychain, standard directories) and the saved session.
    public convenience init() {
        let device = UIDevice.current
        self.init(
            environment: .live(device: device.model, deviceName: device.name, platformVersion: device.systemVersion),
            start: true)
    }

    init(environment: AppEnvironment, start: Bool) {
        app = AppState(sink: self, environment: environment)
        ArtworkLoader.app = app
        let snap = Commands.snapshot(app)
        connected = snap.connected
        stage = snap.stage
        session = snap.session
        sections = snap.sections
        nowPlaying = snap.nowPlaying
        queue = snap.queue
        activities = snap.activities
        settings = Commands.settings(app)
        downloads = Downloads.list(app.paths)
        seerr = PendingCommands.seerrConfigured(app)
        if start {
            let app = app!
            do {
                let engine = try AetherPlaybackEngine()
                Player.attach(app, engine)
                // Picture in Picture's "return to app" button brings the full player back.
                engine.pictureInPicture.onRestore = { [weak self] in self?.playerOpen = true }
                self.engine = engine
            } catch {
                logger("ui").error("the player could not start: \(errorText(error))")
            }
            // desktop/main.ts: `app.spawn(session.restore(app))`.
            app.spawn { await PatinaCore.Session.restore(app) }
        }
    }

    // MARK: Derived

    /// The active account and server; anything tied to one is dropped when it changes.
    public var scope: String { "\(session?.accountId ?? ""):\(session?.serverId ?? "")" }
    public var owned: Bool { session?.owned ?? false }
    public var hasTv: Bool { session?.hasTv ?? false }
    public var spoilers: Bool { settings?.ui.hideSpoilers ?? false }

    /// The user's accent (`ui.accent`: an accent id, or `#rrggbb`).
    public var accent: Color {
        guard let value = settings?.ui.accent, !value.isEmpty else { return Theme.defaultAccent }
        if let found = Config.ACCENTS.first(where: { $0.id == value }) {
            return Theme.color(hex: found.color) ?? Theme.defaultAccent
        }
        return Theme.color(hex: value) ?? Theme.defaultAccent
    }

    /// The machine id the item came from when it isn't the connected server.
    public func serverName(_ id: String?) -> String {
        guard let id else { return session?.serverName ?? "" }
        return session?.servers.first { $0.id == id }?.name ?? "another server"
    }

    // MARK: Events

    public func emit(_ event: CoreEvent) {
        switch event {
        case .connection(let online):
            connected = online
        case .settingsChanged:
            reloadSettings()
        case .stage(let s):
            stage = s
            if s == .ready { onReady() }
            #if DEBUG
                // Debug builds: PATINA_SIGNIN=<url>|<token> connects to a server (the mock) at the
                // sign-in screen, like `patina-tui login --url --token`.
                if s == .login, !debugSignInDone, let spec = ProcessInfo.processInfo.environment["PATINA_SIGNIN"],
                    let bar = spec.firstIndex(of: "|")
                {
                    debugSignInDone = true
                    let (url, token) = (String(spec[..<bar]), String(spec[spec.index(after: bar)...]))
                    perform { try await Commands.signInManual(self.app, url: url, token: token) }
                }
            #endif
        case .session(let view, let sections):
            let before = scope
            session = view
            if before != scope {
                resetNavigation()
                searchText = ""
                clearEditors()
                // Their entries belong to the previous account or server.
                toasts.removeAll { $0.undo != nil }
                savedQueue = nil
                libraryMemory.removeAll()
                loadSavedQueue()
                checkLiveTv()
            }
            self.sections = sections
        case .nowPlaying(let np):
            let started = nowPlaying == nil && np != nil
            nowPlaying = np
            // Once something plays the offer is moot (resuming, or the user chose otherwise).
            if np != nil { savedQueue = nil }
            // Video opens full screen; music stays in the mini player until it is tapped. A
            // failure keeps the player up to offer what to do.
            if started, let np, !PlexUI.isAudio(np.item) { playerOpen = true }
            if np == nil, failure == nil { playerOpen = false }
        case .queue(let q):
            queue = q
        case .activities(let list):
            activities = list
        case .revision:
            bump()
        case .changes(let c):
            noteChanges(c)
        case .toast(let t):
            push(t.text, error: t.error, undo: t.undo)
        case .downloads(let list):
            downloads = list
        case .sessionsChanged:
            sessionsTick += 1
        case .playbackState(let s):
            playback = s
        case .playbackFailure(let f):
            failure = f
            if f == nil, nowPlaying == nil { playerOpen = false }
        }
    }

    private func onReady() {
        checkLiveTv()
        #if DEBUG
            // Debug builds: PATINA_ROUTE=/item/100 (or /search?q=…) opens a page, as the desktop
            // dev build does.
            if !debugRouteDone, let spec = ProcessInfo.processInfo.environment["PATINA_ROUTE"],
                let url = URLComponents(string: spec), let route = Route.parse(url.path)
            {
                debugRouteDone = true
                if route == .search, let q = url.queryItems?.first(where: { $0.name == "q" })?.value { searchText = q }
                navigate(to: route)
            }
            // PATINA_PLAY=<ratingKey> starts playback on startup.
            if !debugPlayDone, let key = ProcessInfo.processInfo.environment["PATINA_PLAY"] {
                debugPlayDone = true
                perform {
                    guard let item = try await Commands.metadata(self.app, ratingKey: key) else { return }
                    try await PendingCommands.play(self.app, item: item)
                }
            }
        #endif
    }

    private func checkLiveTv() {
        liveTv = false
        guard stage == .ready, let id = session?.serverId else { return }
        Task {
            let on = await PendingCommands.livetvAvailable(app)
            if session?.serverId == id { liveTv = on }
        }
    }

    /// Look for a saved queue to offer, unless something is already playing.
    func loadSavedQueue() {
        guard nowPlaying == nil, session != nil else {
            savedQueue = nil
            return
        }
        let before = scope
        Task {
            // No offer is better than an error at start-up.
            guard let saved = try? await PendingCommands.savedQueue(app) else { return }
            if before == scope, nowPlaying == nil { savedQueue = saved }
        }
    }

    // MARK: Revisions

    /// Refetch everything that depends on watch state.
    public func refresh() {
        pending = nil
        bump()
    }

    private func noteChanges(_ c: Changes) {
        var p = pending ?? Changes()
        p.all = p.all || c.all
        for s in c.sections where !p.sections.contains(s) { p.sections.append(s) }
        for i in c.items where !p.items.contains(i) { p.items.append(i) }
        pending = p
    }

    /// A revision without a preceding `library-changed` could affect anything.
    private func bump() {
        let c = pending ?? Changes.everything()
        pending = nil
        revision += 1
        log.append(ChangeLog(revision: revision, changes: c))
        if log.count > 64 { log.removeFirst() }
    }

    /// Whether anything in library section `section` may have changed since `since`.
    public func affects(section: String, since: Int) -> Bool {
        if since >= revision { return false }
        guard let first = log.first, first.revision <= since + 1 else { return true }
        return log.contains { $0.revision > since && ($0.changes.all || $0.changes.sections.contains(section)) }
    }

    /// Whether item `ratingKey` may have changed since `since`.
    public func affects(item: String, since: Int) -> Bool {
        if since >= revision { return false }
        guard let first = log.first, first.revision <= since + 1 else { return true }
        return log.contains {
            $0.revision > since && ($0.changes.all || $0.changes.items.contains(item) || $0.changes.items.isEmpty)
        }
    }

    // MARK: Toasts

    public func toast(_ text: String) {
        push(text, error: false, undo: nil)
    }

    public func error(_ e: any Error) {
        push(errorText(e), error: true, undo: nil)
    }

    public func error(_ text: String) {
        push(text, error: true, undo: nil)
    }

    private func push(_ text: String, error: Bool, undo: Int?) {
        toastId += 1
        let id = toastId
        withAnimation(.spring(duration: 0.35)) {
            toasts.append(ToastItem(id: id, text: text, error: error, undo: undo))
            if toasts.count > 4 { toasts.removeFirst() }
        }
        if error { Haptics.error() }
        let ms = error ? 6000 : undo != nil ? 8000 : 3200
        Task {
            try? await Task.sleep(for: .milliseconds(ms))
            dismissToast(id)
        }
    }

    public func dismissToast(_ id: Int) {
        withAnimation(.spring(duration: 0.35)) { toasts.removeAll { $0.id == id } }
    }

    // MARK: Undo

    /// Reverse a reversible action: a toast's Undo passes its entry, ⌘Z the latest. The core
    /// announces the result.
    public func undo(_ entry: Int? = nil) {
        if let entry { toasts.removeAll { $0.undo == entry } }
        Task {
            do {
                try await Commands.undoAction(app, id: entry)
                Haptics.success()
            } catch {
                self.error(error)
            }
        }
    }

    /// ⌘Z.
    public func undoLatest() {
        guard let last = Commands.undoLast(app) else {
            toast("Nothing to undo")
            return
        }
        undo(last.id)
    }

    // MARK: Actions

    /// Run an action, toasting `ok` on success (and refreshing views if asked) or the error.
    @discardableResult
    public func act(_ ok: String? = nil, refresh: Bool = false, _ op: () async throws -> Void) async -> Bool {
        do {
            try await op()
            if let ok { toast(ok) }
            if refresh { self.refresh() }
            return true
        } catch is CancellationError {
            return false
        } catch {
            self.error(error)
            return false
        }
    }

    /// `act` without waiting for it.
    public func perform(_ ok: String? = nil, refresh: Bool = false, _ op: @escaping () async throws -> Void) {
        Task { await act(ok, refresh: refresh, op) }
    }

    /// Ask before something destructive; returns whether it was confirmed.
    public func confirm(_ title: String, text: String? = nil, action: String = "OK", danger: Bool = false) async -> Bool {
        confirming?.resolve(false)
        return await withCheckedContinuation { cont in
            confirming = ConfirmRequest(title: title, text: text, action: action, danger: danger) { cont.resume(returning: $0) }
        }
    }

    /// Answer the open confirmation.
    public func answer(_ ok: Bool) {
        let c = confirming
        confirming = nil
        c?.resolve(ok)
    }

    /// Close every dialog held here, so none can act on a previous account or server.
    func clearEditors() {
        sheet = nil
        palette = false
        answer(false)
    }

    // MARK: Settings

    func reloadSettings() {
        settings = Commands.settings(app)
        seerr = PendingCommands.seerrConfigured(app)
    }

    /// Change some appearance settings: `patch` holds the changed keys (`["saved_views": ...]`).
    public func updateUi(_ patch: [String: JSONValue]) {
        do {
            try Commands.setUiSettings(app, ui: .object(patch))
        } catch {
            self.error("Could not save settings: \(errorText(error))")
        }
        reloadSettings()
    }

    // MARK: Session

    public func switchServer(_ id: String) {
        guard id != session?.serverId else { return }
        resetNavigation()
        Task {
            do {
                try await Commands.switchServer(app, id: id)
            } catch {
                self.error(error)
            }
        }
    }

    public func switchUser(_ uuid: String, pin: String?) async -> Bool {
        do {
            try await Commands.switchUser(app, uuid: uuid, pin: pin)
            resetNavigation()
            return true
        } catch {
            self.error(error)
            return false
        }
    }

    public func signOut() {
        resetNavigation()
        Commands.signOut(app)
    }

    // MARK: Navigation

    /// Show `route`: a destination of the sidebar or tab bar is selected (and popped to its
    /// root); anything else is pushed onto the current destination's stack. Works from a sheet
    /// or the command palette, which close first.
    public func navigate(to route: Route) {
        sheet = nil
        palette = false
        if let (destination, path) = Destination.place(route, compact: compact) {
            selection = destination
            paths[destination] = path
            return
        }
        var path = paths[selection] ?? []
        if path.last == route { return }
        path.append(route)
        paths[selection] = path
    }

    /// Forget every stack (after switching account or server).
    public func resetNavigation() {
        paths = [:]
        selection = .home
    }

    /// The stack of `destination`, for `NavigationStack(path:)`.
    public func path(_ destination: Destination) -> Binding<[Route]> {
        Binding(get: { self.paths[destination] ?? [] }, set: { self.paths[destination] = $0 })
    }

    /// ⌘F: show Search and put the cursor in its field.
    public func focusSearch() {
        navigate(to: .search)
        searchFocusRequest += 1
    }

    // MARK: Library memory

    /// The model of a library, kept across visits so filters, pages and the scroll position
    /// survive (`rememberLibrary`, at most 8).
    func libraryModel(_ id: String) -> LibraryModel {
        let key = "\(scope):\(id)"
        if let at = libraryMemory.firstIndex(where: { $0.0 == key }) {
            let found = libraryMemory.remove(at: at)
            libraryMemory.append(found)
            return found.1
        }
        let made = LibraryModel(id: id, store: self)
        libraryMemory.append((key, made))
        while libraryMemory.count > 8 { libraryMemory.removeFirst() }
        return made
    }
}

/// What only the app can do (Types.swift): open a URL, or a folder in the Files app.
public final class AppHost: PatinaCore.Host {
    public func openUrl(_ url: String) async throws {
        guard let u = URL(string: url) else { throw CoreError("Not a valid address: \(url)") }
        await UIApplication.shared.open(u)
    }

    public func openPath(_ path: String) async throws {
        // `shareddocuments://` opens the Files app at a folder of the app's Documents.
        guard let u = URL(string: "shareddocuments://\(path)") else { throw CoreError("Not a valid path: \(path)") }
        await UIApplication.shared.open(u)
    }
}

// MARK: Previews

extension AppStore {
    /// A store with fixture state and no server, for `#Preview`s.
    static func preview(stage: Stage = .ready, configure: (AppStore) -> Void = { _ in }) -> AppStore {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("patina-preview-\(UUID().uuidString)")
        let env = AppEnvironment(
            paths: .under(root), secrets: MemorySecretStore(), device: "iPhone", deviceName: "Preview",
            tickInterval: nil)
        let store = AppStore(environment: env, start: false)
        store.stage = stage
        store.sections = Fixtures.sections
        configure(store)
        return store
    }
}
