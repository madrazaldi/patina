// Port of web/src/pages/Item.svelte: the detail page of any item — backdrop, poster, facts,
// the play row, version/audio/subtitle choices, cast, seasons and episodes (or albums,
// tracks, collection items), extras, related and similar titles.

import PatinaCore
import PlexKit
import SwiftUI

struct ItemScreen: View {
    let ratingKey: String
    var server: String?

    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var item = Loader<Metadata?>()
    @State private var children = Loader<[Metadata]>()
    @State private var episodes = Loader<[Metadata]>()
    @State private var hubs = Loader<[Hub]>()
    @State private var season: String?
    @State private var watchlisted: Bool?
    @State private var expanded = false

    private var regular: Bool { sizeClass == .regular }
    private var margin: CGFloat { Theme.margin(sizeClass) }

    var body: some View {
        Group {
            if let found = item.value {
                if let m = found {
                    page(m)
                } else {
                    StatusView(title: "Not found", text: "This item is no longer on the server.", systemImage: "questionmark.square.dashed")
                }
            } else if let error = item.error {
                StatusView(error: error) { item.reload() }
            } else {
                skeleton
            }
        }
        .navigationTitle(item.value??.title ?? "")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if let m = item.value ?? nil {
                ToolbarItem(placement: .topBarTrailing) { ItemMenuButton(item: m) }
            }
        }
        .task(id: store.revision) { await load() }
    }

    // MARK: Loading

    private func load(force: Bool = false) async {
        let (app, key, server, revision) = (store.app!, ratingKey, server, store.revision)
        if !force, item.value != nil, item.revision != revision, !store.affects(item: key, since: item.revision) { return }
        if force { await item.refresh() } else {
            await item.load(at: revision) { try await Commands.metadata(app, server: server, ratingKey: key) }
        }
        guard let m = item.value ?? nil else { return }
        let kind = Metadata.kind(m)
        if ["show", "season", "artist", "album", "collection", "playlist"].contains(kind) || m.extra["subtype"] != nil {
            await children.load(at: revision) {
                try await Commands.children(app, server: server, ratingKey: key, collection: kind == "collection")
            }
            if kind == "show" { await loadSeason(season ?? defaultSeason(m), revision: revision) }
        }
        if watchlisted == nil, let id = Metadata.discoverId(m) {
            watchlisted = try? await PendingCommands.watchlisted(app, discoverId: id)
        }
        await hubs.load(at: revision) { try await PendingCommands.itemHubs(app, ratingKey: key, kind: kind, server: server) }
    }

    /// The season to open on: the one of the next episode, or the first unwatched one.
    private func defaultSeason(_ show: Metadata) -> String? {
        let seasons = children.value ?? []
        if let next = Metadata.onDeckItem(show)?.parentRatingKey { return next }
        return (seasons.first { !Metadata.isWatched($0) && ($0.index ?? 0) > 0 } ?? seasons.first)?.ratingKey
    }

    private func loadSeason(_ key: String?, revision: Int? = nil) async {
        guard let key else { return }
        season = key
        let (app, server) = (store.app!, server)
        if let revision {
            await episodes.load(at: revision) { try await Commands.children(app, server: server, ratingKey: key) }
        } else {
            episodes = Loader()
            await episodes.load { try await Commands.children(app, server: server, ratingKey: key) }
        }
    }

    // MARK: Page

    private func page(_ m: Metadata) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 28) {
                hero(m)
                if let error = item.error {
                    StaleBanner(error: error) { Task { await load(force: true) } }.padding(.horizontal, margin)
                }
                streams(m).padding(.horizontal, margin)
                facts(m)
                cast(m)
                childList(m)
                if !Metadata.extras(m).isEmpty {
                    CardRow(title: "Extras", items: Metadata.extras(m), shape: .landscape)
                }
                ForEach(Array(Metadata.relatedHubs(m).enumerated()), id: \.offset) { _, hub in
                    if !Hub.isEmpty(hub) { HubRow(hub: hub) }
                }
                ForEach(Array((hubs.value ?? []).enumerated()), id: \.offset) { _, hub in
                    if !Hub.isEmpty(hub) { HubRow(hub: hub) }
                }
            }
            .padding(.bottom, 32)
        }
        .ignoresSafeArea(edges: .top)
        .toolbarBackground(.hidden, for: .navigationBar)
        .refreshable { await load(force: true) }
    }

    // MARK: Hero

    private func hero(_ m: Metadata) -> some View {
        let height: CGFloat = regular ? 520 : 440
        let kind = Metadata.kind(m)
        let square = PlexUI.shape(m) == .square
        return VStack(alignment: .leading, spacing: 18) {
            ZStack(alignment: .bottomLeading) {
                // The backdrop moves at half speed as the page scrolls, and stretches when pulled.
                Artwork(Metadata.backdrop(m), width: 1600, height: 900, server: Metadata.serverId(m))
                    .frame(maxWidth: .infinity)
                    .frame(height: height)
                    .visualEffect { content, proxy in
                        let y = proxy.frame(in: .scrollView).minY
                        return content
                            .scaleEffect(y > 0 ? 1 + y / height : 1, anchor: .bottom)
                            .offset(y: y > 0 ? 0 : -y * 0.5)
                    }
                    .overlay {
                        LinearGradient(
                            stops: [
                                .init(color: Theme.background.opacity(0.25), location: 0),
                                .init(color: Theme.background.opacity(0.1), location: 0.35),
                                .init(color: Theme.background.opacity(0.85), location: 0.78),
                                .init(color: Theme.background, location: 1),
                            ], startPoint: .top, endPoint: .bottom)
                    }
                    .clipped()
                HStack(alignment: .bottom, spacing: regular ? 28 : 16) {
                    Artwork(Metadata.poster(m), width: 300, height: square ? 300 : 450, server: Metadata.serverId(m))
                        .frame(width: regular ? 190 : 112, height: (regular ? 190 : 112) * (square ? 1 : 1.5))
                        .clipShape(.rect(cornerRadius: 12, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(Theme.hairline))
                        .shadow(color: .black.opacity(0.5), radius: 18, y: 8)
                    VStack(alignment: .leading, spacing: 6) {
                        crumbs(m)
                        Text(Metadata.title(m))
                            .font(regular ? .largeTitle.bold() : .title.bold())
                            .lineLimit(3)
                            .minimumScaleFactor(0.7)
                        if let original = m.originalTitle, original != m.title, kind != "episode", kind != "track" {
                            Text(original).font(.subheadline).foregroundStyle(Theme.secondaryText)
                        }
                        chips(m)
                        ratings(m)
                    }
                }
                .padding(.horizontal, margin)
                .padding(.bottom, 4)
            }
            .frame(height: height)
            .containerRelativeFrame(.horizontal)
            Group {
                actions(m)
                summary(m)
            }
            .padding(.horizontal, margin)
        }
    }

    /// The show, season or artist above the item.
    @ViewBuilder
    private func crumbs(_ m: Metadata) -> some View {
        let kind = Metadata.kind(m)
        HStack(spacing: 6) {
            if kind == "episode" || kind == "season" || kind == "album" || kind == "track" {
                let top = kind == "season" || kind == "album" ? m.parentRatingKey : m.grandparentRatingKey
                let title = kind == "season" || kind == "album" ? m.parentTitle : m.grandparentTitle
                if let top, let title {
                    NavigationLink(value: Route.item(ratingKey: top, server: server)) {
                        Text(title).lineLimit(1)
                    }
                }
                if kind == "episode" {
                    Text(Metadata.episodeCode(m)).foregroundStyle(Theme.secondaryText)
                } else if kind == "track", let album = m.parentTitle, let key = m.parentRatingKey {
                    Text("·").foregroundStyle(Theme.tertiaryText)
                    NavigationLink(value: Route.item(ratingKey: key, server: server)) { Text(album).lineLimit(1) }
                }
            } else {
                Text(kind == "collection" ? "COLLECTION" : kind.uppercased())
                    .kerning(1.1)
                    .foregroundStyle(store.accent)
            }
        }
        .font(.caption.weight(.bold))
        .buttonStyle(.plain)
        .foregroundStyle(store.accent)
    }

    private func chips(_ m: Metadata) -> some View {
        let media = m.media.first
        let part = media?.part.first
        var bits: [String] = []
        if Metadata.kind(m) == "episode", let date = m.originallyAvailableAt { bits.append(Format.airDate(date)) } else if let y = m.year {
            bits.append(String(y))
        }
        if let r = m.contentRating { bits.append(r) }
        if let d = m.duration, d > 0 { bits.append(Format.duration(ms: Int64(d))) }
        if let n = m.childCount, Metadata.kind(m) == "show" { bits.append(Format.plural(n, "season")) }
        if let n = m.leafCount, ["show", "season", "album"].contains(Metadata.kind(m)) {
            bits.append(Format.plural(n, Metadata.kind(m) == "album" ? "track" : "episode"))
        }
        var badges: [String] = []
        if let res = PlexUI.resolution(media?.videoResolution) { badges.append(res) }
        if let video = part.flatMap(Part.videoStream), let hdr = MediaStream.hdrLabel(video) { badges.append(hdr) }
        if let audio = part.flatMap(Part.selectedAudio) ?? part.flatMap({ Part.audioStreams($0).first }) {
            badges.append(MediaStream.shortLabel(audio))
        }
        return FlowLayout(spacing: 6) {
            if !bits.isEmpty {
                Text(bits.joined(separator: " · ")).font(.subheadline).foregroundStyle(Theme.secondaryText)
            }
            ForEach(badges, id: \.self) { b in
                Text(b)
                    .font(.caption2.weight(.bold))
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .overlay(RoundedRectangle(cornerRadius: 4).strokeBorder(Theme.secondaryText.opacity(0.6)))
                    .foregroundStyle(Theme.secondaryText)
            }
        }
    }

    @ViewBuilder
    private func ratings(_ m: Metadata) -> some View {
        let critic = m.rating.map { r in (m.ratingImage ?? "").contains("rottentomatoes") ? "🍅 \(Int(r * 10))%" : "★ \(String(format: "%.1f", r))" }
        let audience = m.audienceRating.map { r in
            (m.audienceRatingImage ?? "").contains("rottentomatoes") ? "🍿 \(Int(r * 10))%" : "Audience \(String(format: "%.1f", r))"
        }
        if critic != nil || audience != nil {
            HStack(spacing: 12) {
                if let critic { Text(critic) }
                if let audience { Text(audience) }
            }
            .font(.footnote.weight(.semibold).monospacedDigit())
            .foregroundStyle(Theme.secondaryText)
        }
    }

    // MARK: Actions

    private func actions(_ m: Metadata) -> some View {
        let kind = Metadata.kind(m)
        let progress = Metadata.progress(m)
        let resumable = (m.viewOffset ?? 0) > 0 && kind != "show" && kind != "season"
        let next = kind == "show" || kind == "season" ? Metadata.onDeckItem(m) : nil
        let playLabel: String = {
            if let next, (next.viewOffset ?? 0) > 0 || Metadata.isWatched(next) == false && (m.viewedLeafCount ?? 0) > 0 {
                return "\((next.viewOffset ?? 0) > 0 ? "Resume" : "Play") \(Metadata.episodeCode(next))"
            }
            return resumable ? "Resume" : "Play"
        }()
        let container = Metadata.isContainer(m) || kind == "collection" || kind == "playlist"
        let dl = store.downloaded(m)
        return VStack(alignment: .leading, spacing: 10) {
            FlowLayout(spacing: 10) {
                if PlexUI.canPlay(m) {
                    Button {
                        Haptics.tap()
                        store.play(m)
                    } label: {
                        VStack(spacing: 4) {
                            Label(playLabel, systemImage: "play.fill").font(.headline).lineLimit(1)
                            if resumable, let left = Format.remaining(durationMs: m.duration.map(Int64.init), viewOffsetMs: m.viewOffset.map(Int64.init)) {
                                Text(left).font(.caption2).opacity(0.75)
                            }
                        }
                        .frame(minWidth: regular ? 150 : 120, minHeight: 36)
                    }
                    .buttonStyle(.borderedProminent)
                    .foregroundStyle(.black)
                    .buttonBorderShape(.capsule)
                    if resumable {
                        Button("From Start", systemImage: "arrow.counterclockwise") {
                            store.play(m, .init(resume: false))
                        }
                        .labelStyle(.iconOnly)
                        .buttonStyle(.bordered)
                        .controlSize(.large)
                    }
                    if container {
                        Button("Shuffle", systemImage: "shuffle") { store.play(m, .init(shuffle: true)) }
                            .labelStyle(.iconOnly)
                            .buttonStyle(.bordered)
                            .controlSize(.large)
                    }
                }
                if let trailer = trailer(m) {
                    Button("Trailer", systemImage: "film") { store.play(trailer) }
                        .labelStyle(.iconOnly)
                        .buttonStyle(.bordered)
                        .controlSize(.large)
                }
                if let id = Metadata.discoverId(m), let on = watchlisted {
                    Button(on ? "On Watchlist" : "Add to Watchlist", systemImage: on ? "bookmark.fill" : "bookmark") {
                        toggleWatchlist(id, on: !on)
                    }
                    .labelStyle(.iconOnly)
                    .buttonStyle(.bordered)
                    .controlSize(.large)
                    .contentTransition(.symbolEffect(.replace))
                }
                if PlexUI.hasWatchState(m) {
                    let watched = Metadata.isWatched(m)
                    Button(watched ? "Mark Unwatched" : "Mark Watched", systemImage: watched ? "checkmark.circle.fill" : "checkmark.circle") {
                        store.setWatched(m, !watched)
                    }
                    .labelStyle(.iconOnly)
                    .buttonStyle(.bordered)
                    .controlSize(.large)
                    .contentTransition(.symbolEffect(.replace))
                }
                if PlexUI.canPlay(m) && kind != "collection" && kind != "playlist" {
                    Button(dl == nil ? "Download" : "Downloaded", systemImage: dl == nil ? "arrow.down.circle" : "arrow.down.circle.fill") {
                        if dl == nil { store.download(m) } else { store.navigate(to: .downloads) }
                    }
                    .labelStyle(.iconOnly)
                    .buttonStyle(.bordered)
                    .controlSize(.large)
                }
            }
            .buttonBorderShape(.circle)
            if let progress, progress > 0, progress < 1, !regular {
                ProgressStrip(value: progress, height: 3).frame(maxWidth: 240)
            }
            if ["movie", "show", "season", "episode", "artist", "album", "track"].contains(kind) {
                StarRating(value: m.userRating ?? 0) { rating in
                    store.perform { try await Commands.rate(store.app, ratingKey: Metadata.ratingKey(m), rating: rating) }
                }
            }
        }
    }

    private func trailer(_ m: Metadata) -> Metadata? {
        let extras = Metadata.extras(m)
        if let key = m.primaryExtraKey, let found = extras.first(where: { $0.key == key || $0.ratingKey == key.split(separator: "/").last.map(String.init) }) {
            return found
        }
        return extras.first { $0.subtype == "trailer" || $0.extraType == 1 }
    }

    private func toggleWatchlist(_ id: String, on: Bool) {
        Haptics.tap()
        watchlisted = on
        Task {
            if !(await store.act(on ? "Added to Watchlist" : "Removed from Watchlist", {
                try await PendingCommands.setWatchlisted(store.app, discoverId: id, on: on)
            })) {
                watchlisted = !on
            }
        }
    }

    @ViewBuilder
    private func summary(_ m: Metadata) -> some View {
        let hidden = store.spoilers && Metadata.kind(m) == "episode" && !Metadata.isWatched(m)
        VStack(alignment: .leading, spacing: 6) {
            if let tagline = m.tagline, !tagline.isEmpty {
                Text(tagline).font(.headline).italic().foregroundStyle(Theme.secondaryText)
            }
            if let text = m.summary, !text.isEmpty {
                if hidden {
                    Label("Summary hidden until watched", systemImage: "eye.slash")
                        .font(.callout).foregroundStyle(Theme.tertiaryText)
                } else {
                    Text(text)
                        .font(.callout)
                        .foregroundStyle(.primary.opacity(0.88))
                        .lineLimit(expanded ? nil : 4)
                        .frame(maxWidth: 760, alignment: .leading)
                        .onTapGesture { withAnimation(.snappy) { expanded.toggle() } }
                    if text.count > 260 {
                        Button(expanded ? "Less" : "More") { withAnimation(.snappy) { expanded.toggle() } }
                            .font(.callout.weight(.semibold))
                    }
                }
            }
        }
    }

    // MARK: Streams

    /// Version, audio and subtitle choices, saved on the server per part.
    @ViewBuilder
    private func streams(_ m: Metadata) -> some View {
        if PlexUI.supportsTracks(m), let media = m.media.first {
            let versions = m.media
            let picked = versions.first(where: \.selected) ?? media
            if let part = picked.part.first, let partId = part.id {
                let audio = Part.audioStreams(part)
                let subs = Part.subtitleStreams(part)
                let current = Part.selectedSubtitle(part)
                FlowLayout(spacing: 8) {
                    if versions.count > 1 {
                        Menu {
                            ForEach(Array(versions.enumerated()), id: \.offset) { i, v in
                                Button(Media.label(v)) { store.play(m, .init(mediaIndex: i)) }
                            }
                        } label: {
                            pill("Version", Media.label(picked), "square.stack.3d.up")
                        }
                    }
                    if !audio.isEmpty {
                        Menu {
                            ForEach(Array(audio.enumerated()), id: \.offset) { _, s in
                                Toggle(MediaStream.label(s), isOn: Binding(get: { s.selected }, set: { _ in pick(m, partId, audio: s.id) }))
                            }
                        } label: {
                            pill("Audio", Part.selectedAudio(part).map(MediaStream.shortLabel) ?? MediaStream.shortLabel(audio[0]), "speaker.wave.2")
                        }
                    }
                    Menu {
                        Toggle("Off", isOn: Binding(get: { current == nil }, set: { _ in pick(m, partId, subtitle: 0) }))
                        ForEach(Array(subs.enumerated()), id: \.offset) { _, s in
                            Toggle(MediaStream.label(s), isOn: Binding(get: { s.selected }, set: { _ in pick(m, partId, subtitle: s.id) }))
                        }
                        Divider()
                        if store.owned {
                            Button("Search for Subtitles…", systemImage: "magnifyingglass") { store.sheet = .subtitle(m) }
                            if let current, MediaStream.isExternal(current), let id = current.id {
                                Button("Delete “\(MediaStream.shortLabel(current))”", systemImage: "trash", role: .destructive) {
                                    deleteSubtitle(id)
                                }
                            }
                        }
                        if Metadata.kind(m) == "episode" {
                            // TODO(merge): Commands.setShowStreams (bulk "for the whole show") is a later task.
                            Button("Audio & Subtitles for the Whole Show…", systemImage: "rectangle.stack") {}
                                .disabled(true)
                        }
                    } label: {
                        pill("Subtitles", current.map(MediaStream.shortLabel) ?? "Off", "captions.bubble")
                    }
                }
            }
        }
    }

    private func pill(_ title: String, _ value: String, _ icon: String) -> some View {
        HStack(spacing: 6) {
            Image(systemName: icon).foregroundStyle(store.accent)
            Text(title).foregroundStyle(Theme.secondaryText)
            Text(value).fontWeight(.medium).lineLimit(1)
            Image(systemName: "chevron.up.chevron.down").font(.caption2).foregroundStyle(Theme.tertiaryText)
        }
        .font(.subheadline)
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(Theme.raised, in: .capsule)
        .overlay(Capsule().strokeBorder(Theme.hairline))
        .foregroundStyle(.primary)
    }

    private func pick(_ m: Metadata, _ partId: Int, audio: Int? = nil, subtitle: Int? = nil) {
        Haptics.selection()
        let server = Metadata.serverId(m) ?? server
        Task {
            if await store.act(nil, { try await Commands.selectStreams(store.app, server: server, partId: partId, audio: audio, subtitle: subtitle) }) {
                await load(force: true)
            }
        }
    }

    private func deleteSubtitle(_ id: Int) {
        Task {
            guard await store.confirm("Delete this subtitle file?", text: "The file is removed from the server.", action: "Delete", danger: true)
            else { return }
            if await store.act("Subtitle deleted", { try await Commands.deleteStream(store.app, id: id) }) {
                await load(force: true)
            }
        }
    }

    // MARK: Facts and cast

    @ViewBuilder
    private func facts(_ m: Metadata) -> some View {
        let section = m.librarySectionID.map(String.init)
        let rows: [(String, [Tag])] = [
            ("Genre", m.genre), ("Director", m.director), ("Writer", m.writer), ("Country", m.country), ("Mood", m.mood),
            ("Style", m.style), ("Collection", m.collection),
        ].filter { !$0.1.isEmpty }
        if !rows.isEmpty || m.studio != nil {
            LazyVGrid(
                columns: regular ? [GridItem(.flexible(), alignment: .topLeading), GridItem(.flexible(), alignment: .topLeading)] : [GridItem(.flexible(), alignment: .topLeading)],
                alignment: .leading, spacing: 10
            ) {
                ForEach(rows, id: \.0) { title, tags in
                    HStack(alignment: .firstTextBaseline, spacing: 12) {
                        Text(title).font(.subheadline).foregroundStyle(Theme.secondaryText).frame(width: 84, alignment: .leading)
                        FlowLayout(spacing: 6) {
                            ForEach(Array(tags.prefix(8).enumerated()), id: \.offset) { i, tag in
                                tagLink(tag, section: section, last: i == min(tags.count, 8) - 1)
                            }
                        }
                    }
                }
                if let studio = m.studio {
                    HStack(alignment: .firstTextBaseline, spacing: 12) {
                        Text("Studio").font(.subheadline).foregroundStyle(Theme.secondaryText).frame(width: 84, alignment: .leading)
                        Text(studio).font(.subheadline)
                    }
                }
            }
            .padding(16)
            .panel()
            .padding(.horizontal, margin)
        }
    }

    @ViewBuilder
    private func tagLink(_ tag: Tag, section: String?, last: Bool) -> some View {
        let text = Text(Tag.name(tag) + (last ? "" : ","))
            .font(.subheadline)
        if let section, let filter = tag.filter {
            NavigationLink(value: Route.browse(path: "/library/sections/\(section)/all?\(filter)", title: Tag.name(tag))) { text }
                .buttonStyle(.plain)
        } else {
            text
        }
    }

    @ViewBuilder
    private func cast(_ m: Metadata) -> some View {
        if !m.role.isEmpty {
            let section = m.librarySectionID.map(String.init)
            VStack(alignment: .leading, spacing: 12) {
                SectionHeader("Cast")
                ScrollView(.horizontal) {
                    LazyHStack(alignment: .top, spacing: 14) {
                        ForEach(Array(m.role.prefix(40).enumerated()), id: \.offset) { _, person in
                            let card = VStack(spacing: 6) {
                                Avatar(name: Tag.name(person), thumb: person.thumb, size: regular ? 88 : 72, server: Metadata.serverId(m))
                                Text(Tag.name(person)).font(.caption.weight(.medium)).lineLimit(2)
                                if let role = person.role {
                                    Text(role).font(.caption2).foregroundStyle(Theme.secondaryText).lineLimit(2)
                                }
                            }
                            .multilineTextAlignment(.center)
                            .frame(width: regular ? 100 : 84)
                            if let section, let filter = person.filter {
                                NavigationLink(value: Route.browse(path: "/library/sections/\(section)/all?\(filter)", title: Tag.name(person))) {
                                    card
                                }
                                .buttonStyle(CardButtonStyle())
                            } else {
                                card
                            }
                        }
                    }
                }
                .scrollIndicators(.hidden)
                .contentMargins(.horizontal, margin, for: .scrollContent)
            }
        }
    }

    // MARK: Children

    @ViewBuilder
    private func childList(_ m: Metadata) -> some View {
        let kind = Metadata.kind(m)
        let list = children.value ?? []
        switch kind {
        case "show":
            seasons(m, list)
        case "season":
            VStack(alignment: .leading, spacing: 12) {
                SectionHeader(Format.plural(list.count, "Episode"))
                EpisodeList(episodes: list)
            }
        case "album", "playlist":
            VStack(alignment: .leading, spacing: 12) {
                SectionHeader(Format.plural(list.count, "Track"))
                TrackList(tracks: list)
            }
        case "artist":
            if !list.isEmpty {
                VStack(alignment: .leading, spacing: 12) {
                    SectionHeader("Albums")
                    LazyVGrid(columns: cardColumns(.square, sizeClass), spacing: 22) {
                        ForEach(Array(list.enumerated()), id: \.offset) { _, a in Card(a, shape: .square) }
                    }
                    .padding(.horizontal, margin)
                }
            }
        case "collection":
            VStack(alignment: .leading, spacing: 12) {
                SectionHeader(Format.plural(list.count, "Item")) {
                    if store.owned { collectionMenu(m) }
                }
                let shape = PlexUI.commonShape(list)
                LazyVGrid(columns: cardColumns(shape, sizeClass), spacing: 22) {
                    ForEach(Array(list.enumerated()), id: \.offset) { _, child in
                        Card(child, shape: shape)
                            .contextMenu {
                                ItemMenu(item: child)
                                if store.owned && !m.smart {
                                    Button("Remove from Collection", systemImage: "minus.circle", role: .destructive) {
                                        store.perform("Removed from collection", refresh: true) {
                                            try await PendingCommands.removeFromCollection(
                                                store.app, id: Metadata.ratingKey(m), ratingKey: Metadata.ratingKey(child))
                                        }
                                    }
                                }
                            }
                    }
                }
                .padding(.horizontal, margin)
            }
        default:
            EmptyView()
        }
    }

    private func collectionMenu(_ m: Metadata) -> some View {
        let id = Metadata.ratingKey(m)
        return Menu {
            Picker("In Library", selection: Binding(get: { m.collectionMode ?? -1 }, set: { mode in
                store.perform("Collection updated", refresh: true) { try await PendingCommands.collectionSettings(store.app, id: id, mode: mode) }
            })) {
                Text("Library Default").tag(-1)
                Text("Hide Items").tag(0)
                Text("Hide Collection").tag(1)
                Text("Show Both").tag(2)
            }
            Picker("Order", selection: Binding(get: { Int(PlexUI.extra(m, "collectionSort") ?? "0") ?? 0 }, set: { sort in
                store.perform("Collection updated", refresh: true) { try await PendingCommands.collectionSettings(store.app, id: id, sort: sort) }
            })) {
                Text("Release Date").tag(0)
                Text("Alphabetical").tag(1)
                Text("Custom").tag(2)
            }
        } label: {
            Image(systemName: "slider.horizontal.3")
        }
    }

    @ViewBuilder
    private func seasons(_ show: Metadata, _ list: [Metadata]) -> some View {
        if !list.isEmpty {
            VStack(alignment: .leading, spacing: 14) {
                SectionHeader(list.count == 1 ? Metadata.title(list[0]) : "Seasons")
                if list.count > 1 {
                    ScrollView(.horizontal) {
                        HStack(spacing: 8) {
                            ForEach(Array(list.enumerated()), id: \.offset) { _, s in
                                let on = s.ratingKey == season
                                Button {
                                    Haptics.selection()
                                    Task { await loadSeason(s.ratingKey) }
                                } label: {
                                    HStack(spacing: 6) {
                                        Text(Metadata.title(s))
                                        if Metadata.isWatched(s) { Image(systemName: "checkmark").font(.caption2.weight(.bold)) }
                                    }
                                    .font(.subheadline.weight(on ? .semibold : .medium))
                                    .foregroundStyle(on ? Color.black : .primary)
                                    .padding(.horizontal, 14)
                                    .padding(.vertical, 8)
                                    .background(on ? AnyShapeStyle(store.accent) : AnyShapeStyle(Theme.raised), in: .capsule)
                                }
                                .buttonStyle(.plain)
                                .contextMenu { ItemMenu(item: s) }
                            }
                        }
                    }
                    .scrollIndicators(.hidden)
                    .contentMargins(.horizontal, margin, for: .scrollContent)
                }
                if let error = episodes.error, episodes.value == nil {
                    StatusView(error: error) { Task { await loadSeason(season) } }
                } else if let eps = episodes.value {
                    EpisodeList(episodes: eps)
                } else {
                    VStack(spacing: 12) { ForEach(0..<3, id: \.self) { _ in Skeleton().frame(height: 90) } }
                        .padding(.horizontal, margin)
                }
            }
        }
    }

    private var skeleton: some View {
        VStack(alignment: .leading, spacing: 16) {
            Skeleton().frame(height: regular ? 420 : 300)
            Skeleton().frame(width: 220, height: 28).padding(.horizontal, margin)
            Skeleton().frame(height: 80).padding(.horizontal, margin)
            Spacer()
        }
        .ignoresSafeArea(edges: .top)
    }
}

