import AppKit
import Foundation
import Observation
import SwiftUI

/// Top-level places in the sidebar.
enum SidebarItem: Hashable {
    case home, search, watchlist, playlists, livetv, downloads, activity, server
    case library(String)
}

/// Pages pushed onto a sidebar place's navigation stack.
enum Route: Hashable {
    case item(String)
    case playlist(String)
    /// Any server listing: "See all" for hubs, tag filters (genre, actor, ...).
    case browse(path: String, title: String)
    case library(String)

    /// Where selecting an item should go.
    static func of(_ item: Metadata) -> Route {
        item.type == "playlist" ? .playlist(item.ratingKey ?? "") : .item(item.ratingKey ?? "")
    }
}

/// Sheets for an item, opened from menus anywhere.
enum ItemSheet: Identifiable {
    case playlist(Metadata)
    case collection(Metadata)
    case edit(Metadata)
    case artwork(Metadata, kind: String)
    case match(Metadata)
    case subtitles(Metadata)
    case tracks(Metadata)

    var id: String {
        switch self {
        case .playlist(let m): "playlist:\(m.id)"
        case .collection(let m): "collection:\(m.id)"
        case .edit(let m): "edit:\(m.id)"
        case .artwork(let m, let kind): "artwork:\(kind):\(m.id)"
        case .match(let m): "match:\(m.id)"
        case .subtitles(let m): "subtitles:\(m.id)"
        case .tracks(let m): "tracks:\(m.id)"
        }
    }
}

/// A destructive action waiting for confirmation.
struct Confirmation: Identifiable {
    let id = UUID()
    var title: String
    var message: String?
    var action: String
    var destructive: Bool
    var perform: () async -> Void
}

/// Global app state. Mirrors the backend through its events (see
/// `CoreEvent` in crates/core/src/state.rs), plus UI-only state.
@MainActor
@Observable
final class AppModel {
    static let shared = AppModel()

    var connected = true
    var stage: Stage = .booting
    var session: SessionView?
    var sections: [Directory] = []
    var nowPlaying: NowPlaying?
    var queue: QueueView?
    /// Server background tasks, live.
    var activities: [ServerActivity] = []
    var downloads: [Download] = []
    /// Whether the server has a DVR (Live TV page).
    var liveTv = false
    /// Bumped whenever watch state may have changed; views refetch.
    var revision = 0
    /// Bumped when playback sessions on the server change.
    var sessionsTick = 0
    var settings: Settings?
    var info: Info?
    var toasts: [Toast] = []
    /// Bulk audio/subtitle apply progress: (done, total).
    var tracksProgress: (Int, Int)?

    // Navigation
    var selection: SidebarItem? = .home
    var paths: [SidebarItem: [Route]] = [:]
    var searchText = ""

    // Overlays
    var sheet: ItemSheet?
    var confirmation: Confirmation?
    var showQueue = false
    var showLyrics = false
    /// Bumped to open the Settings window (RootView owns `openSettings`).
    var settingsRequest = 0

    private var started = false
    /// Events that arrive before the startup snapshot is applied, replayed after it
    /// so the snapshot can't overwrite newer state. Nil once replayed.
    private var pendingEvents: [(String, Data)]? = []

    var owned: Bool { session?.owned ?? false }
    var hasTv: Bool { session?.hasTv ?? false }
    var spoilers: Bool { settings?.ui.hide_spoilers ?? false }
    var posterWidth: CGFloat { CGFloat(settings?.ui.poster_size ?? 160) }
    /// Account and server; layouts and saved views are stored per scope.
    var scope: String { "\(session?.accountId ?? ""):\(session?.serverId ?? "")" }

    var accent: Color {
        let id = settings?.ui.accent ?? "paper"
        guard id != "paper",
              let hex = info?.accents.first(where: { $0.id == id })?.color,
              let color = Color(hex: hex)
        else { return Theme.defaultAccent }
        return color
    }

