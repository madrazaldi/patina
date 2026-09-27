import SwiftUI

/// Seasons, episodes, albums, tracks or collection items below a detail
/// header (Children.svelte).
struct ItemChildren: View {
    let item: Metadata
    @Environment(AppModel.self) private var app

    var body: some View {
        let kind = item.kind
        Loading(id: [AnyHashable(item.ratingKey), AnyHashable(app.revision)], {
            try await API.children(item.ratingKey ?? "", collection: kind == "collection")
        }) { list in
            if !list.isEmpty {
                switch kind {
                case "show": HubRow(title: "Seasons", items: list)
                case "season": EpisodeList(episodes: list).padding(.horizontal, Theme.gutter)
                case "album": TrackList(tracks: list).padding(.horizontal, Theme.gutter)
                case "collection": CollectionItems(collection: item, items: list)
                default:
                    VStack(alignment: .leading, spacing: 12) {
                        Text(kind == "artist" ? "Albums" : Fmt.plural(list.count, "item"))
                            .font(Theme.sectionTitle)
                            .padding(.horizontal, Theme.gutter)
                        CardGrid(items: list)
                    }
                }
            }
        }
    }
}

/// A season's episodes: number, thumbnail with progress, title, air date and summary.
struct EpisodeList: View {
    let episodes: [Metadata]

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Episodes").font(Theme.sectionTitle).padding(.bottom, 6)
            ForEach(Array(episodes.enumerated()), id: \.offset) { i, ep in
                if i > 0 { Hairline() }
                EpisodeRow(episode: ep)
            }
        }
    }
}

struct EpisodeRow: View {
    let episode: Metadata
    @Environment(AppModel.self) private var app
    @State private var hovering = false

    var body: some View {
        let watched = episode.isWatched
        let spoiler = app.spoilers && !watched
        HStack(alignment: .top, spacing: 18) {
            Text(episode.index.map(String.init) ?? "")
                .font(.system(size: 13).monospacedDigit())
                .foregroundStyle(.tertiary)
                .frame(width: 24, alignment: .trailing)
                .padding(.top, 2)
            Color.clear.frame(width: 208, height: 117)
                .overlay { Artwork(episode.thumb, width: 480, height: 270).blur(radius: spoiler ? 14 : 0) }
                .overlay(alignment: .bottom) { if let p = episode.progress { ProgressLine(value: p) } }
                .overlay(alignment: .bottomLeading) {
                    if hovering {
                        Button { Actions.play(episode) } label: {
                            Image(systemName: "play.fill")
                                .font(.system(size: 12))
                                .foregroundStyle(.black)
                                .frame(width: 30, height: 30)
                                .background(Circle().fill(.white.opacity(0.92)))
                        }
                        .buttonStyle(.plain)
                        .help(episode.progress != nil ? "Resume" : "Play")
                        .padding(8)
                    }
                }
                .clipShape(RoundedRectangle(cornerRadius: Theme.radius))
                .overlay(RoundedRectangle(cornerRadius: Theme.radius).strokeBorder(Theme.hairline))
            VStack(alignment: .leading, spacing: 4) {
                Text(episode.name)
                    .font(.system(size: 14, weight: .medium))
                    .underline(hovering, color: .secondary)
                HStack(spacing: 0) {
                    Text([episode.originallyAvailableAt.map(Fmt.airDate), episode.remaining ?? episode.duration.map(Fmt.duration)]
                        .compactMap { $0 }.joined(separator: " · "))
                    if watched {
                        Text(" · ")
                        Label("Watched", systemImage: "checkmark").labelStyle(.titleOnly).foregroundStyle(app.accent)
                    }
                }
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
                if let s = episode.summary, !s.isEmpty {
                    Text(s)
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                        .lineLimit(3)
                        .lineSpacing(2)
                        .blur(radius: spoiler && !hovering ? 5 : 0)
                        .frame(maxWidth: 620, alignment: .leading)
                        .padding(.top, 4)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.vertical, 14)
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
        .onTapGesture { app.open(episode) }
        .contextMenu { ItemMenu(item: episode) }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityAction { app.open(episode) }
    }
}

/// An album's tracks: numbered rows; double-click plays.
struct TrackList: View {
    let tracks: [Metadata]
    @Environment(AppModel.self) private var app
    @State private var selected: String?

    var body: some View {
        let showDisc = Set(tracks.compactMap(\.parentIndex)).count > 1
        let total = tracks.compactMap(\.duration).reduce(0, +)
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .firstTextBaseline) {
                Text("Tracks").font(Theme.sectionTitle)
                Spacer()
                if total > 0 { Text(Fmt.duration(total)).font(Theme.caption).foregroundStyle(.secondary) }
            }
            .padding(.bottom, 8)
            ForEach(Array(tracks.enumerated()), id: \.offset) { i, t in
                if showDisc, t.parentIndex != (i > 0 ? tracks[i - 1].parentIndex : nil), let disc = t.parentIndex {
                    Eyebrow("Disc \(disc)").padding(.top, i > 0 ? 14 : 0).padding(.bottom, 4)
                }
                TrackRow(track: t, selected: selected == t.id) { selected = t.id }
            }
        }
    }
}

struct TrackRow: View {
    let track: Metadata
    let selected: Bool
    let select: () -> Void
    @Environment(AppModel.self) private var app
    @State private var hovering = false

    var body: some View {
        let playing = app.nowPlaying?.item.ratingKey == track.ratingKey && track.ratingKey != nil
        HStack(spacing: 14) {
            ZStack(alignment: .trailing) {
                if hovering && !playing {
                    Button { Actions.play(track) } label: { Image(systemName: "play.fill").font(.system(size: 10)) }
                        .buttonStyle(.plain)
                        .help("Play")
                } else if playing {
                    Image(systemName: "speaker.wave.2.fill").font(.system(size: 10)).foregroundStyle(app.accent)
                } else {
                    Text(track.index.map(String.init) ?? "").foregroundStyle(.tertiary)
                }
            }
            .font(.system(size: 12.5).monospacedDigit())
            .frame(width: 24, alignment: .trailing)
            VStack(alignment: .leading, spacing: 1) {
                Text(track.name).font(.system(size: 13)).foregroundStyle(playing ? AnyShapeStyle(app.accent) : AnyShapeStyle(.primary)).lineLimit(1)
                if let o = track.originalTitle {
                    Text(o).font(Theme.caption).foregroundStyle(.secondary).lineLimit(1)
                }
            }
            Spacer()
            if let r = track.userRating, r > 0 {
                Image(systemName: "star.fill").font(.system(size: 9)).foregroundStyle(.tertiary)
            }
            Text(track.duration.map(Fmt.clock) ?? "")
                .font(.system(size: 12).monospacedDigit())
                .foregroundStyle(.secondary)
        }
        .padding(.horizontal, 8)
        .frame(height: 34)
        .background(
            RoundedRectangle(cornerRadius: 5)
                .fill(selected ? Color.primary.opacity(0.09) : hovering ? Color.primary.opacity(0.04) : .clear)
        )
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
        .gesture(TapGesture(count: 2).onEnded { Actions.play(track) }.exclusively(before: TapGesture().onEnded { select() }))
        .contextMenu { ItemMenu(item: track) }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityAction { Actions.play(track) }
        .help("Double-click to play")
    }
}