/// Episodes with a still, title, summary, progress and watched state.
struct EpisodeList: View {
    let episodes: [Metadata]
    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        let regular = sizeClass == .regular
        LazyVStack(spacing: 14) {
            ForEach(Array(episodes.enumerated()), id: \.offset) { _, e in
                let hidden = store.spoilers && !Metadata.isWatched(e)
                NavigationLink(value: PlexUI.route(e)) {
                    HStack(alignment: .top, spacing: 14) {
                        ZStack(alignment: .bottom) {
                            Artwork(e.thumb, width: 560, height: 315, server: Metadata.serverId(e), blur: hidden)
                                .aspectRatio(16 / 9, contentMode: .fit)
                            if let p = Metadata.progress(e), p > 0, p < 1 { ProgressStrip(value: p, height: 3) }
                        }
                        .frame(width: regular ? 240 : 150)
                        .clipShape(.rect(cornerRadius: 8, style: .continuous))
                        .overlay(alignment: .topTrailing) {
                            if Metadata.isWatched(e) {
                                Image(systemName: "checkmark.circle.fill")
                                    .symbolRenderingMode(.palette)
                                    .foregroundStyle(.black, store.accent)
                                    .padding(5)
                            }
                        }
                        VStack(alignment: .leading, spacing: 4) {
                            Text("\(e.index.map { "\($0). " } ?? "")\(Metadata.title(e))")
                                .font(.subheadline.weight(.semibold))
                                .lineLimit(2)
                            Text(
                                [e.duration.map { Format.duration(ms: Int64($0)) }, e.originallyAvailableAt.map(Format.airDate)]
                                    .compactMap { $0 }.joined(separator: " · ")
                            )
                            .font(.caption)
                            .foregroundStyle(Theme.secondaryText)
                            if let s = e.summary, !s.isEmpty, !hidden {
                                Text(s).font(.caption).foregroundStyle(Theme.secondaryText).lineLimit(regular ? 3 : 2)
                            }
                        }
                        Spacer(minLength: 0)
                    }
                    .contentShape(.rect)
                }
                .buttonStyle(CardButtonStyle())
                .contextMenu { ItemMenu(item: e) }
            }
        }
        .padding(.horizontal, Theme.margin(sizeClass))
    }
}