    /// Text and symbols drawn on the accent. Paper flips with the appearance;
    /// the other accents are mid-tones that take black in both.
    var onAccent: Color {
        (settings?.ui.accent ?? "paper") == "paper" ? Color(nsColor: .windowBackgroundColor) : .black
    }

    // MARK: Startup and events

    func start() {
        guard !started else { return }
        started = true
        Core.start { [weak self] name, payload in self?.receive(name, payload) }
        Task {
            defer { replayPendingEvents() }
            do {
                async let snap = API.snapshot()
                async let settings = API.settings()
                async let info = API.info()
                async let downloads = API.downloads()
                let s = try await snap
                connected = s.connected
                stage = s.stage
                session = s.session
                sections = s.sections
                nowPlaying = s.nowPlaying
                queue = s.queue
                activities = s.activities
                self.settings = try await settings
                self.info = try await info
                self.downloads = try await downloads
                refreshLiveTv()
                openDebugRoute()
            } catch {
                stage = .failed(message: error.localizedDescription)
            }
        }
    }

    private func decode<T: Decodable>(_ type: T.Type, _ data: Data) -> T? {
        do { return try JSONDecoder.patina.decode(T.self, from: data) } catch {
            NSLog("Patina: bad event payload for %@: %@", String(describing: T.self), String(describing: error))
            return nil
        }
    }

    private func receive(_ name: String, _ data: Data) {
        if pendingEvents != nil {
            pendingEvents?.append((name, data))
        } else {
            handle(name, data)
        }
    }

    private func replayPendingEvents() {
        let events = pendingEvents ?? []
        pendingEvents = nil
        for (name, data) in events { handle(name, data) }
    }

    private func handle(_ name: String, _ data: Data) {
        switch name {
        case "connection": connected = decode(Bool.self, data) ?? connected
        case "settings-changed": Task { await reloadSettings() }
        case "stage":
            if let s = decode(Stage.self, data) {
                stage = s
                if case .ready = s { refreshLiveTv(); openDebugRoute() }
            }
        case "session":
            struct Payload: Decodable { var session: SessionView?; var sections: [Directory] }
            guard let p = decode(Payload.self, data) else { return }
            let before = scope
            session = p.session
            sections = p.sections
            if before != scope {
                resetNavigation()
                refreshLiveTv()
            }
        case "now-playing": nowPlaying = decode(NowPlaying?.self, data) ?? nil
        case "queue":
            queue = decode(QueueView?.self, data) ?? nil
            // The queue only goes away when playback ends, unlike now-playing between tracks.
            if queue == nil { showQueue = false; showLyrics = false }
        case "activities": activities = decode([ServerActivity].self, data) ?? activities
        case "downloads": downloads = decode([Download].self, data) ?? downloads
        case "sessions-changed": sessionsTick += 1
        case "revision": revision += 1
        case "toast":
            if let t = decode(Toast.self, data) { push(t) }
        case "tracks-progress":
            if let p = decode([Int].self, data), p.count == 2 { tracksProgress = (p[0], p[1]) }
        default: break
        }
    }

    private func refreshLiveTv() {
        liveTv = false
        guard case .ready = stage, let id = session?.serverId else { return }
        Task {
            let on = (try? await API.livetvAvailable()) ?? false
            if session?.serverId == id { liveTv = on }
        }
    }

    // MARK: Navigation

    private var debugRouteDone = false

