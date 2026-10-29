// Port of web/src/pages/Home.svelte (and components/ResumeQueue.svelte): the featured
// banner, the server's home hubs in the order and visibility of Customize Home (saved per
// account and server in `ui.home_layouts`), and the offer to resume the last queue.

import PatinaCore
import PlexKit
import SwiftUI

/// Hubs hidden until turned on: Plex's legacy On Deck, which Continue Watching replaces.
private let hiddenByDefault = ["home.ondeck"]

func hubKey(_ h: Hub) -> String {
    h.hubIdentifier ?? h.key ?? h.title ?? ""
}

extension AppStore {
    /// The Home layout of the current account and server.
    var homeLayout: HomeLayout {
        settings?.ui.homeLayouts[scope] ?? HomeLayout()
    }

    func isHidden(_ key: String, _ layout: HomeLayout) -> Bool {
        layout.hidden.contains(key) || (hiddenByDefault.contains(key) && !layout.shown.contains(key))
    }

    func saveHomeLayout(_ layout: HomeLayout) {
        var all = settings?.ui.homeLayouts ?? [:]
        all[scope] = layout
        updateUi(["home_layouts": .object(all.mapValues(\.json))])
    }

    /// The hubs with something in them, in the saved order.
    func orderedHubs(_ hubs: [Hub]) -> [Hub] {
        let order = homeLayout.order
        return hubs.filter { !Hub.isEmpty($0) }.enumerated().sorted { a, b in
            let ai = order.firstIndex(of: hubKey(a.element)) ?? (100_000 + a.offset)
            let bi = order.firstIndex(of: hubKey(b.element)) ?? (100_000 + b.offset)
            return ai < bi
        }.map(\.element)
    }
}

struct HomeScreen: View {
    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var hubs = Loader<[Hub]>()

    var body: some View {
        let layout = store.homeLayout
        let all = store.orderedHubs(hubs.value ?? [])
        let rows = all.filter { !store.isHidden(hubKey($0), layout) }
        let picks = featured(rows)
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 30) {
                if !picks.isEmpty {
                    FeaturedBanner(picks: picks)
                } else {
                    Color.clear.frame(height: 1)
                }
                ResumeQueueBanner()
                if let error = hubs.error, hubs.value != nil {
                    StaleBanner(error: error) { hubs.reload() }.padding(.horizontal, Theme.margin(sizeClass))
                }
                if hubs.value == nil {
                    if let error = hubs.error {
                        StatusView(error: error) { hubs.reload() }
                    } else {
                        ForEach(0..<3, id: \.self) { i in
                            SkeletonRow(shape: i == 0 ? .landscape : .poster, width: cardWidth(i == 0 ? .landscape : .poster, sizeClass))
                                .padding(.leading, Theme.margin(sizeClass))
                        }
                    }
                } else if rows.isEmpty {
                    StatusView(
                        title: "Nothing here yet",
                        text: all.isEmpty
                            ? "Your libraries are empty or still scanning."
                            : "Your Home rows are hidden. Use Customize Home to show them.",
                        systemImage: "house")
                } else {
                    ForEach(Array(rows.enumerated()), id: \.offset) { _, hub in
                        HubRow(hub: hub, compact: layout.compactContinue)
                    }
                }
            }
            .padding(.bottom, 30)
        }
        .ignoresSafeArea(edges: picks.isEmpty ? [] : .top)
        .scrollIndicators(.hidden)
        .background(Theme.background)
        .navigationTitle("Home")
        .toolbarTitleDisplayMode(picks.isEmpty ? .large : .inline)
        .toolbarBackground(.hidden, for: .navigationBar)
        .toolbar {
            ToolbarItem(placement: .principal) { Text("") }
            ToolbarItemGroup(placement: .topBarTrailing) {
                Button("Pick Something", systemImage: "dice") { store.navigate(to: .pick) }
                Button("Customize Home", systemImage: "slider.horizontal.3") { store.sheet = .customizeHome }
            }
        }
        .refreshable { await hubs.refresh() }
        .task(id: store.revision) {
            await hubs.load(at: store.revision) { try await Commands.homeHubs(store.app) }
        }
    }

    /// Featured items: recent or in-progress movies and shows with backdrop art.
    private func featured(_ list: [Hub]) -> [(Metadata, String)] {
        var out: [(Metadata, String)] = []
        for hub in list {
            let id = (hub.hubIdentifier ?? "").lowercased()
            let label = id.contains("continue") || id.contains("ondeck") ? "Continue Watching" : id.contains("recent") ? "Recently Added" : nil
            guard let label else { continue }
            for m in hub.metadata {
                let k = Metadata.kind(m)
                if out.count < 6, m.art != nil, k == "movie" || k == "show", !out.contains(where: { $0.0.ratingKey == m.ratingKey }) {
                    out.append((m, label))
                }
            }
        }
        return out
    }
}

