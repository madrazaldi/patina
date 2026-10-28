// Port of web/src/components/ItemMenu.svelte: the actions on an item, as the contents of a
// `.contextMenu` or a "•••" `Menu`.

import PatinaCore
import PlexKit
import SwiftUI

struct ItemMenu: View {
    let item: Metadata
    @Environment(AppStore.self) private var store

    private static let library = ["movie", "show", "season", "episode", "artist", "album", "track", "collection"]

    var body: some View {
        let kind = Metadata.kind(item)
        let resume = Metadata.progress(item) != nil
        let watched = Metadata.isWatched(item)
        let rk = Metadata.ratingKey(item)
        let foreign = Metadata.serverId(item)
        let queueable = PlexUI.canPlay(item) && !["photo", "photoalbum"].contains(kind)
        let inLibrary = Self.library.contains(kind)

        if PlexUI.canPlay(item) {
            Section {
                Button(resume ? "Resume" : "Play", systemImage: "play.fill") { store.play(item) }
                if resume {
                    Button("Play from Beginning", systemImage: "arrow.counterclockwise") {
                        store.play(item, .init(resume: false))
                    }
                }
                if Metadata.isContainer(item) {
                    Button("Shuffle", systemImage: "shuffle") { store.play(item, .init(shuffle: true)) }
                }
                if queueable {
                    Button("Play Next", systemImage: "text.line.first.and.arrowtriangle.forward") {
                        store.enqueue(item, next: true)
                    }
                    Button("Add to Queue", systemImage: "text.line.last.and.arrowtriangle.forward") {
                        store.enqueue(item, next: false)
                    }
                }
                if ["artist", "album", "track"].contains(kind) {
                    Button("Start Radio", systemImage: "dot.radiowaves.left.and.right") {
                        store.play(item, .init(radio: true))
                    }
                }
            }
        }

        Section {
            if kind == "episode" || kind == "track", let show = item.grandparentRatingKey {
                Button(kind == "track" ? "Go to Artist" : "Go to Show", systemImage: kind == "track" ? "music.mic" : "tv") {
                    store.navigate(to: .item(ratingKey: show, server: foreign))
                }
            }
            if ["episode", "track", "season"].contains(kind), let parent = item.parentRatingKey {
                Button(
                    kind == "track" ? "Go to Album" : kind == "season" ? "Go to Show" : "Go to Season",
                    systemImage: "square.stack"
                ) {
                    store.navigate(to: .item(ratingKey: parent, server: foreign))
                }
            }
            if PlexUI.hasWatchState(item) {
                Button(
                    watched ? "Mark as Unwatched" : "Mark as Watched",
                    systemImage: watched ? "circle" : "checkmark.circle"
                ) {
                    store.setWatched(item, !watched)
                }
            }
        }

        // What follows edits the connected server, so it is not offered for an item found on another one.
        if foreign == nil {
            Section {
                if resume {
                    Button("Clear Progress", systemImage: "eraser") {
                        store.perform { try await Commands.clearProgress(store.app, ratingKey: rk) }
                    }
                }
                if kind == "episode" || (kind == "movie" && resume) {
                    Button("Remove from Continue Watching", systemImage: "eye.slash") {
                        store.perform { try await Commands.removeFromContinueWatching(store.app, ratingKey: rk) }
                    }
                }
                if ["movie", "show", "episode", "season", "album", "track", "artist"].contains(kind) {
                    Menu("Rate", systemImage: "star") {
                        RatingMenu(item: item)
                    }
                }
                if kind != "playlist" && kind != "collection" {
                    Button("Add to Playlist…", systemImage: "text.badge.plus") { store.sheet = .playlist(item, items: []) }
                }
                if store.owned && ["movie", "show", "artist", "album"].contains(kind) {
                    Button("Add to Collection…", systemImage: "rectangle.stack.badge.plus") {
                        store.sheet = .collection(item, items: [])
                    }
                }
                if store.hasTv, ["movie", "show"].contains(kind), let discover = Metadata.discoverId(item) {
                    Button("Add to Watchlist", systemImage: "bookmark") {
                        store.perform("Added to watchlist") {
                            try await PendingCommands.setWatchlisted(store.app, discoverId: discover, on: true)
                        }
                    }
                }
                if ["movie", "episode", "track", "show", "season", "album", "artist"].contains(kind) {
                    downloadButton
                }
            }

            if store.owned && kind != "playlist" {
                Section {
                    if inLibrary {
                        Button("Edit…", systemImage: "pencil") { store.sheet = .edit(item) }
                        if kind != "track" {
                            Button("Choose Poster…", systemImage: "photo") { store.sheet = .artwork(item, kind: .poster) }
                        }
                        if ["movie", "show", "artist", "album", "collection"].contains(kind) {
                            Button("Choose Background…", systemImage: "photo.on.rectangle") {
                                store.sheet = .artwork(item, kind: .art)
                            }
                        }
                        if ["movie", "show", "artist", "album"].contains(kind) {
                            Button("Fix Match…", systemImage: "scope") { store.sheet = .match(item) }
                        }
                    }
                    Button("Refresh Metadata", systemImage: "arrow.clockwise") {
                        store.perform("Refreshing metadata…") { try await Commands.refreshMetadata(store.app, ratingKey: rk) }
                    }
                    Button("Analyze", systemImage: "waveform.badge.magnifyingglass") {
                        store.perform("Analyzing…") { try await Commands.analyzeMetadata(store.app, ratingKey: rk) }
                    }
                    if inLibrary {
                        Button("Delete…", systemImage: "trash", role: .destructive) {
                            Task { await store.delete(item) }
                        }
                    }
                }
            }

            if let url = store.plexWebUrl(item) {
                Section {
                    Button("Copy Link", systemImage: "link") {
                        UIPasteboard.general.url = url
                        store.toast("Link copied")
                    }
                    Button("Open in Plex Web", systemImage: "safari") {
                        store.perform { try await store.host.openUrl(url.absoluteString) }
                    }
                }
            }
        }
    }