    /// Debug builds: `PATINA_ROUTE=/item/100` (or /library/1, /playlist/5,
    /// /browse/<path>, /search?q=, /downloads, /livetv, /activity, /server,
    /// /playlists, /watchlist, /settings) opens a page once signed in.
    func openDebugRoute() {
        #if DEBUG
        guard !debugRouteDone, case .ready = stage,
              let raw = ProcessInfo.processInfo.environment["PATINA_ROUTE"] else { return }
        debugRouteDone = true
        let (path, query) = raw.split(separator: "?", maxSplits: 1).map(String.init).splitPair()
        let parts = path.split(separator: "/", maxSplits: 1).map(String.init)
        let head = parts.first ?? "", rest = parts.count > 1 ? parts[1] : ""
        switch head {
        case "library": selection = .library(rest)
        case "item": open(.item(rest))
        case "playlist": selection = .playlists; open(.playlist(rest))
        case "browse": open(.browse(path: "/" + rest, title: "Browse"))
        case "search":
            selection = .search
            searchText = query.flatMap { URLComponents(string: "?" + $0)?.queryItems?.first?.value } ?? ""
        case "downloads": selection = .downloads
        case "livetv": selection = .livetv
        case "activity": selection = .activity
        case "server": selection = .server
        case "playlists": selection = .playlists
        case "watchlist": selection = .watchlist
        case "settings": settingsRequest += 1
        default: break
        }
        #endif
    }

    func resetNavigation() {
        paths = [:]
        selection = .home
        searchText = ""
        sheet = nil
        confirmation = nil
    }

    /// Push a page onto the current sidebar place's stack.
    func open(_ route: Route) {
        let place = selection ?? .home
        paths[place, default: []].append(route)
    }

    func open(_ item: Metadata) {
        open(Route.of(item))
    }

    func path(for place: SidebarItem) -> Binding<[Route]> {
        Binding(get: { self.paths[place] ?? [] }, set: { self.paths[place] = $0 })
    }

    // MARK: Feedback

    /// Refetch everything that depends on watch state.
    func refresh() {
        revision += 1
    }

    func toast(_ text: String) {
        push(Toast(text: text, error: false))
    }

    func error(_ e: Error) {
        push(Toast(text: e.localizedDescription, error: true))
    }

    func error(_ text: String) {
        push(Toast(text: text, error: true))
    }

    private func push(_ t: Toast) {
        toasts.append(t)
        let id = t.id
        Task {
            try? await Task.sleep(for: .seconds(t.error ? 6 : 3.2))
            toasts.removeAll { $0.id == id }
        }
    }

    /// Ask before something destructive.
    func confirm(_ title: String, message: String? = nil, action: String = "OK", destructive: Bool = false, perform: @escaping () async -> Void) {
        confirmation = Confirmation(title: title, message: message, action: action, destructive: destructive, perform: perform)
    }

    /// Run an action, toasting `ok` on success (and refreshing views if asked) or the error.
    @discardableResult
    func act(_ ok: String? = nil, refresh: Bool = false, _ work: () async throws -> Void) async -> Bool {
        do {
            try await work()
            if let ok { toast(ok) }
            if refresh { self.refresh() }
            return true
        } catch {
            self.error(error)
            return false
        }
    }

    /// `act` from a synchronous context, e.g. a button.
    func perform(_ ok: String? = nil, refresh: Bool = false, _ work: @escaping () async throws -> Void) {
        Task { await act(ok, refresh: refresh, work) }
    }

    // MARK: Settings

    func reloadSettings() async {
        do { settings = try await API.settings() } catch { self.error(error) }
    }

    /// Save player settings fields (snake_case keys), updating the local copy first.
    func updatePlayer(_ patch: [String: JSON], apply: (inout PlayerSettings) -> Void) {
        if settings != nil { apply(&settings!.player) }
        perform { try await API.setPlayerSettings(patch) }
    }

    /// Save UI settings fields (snake_case keys), updating the local copy first.
    func updateUi(_ patch: [String: JSON], apply: (inout UiSettings) -> Void) {
        if settings != nil { apply(&settings!.ui) }
        perform { try await API.setUiSettings(patch) }
    }
}

extension JSON {
    /// A JSON value from any Encodable, for settings patches.
    static func from<T: Encodable>(_ value: T) -> JSON {
        (try? JSONEncoder.patina.encode(value)).flatMap { try? JSONDecoder.patina.decode(JSON.self, from: $0) } ?? .null
    }
}

private extension Array where Element == String {
    func splitPair() -> (String, String?) {
        (first.map { $0.hasPrefix("/") ? String($0.dropFirst()) : $0 } ?? "", count > 1 ? self[1] : nil)
    }
}