/// The full-bleed banner of featured items, swiped or stepped through.
struct FeaturedBanner: View {
    let picks: [(Metadata, String)]

    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var index: Int? = 0

    var body: some View {
        let regular = sizeClass == .regular
        GeometryReader { proxy in
            let height = proxy.size.height
            ScrollView(.horizontal) {
                LazyHStack(spacing: 0) {
                    ForEach(Array(picks.enumerated()), id: \.offset) { i, pick in
                        page(pick.0, label: pick.1, height: height, regular: regular)
                            .frame(width: proxy.size.width, height: height)
                            .id(i)
                    }
                }
                .scrollTargetLayout()
            }
            .scrollTargetBehavior(.paging)
            .scrollIndicators(.hidden)
            .scrollPosition(id: $index)
            .overlay(alignment: .bottom) {
                // Below the buttons, so the two never overlap on a narrow screen.
                if picks.count > 1 { pager.padding(.bottom, 8) }
            }
        }
        // Full width under the side safe areas too, so the next page never peeks in.
        .ignoresSafeArea(edges: .horizontal)
        .frame(height: regular ? 560 : 520)
        .sensoryFeedback(.selection, trigger: index)
    }

    private func page(_ item: Metadata, label: String, height: CGFloat, regular: Bool) -> some View {
        let progress = Metadata.progress(item)
        let meta = [
            item.year.map(String.init),
            item.contentRating,
            Metadata.kind(item) == "show"
                ? item.childCount.map { Format.plural($0, "season") }
                : item.duration.map { Format.duration(ms: Int64($0)) },
            item.genre.prefix(2).map(Tag.name).joined(separator: ", "),
        ].compactMap { $0 }.filter { !$0.isEmpty }
        return ZStack(alignment: .bottomLeading) {
            Artwork(item.art, width: 1920, height: 1080, server: Metadata.serverId(item))
                .overlay {
                    LinearGradient(
                        stops: [
                            .init(color: Theme.background.opacity(0.35), location: 0),
                            .init(color: .clear, location: 0.25),
                            .init(color: Theme.background.opacity(0.55), location: 0.6),
                            .init(color: Theme.background, location: 1),
                        ], startPoint: .top, endPoint: .bottom)
                }
                .overlay {
                    if regular {
                        LinearGradient(colors: [Theme.background.opacity(0.8), .clear], startPoint: .leading, endPoint: .center)
                    }
                }
            VStack(alignment: .leading, spacing: 10) {
                Text(label.uppercased())
                    .font(.caption.weight(.bold))
                    .kerning(1.2)
                    .foregroundStyle(store.accent)
                Text(Metadata.title(item))
                    .font(.system(size: regular ? 52 : 36, weight: .bold))
                    .lineLimit(2)
                    .minimumScaleFactor(0.7)
                    .shadow(color: .black.opacity(0.4), radius: 12)
                Text(meta.joined(separator: "  ·  "))
                    .font(.subheadline)
                    .foregroundStyle(Theme.secondaryText)
                if let summary = item.summary, regular {
                    Text(summary).font(.callout).foregroundStyle(Theme.secondaryText).lineLimit(3)
                }
                HStack(spacing: 10) {
                    Button {
                        store.play(item)
                    } label: {
                        Label(progress != nil ? "Resume" : "Play", systemImage: "play.fill")
                            .font(.headline)
                            .padding(.horizontal, 10)
                            .frame(minHeight: 34)
                    }
                    .buttonStyle(.borderedProminent)
                    .foregroundStyle(.black)
                    NavigationLink(value: PlexUI.route(item)) {
                        Label("Details", systemImage: "info.circle")
                            .font(.headline)
                            .padding(.horizontal, 6)
                            .frame(minHeight: 34)
                    }
                    .buttonStyle(.bordered)
                    .tint(.white)
                }
                .padding(.top, 4)
            }
            .frame(maxWidth: regular ? 560 : .infinity, alignment: .leading)
            .padding(.horizontal, Theme.margin(sizeClass))
            .padding(.bottom, 48)
        }
        .contentShape(.rect)
        .contextMenu { ItemMenu(item: item) }
    }

