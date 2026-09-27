import SwiftUI

/// Detail page for any library item: movie, show, season, episode, artist,
/// album, track, collection (web/src/pages/Item.svelte).
struct ItemPage: View {
    let ratingKey: String
    @Environment(AppModel.self) private var app

    var body: some View {
        ScrollView {
            Loading(id: [AnyHashable(ratingKey), AnyHashable(app.revision)], { try await API.metadata(ratingKey) }) { item in
                if let item {
                    ItemDetail(item: item)
                } else {
                    EmptyState(title: "Not found", message: "This item no longer exists on the server.")
                }
            }
        }
    }
}

/// Leave the current page.
@MainActor func popPage() {
    let app = AppModel.shared
    let place = app.selection ?? .home
    if app.paths[place]?.isEmpty == false { app.paths[place]?.removeLast() }
}

struct ItemDetail: View {
    let item: Metadata
    @Environment(AppModel.self) private var app
    @State private var mediaIndex = 0
    @State private var hubs: [Hub] = []

    var body: some View {
        let kind = item.kind
        VStack(alignment: .leading, spacing: 0) {
            ItemHeader(item: item, mediaIndex: $mediaIndex)
                .padding(.top, item.backdropPath == nil ? 36 : 150)
                .padding(.horizontal, Theme.gutter)
                .frame(maxWidth: .infinity, alignment: .leading)
                // The art sits behind the header without setting its height.
                .background(alignment: .top) { ItemBackdrop(path: item.backdropPath) }

            ItemBody(item: item, mediaIndex: $mediaIndex)
                .padding(.horizontal, Theme.gutter)
                .padding(.top, 28)

            VStack(alignment: .leading, spacing: 34) {
                if kind == "collection" && app.owned {
                    CollectionTools(item: item).padding(.horizontal, Theme.gutter)
                }
                if kind == "show", let next = item.onDeck?.metadata.first {
                    OnDeckRow(episode: next).padding(.horizontal, Theme.gutter)
                }
                if ["show", "season", "artist", "album", "collection", "photoalbum"].contains(kind) {
                    ItemChildren(item: item)
                }
                if !item.roles.isEmpty {
                    ItemCast(item: item)
                }
                if let extras = item.extras?.metadata, !extras.isEmpty {
                    HubRow(title: "Extras", items: extras)
                }
                ForEach(Array(relatedHubs.enumerated()), id: \.offset) { _, hub in
                    HubRow(hub: hub)
                }
            }
            .padding(.top, 34)
            .padding(.bottom, 40)
        }
        .navigationTitle(item.name)
        .task(id: [AnyHashable(item.ratingKey), AnyHashable(app.revision)]) {
            hubs = (try? await API.itemHubs(item.ratingKey ?? "", kind: kind)) ?? []
        }
        .onChange(of: item.ratingKey) { _, _ in mediaIndex = 0 }
    }

    /// Related hubs from the item, then the item-page hubs not already among them.
    private var relatedHubs: [Hub] {
        let related = (item.related?.hubs ?? []).filter { !$0.metadata.isEmpty }
        let extra = hubs.filter { h in
            !h.metadata.isEmpty && !related.contains { r in
                (h.hubIdentifier != nil && r.hubIdentifier == h.hubIdentifier)
                    || (h.title != nil && r.title?.lowercased() == h.title?.lowercased())
            }
        }
        return related + extra
    }
}

/// The item's background art, faded into the window background.
struct ItemBackdrop: View {
    let path: String?

    var body: some View {
        if let path {
            Artwork(path, width: 1920, height: 1080)
                .frame(height: 440)
                .frame(maxWidth: .infinity)
                .opacity(0.45)
                .mask(LinearGradient(colors: [.black, .black.opacity(0.6), .clear], startPoint: .top, endPoint: .bottom))
                .allowsHitTesting(false)
                .accessibilityHidden(true)
        }
    }
}

/// The show's next episode.
struct OnDeckRow: View {
    let episode: Metadata
    @Environment(AppModel.self) private var app

    var body: some View {
        let spoiler = app.spoilers && !episode.isWatched
        VStack(alignment: .leading, spacing: 10) {
            Text("Up Next").font(Theme.sectionTitle)
            HStack(alignment: .center, spacing: 16) {
                Color.clear.frame(width: 200, height: 112)
                    .overlay { Artwork(episode.thumb, width: 480, height: 270).blur(radius: spoiler ? 14 : 0) }
                    .overlay(alignment: .bottom) { if let p = episode.progress { ProgressLine(value: p) } }
                    .clipShape(RoundedRectangle(cornerRadius: Theme.radius))
                VStack(alignment: .leading, spacing: 4) {
                    Text(episode.episodeCode).font(Theme.caption).foregroundStyle(.secondary)
                    Text(episode.name).font(.system(size: 14, weight: .medium))
                    if let left = episode.remaining ?? episode.duration.map(Fmt.duration) {
                        Text(left).font(Theme.caption).foregroundStyle(.secondary)
                    }
                    Button(episode.progress != nil ? "Resume" : "Play", systemImage: "play.fill") { Actions.play(episode) }
                        .padding(.top, 6)
                }
                Spacer()
            }
            .contentShape(Rectangle())
            .onTapGesture { app.open(episode) }
            .contextMenu { ItemMenu(item: episode) }
        }
    }
}