    @ViewBuilder
    private var downloadButton: some View {
        if let saved = store.downloaded(item) {
            switch saved.state {
            case .done:
                Button("Downloaded", systemImage: "arrow.down.circle.fill") {}.disabled(true)
            case .paused, .failed:
                Button("Resume Download", systemImage: "arrow.down.circle") {
                    store.perform { try await PendingCommands.retryDownload(store.app, id: saved.id) }
                }
            case .queued, .downloading:
                Button("Downloading…", systemImage: "arrow.down.circle.dotted") {}.disabled(true)
            }
        } else {
            Button("Download", systemImage: "arrow.down.circle") { store.download(item) }
        }
    }
}

/// Your rating, 1–5 stars, as menu entries (Plex stores 0–10).
struct RatingMenu: View {
    let item: Metadata
    @Environment(AppStore.self) private var store

    var body: some View {
        let stars = Int(((item.userRating ?? 0) / 2).rounded())
        ForEach((1...5).reversed(), id: \.self) { n in
            Button {
                rate(n == stars ? nil : Double(n * 2))
            } label: {
                if n == stars {
                    Label(String(repeating: "★", count: n), systemImage: "checkmark")
                } else {
                    Text(String(repeating: "★", count: n))
                }
            }
        }
        if stars > 0 {
            Button("Clear Rating", systemImage: "star.slash") { rate(nil) }
        }
    }

    private func rate(_ value: Double?) {
        Haptics.selection()
        store.perform(refresh: true) { try await Commands.rate(store.app, ratingKey: Metadata.ratingKey(item), rating: value) }
    }
}

/// The "•••" button that opens the item menu.
struct ItemMenuButton: View {
    let item: Metadata
    var prominent = false

    var body: some View {
        Menu {
            ItemMenu(item: item)
        } label: {
            Image(systemName: "ellipsis")
                .font(.body.weight(.semibold))
                .frame(width: 44, height: 44)
                .contentShape(.rect)
        }
        .accessibilityLabel("More")
    }
}