    private var pager: some View {
        HStack(spacing: 6) {
            ForEach(0..<picks.count, id: \.self) { i in
                Capsule()
                    .fill(i == (index ?? 0) ? Color.white : .white.opacity(0.35))
                    .frame(width: i == (index ?? 0) ? 18 : 6, height: 6)
                    .onTapGesture { withAnimation(.spring) { index = i } }
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(.ultraThinMaterial, in: .capsule)
        .animation(.spring(duration: 0.3), value: index)
        .accessibilityElement()
        .accessibilityLabel("Featured \((index ?? 0) + 1) of \(picks.count)")
        .accessibilityAdjustableAction { direction in
            let i = index ?? 0
            withAnimation { index = direction == .increment ? min(picks.count - 1, i + 1) : max(0, i - 1) }
        }
    }
}

/// Port of web/src/components/ResumeQueue.svelte: offers the queue that was playing when
/// Patina last stopped. Nothing plays until Resume is pressed.
struct ResumeQueueBanner: View {
    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        if let saved = store.savedQueue, store.nowPlaying == nil {
            let (title, sub) = PlexUI.playingLines(saved.current)
            HStack(spacing: 12) {
                Image(systemName: "play.circle.fill").font(.title2).foregroundStyle(store.accent)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Resume \(title)").font(.subheadline.weight(.semibold)).lineLimit(1)
                    Text(
                        [
                            sub.isEmpty ? nil : sub,
                            saved.positionMs > 0 ? "at \(Format.clock(ms: Int64(saved.positionMs)))" : nil,
                            saved.count > 1 ? "\(saved.index + 1) of \(saved.count) in the queue" : nil,
                        ].compactMap { $0 }.joined(separator: " · ")
                    )
                    .font(.caption)
                    .foregroundStyle(Theme.secondaryText)
                    .lineLimit(1)
                }
                Spacer()
                Button("Resume") {
                    store.perform {
                        try await PendingCommands.resumeSaved(store.app)
                        store.savedQueue = nil
                    }
                }
                .buttonStyle(.borderedProminent)
                .foregroundStyle(.black)
                Menu {
                    Button("Not Now") { store.savedQueue = nil }
                    Button("Forget", role: .destructive) {
                        store.savedQueue = nil
                        store.perform { try await PendingCommands.forgetSaved(store.app) }
                    }
                } label: {
                    Image(systemName: "xmark").frame(width: 32, height: 32)
                }
            }
            .padding(12)
            .panel(cornerRadius: 16)
            .padding(.horizontal, Theme.margin(sizeClass))
        }
    }
}

/// Customize Home: show, hide and reorder the rows; compact Continue Watching.
struct CustomizeHomeSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var hubs: [Hub]?
    @State private var error: String?

    var body: some View {
        let layout = store.homeLayout
        let all = store.orderedHubs(hubs ?? [])
        NavigationStack {
            List {
                Section {
                    Toggle(
                        "Compact Continue Watching",
                        isOn: Binding(
                            get: { layout.compactContinue },
                            set: { v in
                                var l = layout
                                l.compactContinue = v
                                store.saveHomeLayout(l)
                            }))
                }
                Section {
                    if let error {
                        Text(error).foregroundStyle(Theme.danger)
                    } else if hubs == nil {
                        ProgressView().frame(maxWidth: .infinity)
                    }
                    ForEach(all, id: \.self) { hub in
                        let key = hubKey(hub)
                        Toggle(hub.title ?? key, isOn: Binding(get: { !store.isHidden(key, layout) }, set: { setShown(key, $0) }))
                    }
                    .onMove { from, to in
                        var order = all.map(hubKey)
                        order.move(fromOffsets: from, toOffset: to)
                        var l = layout
                        l.order = order
                        store.saveHomeLayout(l)
                    }
                } header: {
                    Text("Rows")
                } footer: {
                    Text("Drag to reorder. Saved for this account and server.")
                }
                Section {
                    Button("Reset Layout", role: .destructive) { store.saveHomeLayout(HomeLayout()) }
                }
            }
            .environment(\.editMode, .constant(.active))
            .navigationTitle("Customize Home")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } }
            }
            .task {
                do {
                    hubs = try await Commands.homeHubs(store.app)
                } catch {
                    self.error = errorText(error)
                }
            }
        }
    }

    private func setShown(_ key: String, _ on: Bool) {
        var l = store.homeLayout
        l.hidden.removeAll { $0 == key }
        l.shown.removeAll { $0 == key }
        if hiddenByDefault.contains(key) {
            if on { l.shown.append(key) }
        } else if !on {
            l.hidden.append(key)
        }
        Haptics.selection()
        store.saveHomeLayout(l)
    }
}

#Preview("Featured") {
    NavigationStack {
        FeaturedBanner(picks: [(Fixtures.movie(), "Recently Added"), (Fixtures.movie("2", "Dune"), "Continue Watching")])
    }
    .environment(AppStore.preview())
    .preferredColorScheme(.dark)
}