/// Tracks of an album or playlist: number, title, artist and length; tap to play.
struct TrackList: View {
    let tracks: [Metadata]
    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        VStack(spacing: 0) {
            ForEach(Array(tracks.enumerated()), id: \.offset) { i, t in
                if i > 0 { Divider().overlay(Theme.hairline).padding(.leading, 48) }
                Button {
                    Haptics.tap()
                    store.play(t)
                } label: {
                    HStack(spacing: 12) {
                        Text(t.index.map(String.init) ?? "\(i + 1)")
                            .font(.footnote.monospacedDigit())
                            .foregroundStyle(Theme.tertiaryText)
                            .frame(width: 24, alignment: .trailing)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(Metadata.title(t)).font(.subheadline).lineLimit(1)
                            if let artist = t.originalTitle {
                                Text(artist).font(.caption).foregroundStyle(Theme.secondaryText).lineLimit(1)
                            }
                        }
                        Spacer()
                        if let d = t.duration {
                            Text(Format.clock(ms: Int64(d))).font(.caption.monospacedDigit()).foregroundStyle(Theme.secondaryText)
                        }
                    }
                    .padding(.horizontal, 14)
                    .padding(.vertical, 11)
                    .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .contextMenu { ItemMenu(item: t) }
            }
        }
        .panel()
        .padding(.horizontal, Theme.margin(sizeClass))
    }
}

#Preview {
    NavigationStack {
        ItemScreen(ratingKey: "1")
    }
    .environment(AppStore.preview())
    .preferredColorScheme(.dark)
}
