import SwiftUI

/// The signed-in window: sidebar of places and libraries, a navigation stack
/// per place, and the now-playing bar.
struct Shell: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        @Bindable var app = app
        NavigationSplitView {
            Sidebar()
                .navigationSplitViewColumnWidth(min: 190, ideal: 214, max: 280)
        } detail: {
            let place = app.selection ?? .home
            NavigationStack(path: app.path(for: place)) {
                page(place)
                    .navigationDestination(for: Route.self) { route in
                        destination(route)
                    }
            }
            .id(place)
        }
        .searchable(text: $app.searchText, placement: .sidebar, prompt: "Search")
        .onChange(of: app.searchText) { _, text in
            if !text.isEmpty && app.selection != .search { app.selection = .search }
        }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            if app.nowPlaying != nil || app.queue != nil { NowPlayingBar() }
        }
        .sheet(item: $app.sheet) { sheet in
            ItemSheetView(sheet: sheet)
        }
        .confirmationDialog(
            app.confirmation?.title ?? "",
            isPresented: Binding(get: { app.confirmation != nil }, set: { if !$0 { app.confirmation = nil } }),
            titleVisibility: .visible,
            presenting: app.confirmation
        ) { c in
            Button(c.action, role: c.destructive ? .destructive : nil) { Task { await c.perform() } }
            Button("Cancel", role: .cancel) {}
        } message: { c in
            if let m = c.message { Text(m) }
        }
    }

    @ViewBuilder private func page(_ place: SidebarItem) -> some View {
        switch place {
        case .home: HomePage()
        case .search: SearchPage()
        case .watchlist: WatchlistPage()
        case .playlists: PlaylistsPage()
        case .livetv: LiveTVPage()
        case .downloads: DownloadsPage()
        case .activity: ActivityPage()
        case .server: ServerPage()
        case .library(let id): LibraryPage(id: id)
        }
    }

    @ViewBuilder private func destination(_ route: Route) -> some View {
        switch route {
        case .item(let key): ItemPage(ratingKey: key)
        case .playlist(let id): PlaylistPage(id: id)
        case .browse(let path, let title): BrowsePage(path: path, title: title)
        case .library(let id): LibraryPage(id: id)
        }
    }
}

struct Sidebar: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        @Bindable var app = app
        let scanning = Set(app.activities.compactMap(\.sectionId))
        let activeDownloads = app.downloads.filter { $0.state == .queued || $0.state == .downloading }.count
        List(selection: $app.selection) {
            Section {
                Label("Home", systemImage: "house").tag(SidebarItem.home)
                if app.hasTv { Label("Watchlist", systemImage: "bookmark").tag(SidebarItem.watchlist) }
                Label("Playlists", systemImage: "music.note.list").tag(SidebarItem.playlists)
                if app.liveTv { Label("Live TV", systemImage: "antenna.radiowaves.left.and.right").tag(SidebarItem.livetv) }
                if !app.downloads.isEmpty {
                    Label("Downloads", systemImage: "arrow.down.circle")
                        .badge(activeDownloads)
                        .tag(SidebarItem.downloads)
                }
            }
            if !app.sections.isEmpty {
                Section("Libraries") {
                    ForEach(app.sections) { s in
                        Label {
                            HStack {
                                Text(s.title ?? "")
                                if scanning.contains(s.key ?? "") {
                                    Spacer()
                                    ProgressView().controlSize(.mini)
                                }
                            }
                        } icon: {
                            Image(systemName: Self.icon(s.type))
                        }
                        .tag(SidebarItem.library(s.key ?? ""))
                    }
                }
            }
            Section("Server") {
                Label("Activity", systemImage: "waveform.path.ecg").tag(SidebarItem.activity)
                if app.owned { Label("Manage", systemImage: "server.rack").tag(SidebarItem.server) }
            }
        }
        .listStyle(.sidebar)
        .safeAreaInset(edge: .bottom, spacing: 0) {
            VStack(spacing: 0) {
                SidebarStatus()
                AccountMenu()
            }
        }
    }

    static func icon(_ type: String?) -> String {
        switch type {
        case "movie": "film"
        case "show": "tv"
        case "artist": "music.note"
        case "photo": "photo.on.rectangle"
        default: "square.stack"
        }
    }
}

