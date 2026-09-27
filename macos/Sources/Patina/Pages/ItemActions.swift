import SwiftUI

/// Play / Resume / Shuffle / Radio, then watched, watchlist, tracks, queue,
/// playlist, download, more, and your rating.
struct ItemActions: View {
    let item: Metadata
    let mediaIndex: Int
    @Environment(AppModel.self) private var app
    @State private var watchlisted: Bool?

    private var discover: String? {
        app.hasTv && ["movie", "show"].contains(item.kind) ? item.discoverId : nil
    }

    var body: some View {
        let kind = item.kind
        let resume = item.progress != nil
        HStack(spacing: 8) {
            if item.canPlay {
                Button { play() } label: {
                    HStack(spacing: 6) {
                        Image(systemName: "play.fill").font(.system(size: 11))
                        Text(resume ? "Resume" : "Play")
                        if let left = item.remaining, resume {
                            Text(left).fontWeight(.regular).opacity(0.6)
                        }
                    }
                }
                .buttonStyle(ItemPrimaryButtonStyle())
                .keyboardShortcut(.return, modifiers: .command)
                .help(resume ? "Resume (⌘↩)" : "Play (⌘↩)")

                if resume {
                    Button("From Start", systemImage: "arrow.counterclockwise") { play(resume: false) }
                }
                if item.isContainer {
                    Button("Shuffle", systemImage: "shuffle") { play(shuffle: true) }
                }
                if kind == "artist" || kind == "album" {
                    Button("Radio", systemImage: "dot.radiowaves.left.and.right") { play(radio: true) }
                }
            }
            Spacer().frame(width: 6)
            Group {
                if item.hasWatchState {
                    let watched = item.isWatched
                    iconButton(watched ? "checkmark.circle.fill" : "checkmark.circle", watched ? "Mark as Unwatched" : "Mark as Watched", on: watched) {
                        Actions.setWatched(item, !watched)
                    }
                }
                if let id = discover {
                    let on = watchlisted ?? false
                    iconButton(on ? "bookmark.fill" : "bookmark", on ? "Remove from Watchlist" : "Add to Watchlist", on: on) {
                        toggleWatchlist(id)
                    }
                    .disabled(watchlisted == nil)
                }
                if item.supportsTracks {
                    iconButton("captions.bubble", "Audio & Subtitles for Many Episodes") { app.sheet = .tracks(item) }
                }
                if item.canPlay && kind != "photo" && kind != "photoalbum" {
                    iconButton("text.line.first.and.arrowtriangle.forward", "Play Next") { Actions.enqueue(item, next: true) }
                }
                if kind != "collection" && kind != "playlist" {
                    iconButton("text.badge.plus", "Add to Playlist…") { app.sheet = .playlist(item) }
                }
                if ["movie", "episode", "track", "show", "season", "album", "artist"].contains(kind) {
                    downloadButton
                }
                Menu {
                    ItemMenu(item: item, onDelete: popPage)
                } label: {
                    Image(systemName: "ellipsis")
                }
                .menuIndicator(.hidden)
                .fixedSize()
                .help("More")
            }
            if ["movie", "show", "episode", "season", "album", "track", "artist"].contains(kind) {
                StarRating(value: item.userRating ?? 0) { v in
                    let rk = item.ratingKey ?? ""
                    app.perform(refresh: true) { try await API.rate(rk, v) }
                }
                .padding(.leading, 6)
            }
        }
        .controlSize(.large)
        .tint(.secondary)
        .foregroundStyle(.primary)
        .task(id: discover) {
            watchlisted = nil
            guard let id = discover else { return }
            watchlisted = try? await API.watchlisted(id)
        }
    }

    private func play(resume: Bool = true, shuffle: Bool = false, radio: Bool = false) {
        var o = PlayOptions()
        o.resume = resume
        o.shuffle = shuffle
        o.radio = radio
        o.mediaIndex = mediaIndex
        Actions.play(item, o)
    }

    private func iconButton(_ symbol: String, _ help: String, on: Bool = false, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol).foregroundStyle(on ? AnyShapeStyle(app.accent) : AnyShapeStyle(.primary))
        }
        .help(help)
        .accessibilityLabel(help)
    }

    private func toggleWatchlist(_ id: String) {
        let on = !(watchlisted ?? false)
        Task {
            if await app.act(on ? "Added to watchlist" : "Removed from watchlist", { try await API.setWatchlisted(id, on) }) {
                watchlisted = on
            }
        }
    }

    @ViewBuilder private var downloadButton: some View {
        let saved = Actions.downloaded(item)
        switch saved?.state {
        case .done:
            iconButton("arrow.down.circle.fill", "Downloaded — Show Downloads", on: true) { app.selection = .downloads }
        case .queued, .downloading:
            let pct = saved.map { $0.size > 0 ? "\(Int(Double($0.received) / Double($0.size) * 100))%" : "…" } ?? "…"
            Button {} label: { Text(pct).font(.system(size: 11, weight: .semibold).monospacedDigit()) }
                .disabled(true)
                .help(saved?.state == .queued ? "Waiting to Download" : "Downloading")
        case .paused, .failed:
            let failed = saved?.state == .failed
            iconButton("exclamationmark.arrow.circlepath", failed ? "Download Failed\(saved?.error.map { ": \($0)" } ?? "") — Retry" : "Download Paused — Resume") {
                if let id = saved?.id { app.perform { try await API.retryDownload(id) } }
            }
        case nil:
            iconButton("arrow.down.circle", "Download") { Actions.download(item) }
        }
    }
}

/// Your rating in half stars; Plex stores 0–10. Clicking the current value clears it.
struct StarRating: View {
    let value: Double
    let onRate: (Double?) -> Void
    @Environment(AppModel.self) private var app
    @State private var hover: Int?

    var body: some View {
        let current = Int(value.rounded())
        let shown = hover ?? current
        HStack(spacing: 0) {
            ForEach(1...5, id: \.self) { n in
                Image(systemName: shown >= n * 2 ? "star.fill" : shown == n * 2 - 1 ? "star.leadinghalf.filled" : "star")
                    .font(.system(size: 14))
                    .foregroundStyle(shown >= n * 2 - 1 ? AnyShapeStyle(app.accent) : AnyShapeStyle(.tertiary))
                    .frame(width: 20, height: 28)
                    .contentShape(Rectangle())
                    .onContinuousHover { phase in
                        if case .active(let p) = phase { hover = p.x < 10 ? n * 2 - 1 : n * 2 }
                    }
                    .onTapGesture { location in
                        let v = location.x < 10 ? n * 2 - 1 : n * 2
                        onRate(v == current ? nil : Double(v))
                    }
            }
        }
        .onHover { if !$0 { hover = nil } }
        .help(current > 0 ? "Your rating: \(Self.stars(current)) of 5" : "Rate")
        .accessibilityElement()
        .accessibilityLabel("Your rating")
        .accessibilityValue("\(Self.stars(current)) of 5 stars")
        .accessibilityAdjustableAction { dir in
            let step = dir == .increment ? 1 : -1
            onRate(Double(max(0, min(10, current + step))))
        }
    }

    static func stars(_ v: Int) -> String { String(format: "%g", Double(v) / 2) }
}
