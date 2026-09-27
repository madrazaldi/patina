import SwiftUI

/// Version, audio and subtitle choosers. Audio and subtitle choices are saved
/// to Plex at once, and playback follows them (StreamPickers.svelte).
struct ItemStreamPickers: View {
    let item: Metadata
    @Binding var mediaIndex: Int
    @Environment(AppModel.self) private var app
    /// Choices made here, shown until the refetch catches up.
    @State private var audioChoice: Int?
    @State private var subtitleChoice: Int?

    private var media: Media? { item.media.indices.contains(mediaIndex) ? item.media[mediaIndex] : item.media.first }
    private var part: Part? { media?.parts.first }

    var body: some View {
        if let part {
            let audio = part.audioStreams
            let subs = part.subtitleStreams
            HStack(alignment: .bottom, spacing: 16) {
                if item.media.count > 1 {
                    field("Version") {
                        Picker("Version", selection: $mediaIndex) {
                            ForEach(Array(item.media.enumerated()), id: \.offset) { i, m in
                                Text(m.label.isEmpty ? "Version \(i + 1)" : m.label).tag(i)
                            }
                        }
                    }
                }
                if !audio.isEmpty {
                    field("Audio") {
                        Picker("Audio", selection: Binding(
                            get: { audioChoice ?? part.selectedAudio?.streamID ?? 0 },
                            set: { id in audioChoice = id; save(part, audio: id, subtitle: nil) }
                        )) {
                            ForEach(audio) { s in Text(s.label).tag(s.streamID ?? 0) }
                        }
                    }
                }
                if item.kind != "track" {
                    field("Subtitles") {
                        HStack(spacing: 4) {
                            Picker("Subtitles", selection: Binding(
                                get: { subtitleChoice ?? part.selectedSubtitle?.streamID ?? 0 },
                                set: { id in subtitleChoice = id; save(part, audio: nil, subtitle: id) }
                            )) {
                                Text("None").tag(0)
                                if !subs.isEmpty { Divider() }
                                ForEach(subs) { s in
                                    Text(s.label + (s.isExternal ? " · External" : "")).tag(s.streamID ?? 0)
                                }
                            }
                            if app.owned, let ext = subs.first(where: { $0.selected && $0.isExternal }), let id = ext.streamID {
                                Button { removeSubtitle(id, ext.label) } label: { Image(systemName: "trash") }
                                    .buttonStyle(.borderless)
                                    .help("Delete This Subtitle File")
                            }
                            if app.owned && ["movie", "episode"].contains(item.kind) {
                                Button { app.sheet = .subtitles(item) } label: { Image(systemName: "magnifyingglass") }
                                    .buttonStyle(.borderless)
                                    .help("Search for Subtitles…")
                            }
                        }
                    }
                }
            }
            .labelsHidden()
            .controlSize(.regular)
            .onChange(of: item) { _, _ in audioChoice = nil; subtitleChoice = nil }
            .onChange(of: mediaIndex) { _, _ in audioChoice = nil; subtitleChoice = nil }
        }
    }

    private func field<C: View>(_ label: String, @ViewBuilder _ content: () -> C) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            Eyebrow(label)
            content()
        }
        .frame(minWidth: 180, maxWidth: 320, alignment: .leading)
    }

    private func save(_ part: Part, audio: Int?, subtitle: Int?) {
        guard let id = part.id else { return }
        Task {
            // Show Plex's choice again if saving failed.
            if !(await app.act(refresh: true) { try await API.selectStreams(partId: id, audio: audio, subtitle: subtitle) }) {
                if audio != nil { audioChoice = nil }
                if subtitle != nil { subtitleChoice = nil }
            }
        }
    }

    private func removeSubtitle(_ id: Int, _ label: String) {
        app.confirm("Delete this subtitle?", message: "\(label) is removed from the server.", action: "Delete", destructive: true) {
            await app.act("Subtitle deleted", refresh: true) { try await API.deleteStream(id) }
        }
    }
}

/// Cast (or featured artists) as a row of round portraits; each opens their films.
struct ItemCast: View {
    let item: Metadata
    @Environment(AppModel.self) private var app

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(item.kind == "album" || item.kind == "artist" ? "Featuring" : "Cast")
                .font(Theme.sectionTitle)
                .padding(.horizontal, Theme.gutter)
            ScrollView(.horizontal, showsIndicators: false) {
                LazyHStack(alignment: .top, spacing: 14) {
                    ForEach(Array(item.roles.prefix(40).enumerated()), id: \.offset) { _, role in
                        person(role)
                    }
                }
                .padding(.horizontal, Theme.gutter)
            }
        }
    }

    private func person(_ role: Tag) -> some View {
        let route = ItemFacts.route(item, role)
        return VStack(spacing: 3) {
            Avatar(path: role.thumb, name: role.tag ?? "", size: 72)
                .overlay(Circle().strokeBorder(Theme.hairline))
                .padding(.bottom, 5)
            Text(role.tag ?? "").font(.system(size: 12)).lineLimit(1)
            if let r = role.role {
                Text(r).font(.system(size: 11)).foregroundStyle(.secondary).lineLimit(1)
            }
        }
        .frame(width: 92)
        .contentShape(Rectangle())
        .onTapGesture { if let route { app.open(route) } }
        .help(route != nil ? "Show everything with \(role.tag ?? "")" : role.tag ?? "")
        .contextMenu {
            if let route { Button("Show Titles with \(role.tag ?? "")") { app.open(route) } }
            Button("Copy Name") {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(role.tag ?? "", forType: .string)
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
    }
}