/// Connection state and the running server task, under the sidebar.
struct SidebarStatus: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if !app.connected {
                Label("Reconnecting…", systemImage: "wifi.exclamationmark")
                    .font(Theme.caption).foregroundStyle(.secondary)
            }
            if let task = app.activities.first {
                VStack(alignment: .leading, spacing: 3) {
                    Text(task.title).font(Theme.caption).lineLimit(1)
                    if let s = task.subtitle { Text(s).font(.system(size: 10)).foregroundStyle(.secondary).lineLimit(1) }
                    ProgressView(value: Double(task.progress), total: 100).controlSize(.mini).tint(app.accent)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 14)
        .padding(.vertical, app.connected && app.activities.isEmpty ? 0 : 10)
    }
}

/// The signed-in user: switch server or Home user, sign out.
struct AccountMenu: View {
    @Environment(AppModel.self) private var app
    @State private var users: [HomeUser] = []
    @State private var pinFor: HomeUser?
    @State private var pin = ""

    var body: some View {
        let session = app.session
        Menu {
            Section(session?.user?.name ?? "Server owner") {
                Text(session?.serverName ?? "")
            }
            if let servers = session?.servers, servers.count > 1 {
                Menu("Switch Server") {
                    ForEach(servers) { s in
                        Button {
                            app.resetNavigation()
                            app.perform { try await API.switchServer(s.id) }
                        } label: {
                            if s.id == session?.serverId { Label(s.name, systemImage: "checkmark") } else { Text(s.name) }
                        }
                        .disabled(!s.presence && s.id != session?.serverId)
                    }
                }
            }
            if users.count > 1 {
                Menu("Switch User") {
                    ForEach(users) { u in
                        Button(u.title ?? u.username ?? "") {
                            if u.protected { pin = ""; pinFor = u } else { switchTo(u, pin: nil) }
                        }
                    }
                }
            }
            Divider()
            Button("Reload Libraries") { app.perform { try await API.reloadSections() } }
            Button("Sign Out") {
                app.confirm("Sign out of Plex?", action: "Sign Out") {
                    await app.act { try await API.signOut() }
                }
            }
        } label: {
            HStack(spacing: 8) {
                if let user = session?.user {
                    Avatar(path: user.thumb, name: user.name, size: 24)
                } else {
                    Image(systemName: "person.crop.circle").font(.system(size: 20)).foregroundStyle(.secondary)
                }
                VStack(alignment: .leading, spacing: 0) {
                    Text(session?.user?.name ?? "Server owner").font(.system(size: 12, weight: .medium)).lineLimit(1)
                    Text(session?.serverName ?? "").font(.system(size: 10)).foregroundStyle(.secondary).lineLimit(1)
                }
                Spacer(minLength: 0)
                Image(systemName: "chevron.up.chevron.down").font(.system(size: 9)).foregroundStyle(.tertiary)
            }
            .contentShape(Rectangle())
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .help("Account and server")
        .task(id: app.scope) { users = (try? await API.homeUsers()) ?? [] }
        .alert("Enter PIN for \(pinFor?.title ?? "")", isPresented: Binding(get: { pinFor != nil }, set: { if !$0 { pinFor = nil } })) {
            SecureField("PIN", text: $pin)
            Button("Switch") { if let u = pinFor { switchTo(u, pin: pin) } }
            Button("Cancel", role: .cancel) {}
        }
    }

    private func switchTo(_ u: HomeUser, pin: String?) {
        guard let uuid = u.uuid else { return }
        app.resetNavigation()
        app.perform { try await API.switchUser(uuid, pin: pin) }
    }
}

/// Transient messages in the corner of the window.
struct Toasts: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        VStack(alignment: .trailing, spacing: 8) {
            ForEach(app.toasts) { t in
                HStack(spacing: 8) {
                    if t.error { Image(systemName: "exclamationmark.circle").foregroundStyle(.red) }
                    Text(t.text).font(.callout).lineLimit(4).textSelection(.enabled)
                }
                .padding(.horizontal, 14).padding(.vertical, 10)
                .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8))
                .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Theme.hairline))
                .frame(maxWidth: 360, alignment: .trailing)
                .transition(.move(edge: .trailing).combined(with: .opacity))
                .onTapGesture { app.toasts.removeAll { $0.id == t.id } }
            }
        }
        .padding(18)
        .padding(.bottom, app.nowPlaying != nil ? 64 : 0)
        .animation(.easeOut(duration: 0.2), value: app.toasts)
    }
}
