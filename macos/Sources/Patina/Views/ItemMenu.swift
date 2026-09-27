import SwiftUI

/// Every action for an item, used as its context menu and "…" menu
/// (web/src/components/ItemMenu.svelte).
struct ItemMenu: View {
    let item: Metadata
    /// Runs after the item is deleted, e.g. to leave its page.
    var onDelete: (() -> Void)? = nil

    @Environment(AppModel.self) private var app

    private static let library = ["movie", "show", "season", "episode", "artist", "album", "track", "collection"]

    var body: some View {
        let kind = item.kind
        let rk = item.ratingKey ?? ""
        let resume = item.progress != nil
        let queueable = item.canPlay && !["photo", "photoalbum"].contains(kind)
        let inLibrary = Self.library.contains(kind)

        if item.canPlay {
            Button(resume ? "Resume" : "Play", systemImage: "play") { Actions.play(item) }
            if resume {
                Button("Play from Beginning", systemImage: "arrow.counterclockwise") {
                    var o = PlayOptions(); o.resume = false
                    Actions.play(item, o)
                }
            }
            if item.isContainer {
                Button("Shuffle", systemImage: "shuffle") {
                    var o = PlayOptions(); o.shuffle = true
                    Actions.play(item, o)
                }
            }
            if queueable {
                Button("Play Next", systemImage: "text.line.first.and.arrowtriangle.forward") { Actions.enqueue(item, next: true) }
                Button("Add to Queue", systemImage: "text.line.last.and.arrowtriangle.forward") { Actions.enqueue(item, next: false) }
            }
            if ["artist", "album", "track"].contains(kind) {
                Button("Start Radio", systemImage: "dot.radiowaves.left.and.right") {
                    var o = PlayOptions(); o.radio = true
                    Actions.play(item, o)
                }
            }
            Divider()
        }

        if kind == "episode" || kind == "track", let show = item.grandparentRatingKey {
            Button(kind == "track" ? "Go to Artist" : "Go to Show", systemImage: kind == "track" ? "music.mic" : "tv") {
                app.open(.item(show))
            }
        }
        if ["episode", "track", "season"].contains(kind), let parent = item.parentRatingKey {
            Button(kind == "track" ? "Go to Album" : kind == "season" ? "Go to Show" : "Go to Season", systemImage: "square.stack") {
                app.open(.item(parent))
            }
        }

        if item.hasWatchState {
            let watched = item.isWatched
            Button(watched ? "Mark as Unwatched" : "Mark as Watched", systemImage: watched ? "circle" : "checkmark.circle") {
                Actions.setWatched(item, !watched)
            }
        }
        if resume {
            Button("Clear Progress", systemImage: "eraser") {
                app.perform("Progress cleared", refresh: true) { try await API.clearProgress(rk) }
            }
        }
        if kind == "episode" || (kind == "movie" && resume) {
            Button("Remove from Continue Watching", systemImage: "eye.slash") {
                app.perform("Removed from Continue Watching", refresh: true) { try await API.removeFromContinueWatching(rk) }
            }
        }
        if kind != "playlist" && kind != "collection" {
            Button("Add to Playlist…", systemImage: "text.badge.plus") { app.sheet = .playlist(item) }
        }
        if app.owned && ["movie", "show", "artist", "album"].contains(kind) {
            Button("Add to Collection…", systemImage: "folder.badge.plus") { app.sheet = .collection(item) }
        }
        if item.supportsTracks {
            Button("Audio & Subtitles…", systemImage: "captions.bubble") { app.sheet = .tracks(item) }
        }
        if app.hasTv, let discover = item.discoverId {
            Button("Add to Watchlist", systemImage: "bookmark") {
                app.perform("Added to watchlist") { try await API.setWatchlisted(discover, true) }
            }
        }
        if ["movie", "episode", "track", "show", "season", "album", "artist"].contains(kind) {
            let saved = Actions.downloaded(item)
            switch saved?.state {
            case .done: Button("Downloaded", systemImage: "arrow.down.circle") {}.disabled(true)
            case .paused, .failed:
                if let id = saved?.id {
                    Button("Resume Download", systemImage: "arrow.down.circle") {
                        app.perform { try await API.retryDownload(id) }
                    }
                }
            case .queued, .downloading: Button("Downloading…", systemImage: "arrow.down.circle") {}.disabled(true)
            case nil: Button("Download", systemImage: "arrow.down.circle") { Actions.download(item) }
            }
        }

        if app.owned && kind != "playlist" {
            Divider()
            if inLibrary {
                Button("Edit…", systemImage: "pencil") { app.sheet = .edit(item) }
                if kind != "track" {
                    Button("Choose Poster…", systemImage: "photo") { app.sheet = .artwork(item, kind: "poster") }
                }
                if ["movie", "show", "artist", "album", "collection"].contains(kind) {
                    Button("Choose Background…", systemImage: "photo.artframe") { app.sheet = .artwork(item, kind: "art") }
                }
                if ["movie", "show", "artist", "album"].contains(kind) {
                    Button("Fix Match…", systemImage: "scope") { app.sheet = .match(item) }
                }
            }
            Button("Refresh Metadata", systemImage: "arrow.clockwise") {
                app.perform("Refreshing metadata…") { try await API.refreshMetadata(rk) }
            }
            Button("Analyze", systemImage: "waveform.badge.magnifyingglass") {
                app.perform("Analyzing…") { try await API.analyzeMetadata(rk) }
            }
            if inLibrary {
                Button("Delete…", systemImage: "trash", role: .destructive) { Actions.delete(item, then: onDelete) }
            }
        }
        if app.session?.machineIdentifier != nil {
            Divider()
            Button("Open in Plex Web", systemImage: "arrow.up.forward.app") { Actions.openInPlexWeb(item) }
        }
    }
}
