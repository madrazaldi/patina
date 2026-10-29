// Port of web/src/components/CommandPalette.svelte: the command palette (⌘K). Fuzzy-filtered
// navigation, libraries, playlists, saved views, playback, servers and settings, plus (unlike
// the web, whose last row only hands the text to Search) the items the server finds for the
// text. Return runs the first result.

import PatinaCore
import PlexKit
import SwiftUI

struct CommandPalette: View {
    struct Command: Identifiable {
        var id: String
        var label: String
        var group: String
        var icon: String
        /// Extra words the filter also looks at.
        var keywords = ""
        var run: () -> Void
    }

    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var query = ""
    @State private var playlists: [Metadata] = []
    @State private var items: (q: String, items: [Metadata])?
    @FocusState private var focused: Bool

    private var trimmed: String { query.trimmingCharacters(in: .whitespacesAndNewlines) }

    var body: some View {
        NavigationStack {
            List {
                ForEach(groups, id: \.0) { group, commands in
                    Section(group) {
                        ForEach(commands) { c in row(c) }
                    }
                }
                if trimmed.count >= 2, let found = items, found.q == trimmed, !found.items.isEmpty {
                    Section("Items") {
                        ForEach(found.items.prefix(8), id: \.ratingKey) { m in itemRow(m) }
                    }
                }
                Section {
                    row(search)
                }
            }
            .listStyle(.insetGrouped)
            .safeAreaInset(edge: .top) {
                HStack(spacing: 8) {
                    Image(systemName: "magnifyingglass").foregroundStyle(Theme.secondaryText)
                    TextField("Go to, play, switch…", text: $query)
                        .focused($focused)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .submitLabel(.go)
                        .onSubmit { run(results.first ?? search) }
                }
                .padding(12)
                .background(Theme.raised, in: .rect(cornerRadius: 12, style: .continuous))
                .padding(.horizontal, 16)
                .padding(.bottom, 6)
                .background(.bar)
            }
            .navigationTitle("Command Palette")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() } }
            }
        }
        .onAppear { focused = true }
        .task {
            let scope = store.scope
            let found = (try? await Commands.playlists(store.app)) ?? []
            if scope == store.scope { playlists = found }
        }
        // Items as you type, debounced.
        .task(id: trimmed) {
            let q = trimmed
            guard q.count >= 2 else { return }
            try? await Task.sleep(for: .milliseconds(220))
            if Task.isCancelled { return }
            let hubs = (try? await Commands.search(store.app, query: q)) ?? []
            if !Task.isCancelled && q == trimmed {
                items = (q, hubs.flatMap(\.metadata))
            }
        }
    }

    private func row(_ c: Command) -> some View {
        Button {
            run(c)
        } label: {
            Label(c.label, systemImage: c.icon).foregroundStyle(.primary)
        }
    }

    private func itemRow(_ m: Metadata) -> some View {
        Button {
            dismiss()
            store.navigate(to: PlexUI.route(m))
        } label: {
            HStack(spacing: 12) {
                Artwork(Metadata.poster(m), width: 96, height: 144)
                    .frame(width: 28, height: 42)
                    .clipShape(.rect(cornerRadius: 4))
                VStack(alignment: .leading, spacing: 1) {
                    Text(Metadata.title(m)).foregroundStyle(.primary).lineLimit(1)
                    Text(PlexUI.subtitle(m)).font(.caption).foregroundStyle(Theme.secondaryText).lineLimit(1)
                }
            }
        }
    }

    private func run(_ c: Command) {
        dismiss()
        c.run()
    }

    /// "Search for …" is always last: it hands the text to the Search page.
    private var search: Command {
        let q = trimmed
        return Command(id: "search", label: q.isEmpty ? "Search" : "Search for “\(q)”", group: "Search", icon: "magnifyingglass") {
            if !q.isEmpty { store.searchText = q }
            store.focusSearch()
        }
    }

    private var results: [Command] {
        let q = trimmed
        if q.isEmpty { return commands }
        return commands.compactMap { c in Fuzzy.match(q, label: c.label, keywords: "\(c.keywords) \(c.group)").map { (c, $0) } }
            .sorted { $0.1 > $1.1 }
            .map(\.0)
    }

    /// The results by group, in the order the groups first appear (while filtering, one list by score).
    private var groups: [(String, [Command])] {
        if !trimmed.isEmpty { return results.isEmpty ? [] : [("Results", results)] }
        var out: [(String, [Command])] = []
        for c in results {
            if let at = out.firstIndex(where: { $0.0 == c.group }) { out[at].1.append(c) } else { out.append((c.group, [c])) }
        }
        return out
    }

    private var commands: [Command] {
        var out: [Command] = []
        let app = store.app!
        func go(_ id: String, _ label: String, _ icon: String, _ route: Route, _ keywords: String = "", group: String = "Go To") {
            out.append(Command(id: id, label: label, group: group, icon: icon, keywords: keywords) { store.navigate(to: route) })
        }
        go("home", "Home", "house", .home)
        go("pick", "Pick Something to Watch", "dice", .pick, "random choose decide movie night surprise me what to watch", group: "Actions")
        go("playlists", "Playlists", "music.note.list", .playlists)
        if store.hasTv { go("watchlist", "Watchlist", "bookmark", .watchlist, "plex discover bookmarks") }
        if store.liveTv { go("livetv", "Live TV", "dot.radiowaves.left.and.right", .livetv, "dvr channels guide") }
        if !store.downloads.isEmpty { go("downloads", "Downloads", "arrow.down.circle", .downloads, "offline") }
        if store.seerr { go("requests", "Requests", "tray", .requests, "seerr overseerr jellyseerr request discover missing") }
        go("activity", "Activity", "waveform.path.ecg", .activity, "sessions history tasks")
        if store.owned { go("server", "Server", "server.rack", .server, "administration libraries maintenance") }
        go("settings", "Settings", "gearshape", .settings, "preferences player appearance accent")
        if !(store.paths[store.selection] ?? []).isEmpty {
            out.append(Command(id: "back", label: "Go Back", group: "Go To", icon: "chevron.left", keywords: "previous page history") {
                store.paths[store.selection]?.removeLast()
            })
        }
        for s in store.sections {
            go("library:\(s.key ?? "")", s.title ?? "Library", "books.vertical", .library(id: s.key ?? ""), "library section", group: "Libraries")
        }
        for p in playlists {
            go("playlist:\(p.ratingKey ?? "")", p.title ?? "Playlist", "music.note.list", .playlist(id: p.ratingKey ?? ""), "playlist", group: "Playlists")
        }
        if let np = store.nowPlaying {
            func control(_ id: String, _ label: String, _ icon: String, _ command: PatinaCore.Player.Control, _ keywords: String = "") {
                out.append(Command(id: id, label: label, group: "Playback", icon: icon, keywords: keywords) {
                    Commands.control(app, command: command)
                })
            }
            control("pause", np.paused ? "Play" : "Pause", np.paused ? "play.fill" : "pause.fill", .togglePause, "resume toggle")
            if np.queuePos + 1 < np.queueLen { control("next", "Next", "forward.end.fill", .next, "skip forward track episode") }
            if np.queuePos > 0 { control("previous", "Previous", "backward.end.fill", .previous, "back track episode") }
            control("stop", "Stop Playback", "stop.fill", .stop, "end quit")
            if !np.live {
                control("back10", "Back 10 Seconds", "gobackward.10", .seekRelative(seconds: -10), "rewind seek")
                control("forward30", "Forward 30 Seconds", "goforward.30", .seekRelative(seconds: 30), "skip seek")
            }
            out.append(Command(id: "player", label: "Open Player", group: "Playback", icon: "play.rectangle", keywords: "queue lyrics now playing") {
                afterClosing { store.playerOpen = true }
            })
        }
        for s in store.sections {
            for v in Commands.savedViews(app, library: s.key ?? "") {
                out.append(Command(id: "view:\(v.id)", label: "\(s.title ?? ""): \(v.name)", group: "Saved Views", icon: "film", keywords: "view filter") {
                    store.libraryModel(v.library).useView(v)
                    store.navigate(to: .library(id: v.library))
                })
            }
        }
        if let session = store.session {
            for s in session.servers where s.id != session.serverId {
                out.append(Command(id: "server:\(s.id)", label: "Switch to \(s.name)", group: "Servers", icon: "arrow.left.arrow.right", keywords: "server change") {
                    store.switchServer(s.id)
                })
            }
        }
        if let undo = Commands.undoLast(app) {
            out.append(Command(id: "undo", label: "Undo: \(undo.label)", group: "Actions", icon: "arrow.uturn.backward", keywords: "revert reverse watched playlist continue watching") {
                store.undo(undo.id)
            })
        }
        out.append(Command(id: "user", label: "Switch User…", group: "Account", icon: "person.crop.circle", keywords: "profile account") {
            afterClosing { store.sheet = .switchUser }
        })
        let spoilers = store.spoilers
        out.append(Command(id: "spoilers", label: spoilers ? "Show Spoilers" : "Hide Spoilers", group: "Settings", icon: spoilers ? "eye" : "eye.slash", keywords: "unwatched thumbnails summaries") {
            store.updateUi(["hide_spoilers": .bool(!spoilers)])
        })
        out.append(Command(id: "refresh", label: "Refresh Libraries", group: "Settings", icon: "arrow.clockwise", keywords: "reload reconnect retry") {
            store.refresh()
            store.perform { try await Commands.reloadSections(app) }
        })
        return out
    }

    /// Another sheet or cover can only come up once the palette has gone.
    private func afterClosing(_ open: @escaping () -> Void) {
        Task {
            try? await Task.sleep(for: .milliseconds(450))
            open()
        }
    }
}
