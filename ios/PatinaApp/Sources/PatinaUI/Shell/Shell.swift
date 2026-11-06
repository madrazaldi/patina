// Port of web/src/components/Shell.svelte: the signed-in layout. On iPad (regular width) a
// sidebar lists the places, the libraries and the account; on iPhone (compact) a tab bar
// holds Home, Libraries, Search, Downloads and More. Each place has its own navigation stack.

import PatinaCore
import PlexKit
import SwiftUI

struct Shell: View {
    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        Group {
            if sizeClass == .compact {
                TabShell()
            } else {
                SplitShell()
            }
        }
        .onAppear { adapt() }
        .onChange(of: sizeClass) { adapt() }
    }

    /// Keep the selection valid when the layout changes (iPad Split View, rotation).
    private func adapt() {
        let compact = sizeClass == .compact
        store.compact = compact
        let tabs: [Destination] = [.home, .libraries, .search, .downloads, .more]
        let current = store.selection
        if compact && !tabs.contains(current) {
            // A sidebar page becomes the first page of the Libraries or More tab.
            let target: Destination
            if case .library = current { target = .libraries } else { target = .more }
            store.paths[target] = (current.root.map { [$0] } ?? []) + (store.paths[current] ?? [])
            store.selection = target
        } else if !compact && (current == .libraries || current == .more) {
            // And back: the tab's first page becomes the sidebar selection.
            let path = store.paths[current] ?? []
            if let first = path.first, let (top, _) = Destination.place(first, compact: false) {
                store.paths[top] = Array(path.dropFirst())
                store.selection = top
            } else {
                store.selection = .home
            }
        }
    }
}

// MARK: iPhone

struct TabShell: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        @Bindable var store = store
        TabView(selection: $store.selection) {
            Tab(Destination.home.title, systemImage: Destination.home.systemImage, value: Destination.home) {
                DestinationStack(destination: .home)
            }
            Tab(Destination.libraries.title, systemImage: Destination.libraries.systemImage, value: Destination.libraries) {
                DestinationStack(destination: .libraries)
            }
            Tab(value: Destination.search, role: .search) {
                DestinationStack(destination: .search)
            }
            Tab(Destination.downloads.title, systemImage: Destination.downloads.systemImage, value: Destination.downloads) {
                DestinationStack(destination: .downloads)
            }
            .badge(activeDownloads)
            Tab(Destination.more.title, systemImage: Destination.more.systemImage, value: Destination.more) {
                DestinationStack(destination: .more)
            }
        }
    }

    private var activeDownloads: Int {
        store.downloads.filter { $0.state == .queued || $0.state == .downloading }.count
    }
}

// MARK: iPad

struct SplitShell: View {
    @Environment(AppStore.self) private var store
    @State private var columns = NavigationSplitViewVisibility.all

    var body: some View {
        NavigationSplitView(columnVisibility: $columns) {
            Sidebar()
                .navigationSplitViewColumnWidth(min: 220, ideal: 260, max: 320)
        } detail: {
            DestinationStack(destination: store.selection)
                .id(store.selection)
        }
        .navigationSplitViewStyle(.balanced)
    }
}

struct Sidebar: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let selection = Binding<Destination?>(
            get: { store.selection },
            set: { if let d = $0 { store.selection = d } })
        let scanning = Set(store.activities.compactMap(\.sectionId))
        List(selection: selection) {
            Section {
                row(.home)
                row(.search)
                row(.pick)
                if store.hasTv { row(.watchlist) }
                row(.playlists)
                if store.liveTv { row(.livetv) }
                if store.seerr { row(.requests) }
                row(.downloads, badge: activeDownloads)
            }
            if !store.sections.isEmpty {
                Section("Libraries") {
                    ForEach(store.sections, id: \.key) { s in
                        let id = s.key ?? ""
                        HStack {
                            Label(Directory.title(s), systemImage: Destination.icon(s))
                            Spacer()
                            if scanning.contains(id) {
                                ProgressView().controlSize(.mini)
                            }
                        }
                        .tag(Destination.library(id: id))
                    }
                }
            }
            Section {
                if let task = store.activities.first {
                    Button {
                        store.navigate(to: .activity)
                    } label: {
                        VStack(alignment: .leading, spacing: 6) {
                            Text(task.title).font(.caption).foregroundStyle(Theme.secondaryText).lineLimit(1)
                            ProgressView(value: Double(task.progress), total: 100)
                        }
                        .padding(.vertical, 4)
                    }
                    .buttonStyle(.plain)
                }
                row(.activity)
                if store.owned { row(.server) }
                row(.settings)
            }
        }
        .listStyle(.sidebar)
        .navigationTitle("Patina")
        .safeAreaInset(edge: .bottom) {
            AccountMenu()
                .padding(.horizontal, 12)
                .padding(.bottom, 8)
                .background(.bar)
        }
    }

    private var activeDownloads: Int {
        store.downloads.filter { $0.state == .queued || $0.state == .downloading }.count
    }

    private func row(_ d: Destination, badge: Int = 0) -> some View {
        Label(d.title, systemImage: d.systemImage)
            .badge(badge)
            .tag(d)
    }
}

