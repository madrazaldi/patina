import SwiftUI

/// Cover, title, count and duration, summary and actions of a playlist.
struct PlaylistHeader: View {
    let playlist: Metadata
    let items: [Metadata]
    @Binding var editing: Bool
    let clear: () -> Void
    let delete: () -> Void

    var body: some View {
        let total = items.compactMap(\.duration).reduce(0, +)
        let empty = items.isEmpty
        HStack(alignment: .bottom, spacing: 30) {
            Color.clear.frame(width: 190, height: 190)
                .overlay { Artwork(playlist.composite ?? playlist.thumb, width: 480, height: 480, fallback: playlist.name) }
                .clipShape(RoundedRectangle(cornerRadius: Theme.radius))
                .overlay(RoundedRectangle(cornerRadius: Theme.radius).strokeBorder(Theme.hairline))
            VStack(alignment: .leading, spacing: 0) {
                Eyebrow(playlist.smart ? "Smart Playlist" : "Playlist")
                Text(playlist.name)
                    .font(Theme.display(38))
                    .lineLimit(2)
                    .textSelection(.enabled)
                    .padding(.top, 6)
                Text([Fmt.plural(items.count, "item"), total > 0 ? Fmt.duration(total) : nil].compactMap { $0 }.joined(separator: " · "))
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
                    .padding(.top, 6)
                if let s = playlist.summary, !s.isEmpty {
                    Text(s)
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                        .lineLimit(4)
                        .frame(maxWidth: 560, alignment: .leading)
                        .textSelection(.enabled)
                        .padding(.top, 10)
                }
                HStack(spacing: 8) {
                    Button { Actions.play(playlist) } label: {
                        Label("Play", systemImage: "play.fill")
                    }
                    .buttonStyle(ItemPrimaryButtonStyle())
                    .keyboardShortcut(.return, modifiers: .command)
                    .disabled(empty)
                    Button("Shuffle", systemImage: "shuffle") {
                        var o = PlayOptions(); o.shuffle = true
                        Actions.play(playlist, o)
                    }
                    .disabled(empty)
                    Menu {
                        Button("Edit Title & Description…", systemImage: "pencil") { editing = true }
                        if !playlist.smart && !empty {
                            Button("Clear Playlist…", systemImage: "xmark.circle") { clear() }
                        }
                        Divider()
                        Button("Delete Playlist…", systemImage: "trash", role: .destructive) { delete() }
                    } label: {
                        Image(systemName: "ellipsis")
                    }
                    .menuIndicator(.hidden)
                    .fixedSize()
                    .help("More")
                }
                .controlSize(.large)
                .foregroundStyle(.primary)
                .padding(.top, 18)
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, Theme.gutter)
        .padding(.top, 28)
        .padding(.bottom, 26)
    }
}

/// One playlist entry: position, thumbnail, title, subtitle, duration, and hover actions.
struct PlaylistRow: View {
    let number: Int
    let item: Metadata
    let removable: Bool
    let remove: () -> Void
    @Environment(AppModel.self) private var app
    @State private var hovering = false

    var body: some View {
        HStack(spacing: 12) {
            Text("\(number)")
                .font(.system(size: 12).monospacedDigit())
                .foregroundStyle(.tertiary)
                .frame(width: 26, alignment: .trailing)
            Color.clear.frame(width: 32, height: item.shape == .square ? 32 : 48)
                .overlay { Artwork(item.posterPath, width: 96, height: 144) }
                .clipShape(RoundedRectangle(cornerRadius: 2))
            VStack(alignment: .leading, spacing: 2) {
                Text(item.fullTitle).font(.system(size: 13)).lineLimit(1)
                Text(item.subtitle).font(Theme.caption).foregroundStyle(.secondary).lineLimit(1)
            }
            Spacer(minLength: 8)
            if hovering {
                HStack(spacing: 2) {
                    Button { Actions.play(item) } label: { Image(systemName: "play.fill") }.help("Play")
                    Button { Actions.enqueue(item, next: true) } label: { Image(systemName: "text.line.first.and.arrowtriangle.forward") }.help("Play Next")
                    if removable {
                        Button(action: remove) { Image(systemName: "xmark") }.help("Remove from Playlist")
                    }
                }
                .buttonStyle(.borderless)
                .foregroundStyle(.secondary)
            }
            if let p = item.progress, !item.isWatched {
                ProgressView(value: p).frame(width: 40).tint(app.accent).controlSize(.mini)
            } else if item.hasWatchState && item.isWatched {
                Image(systemName: "checkmark").font(.system(size: 10, weight: .semibold)).foregroundStyle(app.accent).help("Watched")
            }
            Text(item.duration.map(Fmt.duration) ?? "")
                .font(.system(size: 12).monospacedDigit())
                .foregroundStyle(.secondary)
                .frame(minWidth: 48, alignment: .trailing)
        }
        .padding(.vertical, 6)
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
    }
}

/// Rename a playlist and edit its description.
struct PlaylistEditSheet: View {
    let id: String
    @State var title: String
    @State var summary: String
    @Environment(AppModel.self) private var app
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Edit Playlist").font(Theme.display(20))
            Form {
                TextField("Title", text: $title)
                LabeledContent("Description") {
                    TextEditor(text: $summary)
                        .font(.body)
                        .frame(height: 90)
                        .scrollContentBackground(.hidden)
                        .padding(4)
                        .background(RoundedRectangle(cornerRadius: 5).fill(Color(nsColor: .textBackgroundColor)))
                        .overlay(RoundedRectangle(cornerRadius: 5).strokeBorder(Theme.hairline))
                }
            }
            HStack {
                Spacer()
                Button("Cancel", role: .cancel) { dismiss() }.keyboardShortcut(.cancelAction)
                Button("Save") { save() }
                    .keyboardShortcut(.defaultAction)
                    .disabled(title.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
        .padding(20)
        .frame(width: 440)
    }

    private func save() {
        let t = title.trimmingCharacters(in: .whitespacesAndNewlines)
        let s = summary.trimmingCharacters(in: .whitespacesAndNewlines)
        let pid = id
        dismiss()
        app.perform("Playlist updated", refresh: true) { try await API.updatePlaylist(pid, title: t, summary: s) }
    }
}