// MARK: Stacks

/// The navigation stack of one destination, with the mini player and the connection banner.
struct DestinationStack: View {
    let destination: Destination
    @Environment(AppStore.self) private var store
    @Namespace private var zoom

    var body: some View {
        NavigationStack(path: store.path(destination)) {
            root
                .navigationDestination(for: Route.self) { route in
                    RouteDestination(route: route)
                }
        }
        .environment(\.zoomNamespace, zoom)
        .safeAreaInset(edge: .top, spacing: 0) {
            if !store.connected { ConnectionBanner() }
        }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            MiniPlayerSlot()
        }
    }

    @ViewBuilder
    private var root: some View {
        if let route = destination.root {
            RouteDestination(route: route, isRoot: true)
        } else if destination == .libraries {
            LibrariesScreen()
        } else {
            MoreScreen()
        }
    }
}

/// iPhone: the Libraries tab.
struct LibrariesScreen: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let scanning = Set(store.activities.compactMap(\.sectionId))
        List {
            ForEach(store.sections, id: \.key) { s in
                NavigationLink(value: Route.library(id: s.key ?? "")) {
                    HStack {
                        Label(Directory.title(s), systemImage: Destination.icon(s))
                        Spacer()
                        if scanning.contains(s.key ?? "") { ProgressView().controlSize(.small) }
                    }
                }
            }
            NavigationLink(value: Route.playlists) {
                Label(Destination.playlists.title, systemImage: Destination.playlists.systemImage)
            }
        }
        .overlay {
            if store.sections.isEmpty {
                StatusView(title: "No libraries", text: "This server has no libraries you can see.", systemImage: "square.stack")
            }
        }
        .navigationTitle("Libraries")
    }
}

/// iPhone: the More tab, with the account and everything that has no tab.
struct MoreScreen: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        List {
            Section {
                AccountMenu()
                    .listRowInsets(EdgeInsets(top: 4, leading: 8, bottom: 4, trailing: 8))
            }
            Section {
                ForEach(Destination.moreEntries, id: \.self) { d in
                    if visible(d), let route = d.root {
                        NavigationLink(value: route) {
                            Label(d.title, systemImage: d.systemImage)
                        }
                    }
                }
            }
        }
        .navigationTitle("More")
    }

    private func visible(_ d: Destination) -> Bool {
        switch d {
        case .watchlist: store.hasTv
        case .livetv: store.liveTv
        case .requests: store.seerr
        case .server: store.owned
        default: true
        }
    }
}

// MARK: Banners

/// Port of the connection notice of Shell.svelte.
struct ConnectionBanner: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "wifi.exclamationmark").foregroundStyle(Theme.danger)
            Text("Connection lost. Reconnecting…")
                .font(.footnote.weight(.medium))
            Spacer(minLength: 8)
            Button("Retry") {
                store.refresh()
                store.perform { try await Commands.reloadSections(store.app) }
            }
            .font(.footnote.weight(.semibold))
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(.ultraThinMaterial)
        .overlay(alignment: .bottom) { Rectangle().fill(Theme.hairline).frame(height: 1) }
        .transition(.move(edge: .top).combined(with: .opacity))
        .accessibilityElement(children: .combine)
    }
}

#Preview("iPad shell") {
    SplitShell()
        .environment(AppStore.preview())
        .preferredColorScheme(.dark)
}

#Preview("iPhone shell") {
    TabShell()
        .environment(AppStore.preview())
        .preferredColorScheme(.dark)
}
