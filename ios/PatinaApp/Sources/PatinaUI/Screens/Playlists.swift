// Ports of web/src/pages/Playlists.svelte and Playlist.svelte: the server's playlists, and
// one playlist to play, rename, clear, delete, reorder by drag, and remove entries from
// (with Undo).

import PatinaCore
import PlexKit
import SwiftUI

struct PlaylistsScreen: View {
    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var lists = Loader<[Metadata]>()

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("Long-press anything and choose Add to Playlist… to build one.")
                    .font(.subheadline)
                    .foregroundStyle(Theme.secondaryText)
                    .padding(.horizontal, Theme.margin(sizeClass))
                if let error = lists.error, lists.value != nil {
                    StaleBanner(error: error) { lists.reload() }.padding(.horizontal, Theme.margin(sizeClass))
                }
                if let value = lists.value {
                    if value.isEmpty {
                        StatusView(title: "No playlists yet", systemImage: "music.note.list")
                    } else {
                        LazyVGrid(columns: cardColumns(.square, sizeClass), spacing: 22) {
                            ForEach(value, id: \.ratingKey) { Card($0, shape: .square) }
                        }
                        .padding(.horizontal, Theme.margin(sizeClass))
                    }
                } else if let error = lists.error {
                    StatusView(error: error) { lists.reload() }
                } else {
                    LazyVGrid(columns: cardColumns(.square, sizeClass), spacing: 22) {
                        ForEach(0..<8, id: \.self) { _ in SkeletonCard(shape: .square) }
                    }
                    .padding(.horizontal, Theme.margin(sizeClass))
                }
            }
        }
        .navigationTitle("Playlists")
        .refreshable { await lists.refresh() }
        .task(id: store.revision) {
            await lists.load(at: store.revision) { try await PendingCommands.playlists(store.app) }
        }
    }
}

struct PlaylistScreen: View {
    let id: String

    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var data = Loader<PendingCommands.PlaylistView?>()
    /// Local order, changed optimistically while a move is saved.
    @State private var order: [Metadata] = []
    @State private var editMode = EditMode.inactive
    @State private var editing: (title: String, summary: String)?

    var body: some View {
        Group {
            if let view = data.value {
                if let view {
                    list(view)
                } else {
                    StatusView(title: "Playlist not found", systemImage: "music.note.list")
                }
            } else if let error = data.error {
                StatusView(error: error) { data.reload() }
            } else {
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .navigationTitle(data.value??.playlist.title ?? "Playlist")
        .navigationBarTitleDisplayMode(.inline)
        .environment(\.editMode, $editMode)
        .toolbar {
            if let view = data.value ?? nil {
                ToolbarItemGroup(placement: .topBarTrailing) {
                    if !view.playlist.smart && order.count > 1 {
                        Button(editMode.isEditing ? "Done" : "Reorder") {
                            withAnimation { editMode = editMode.isEditing ? .inactive : .active }
                        }
                    }
                    menu(view)
                }
            }
        }
        .sheet(isPresented: Binding(get: { editing != nil }, set: { if !$0 { editing = nil } })) {
            if let editing { PlaylistEditSheet(id: id, title: editing.title, summary: editing.summary) { data.reload() } }
        }
        .refreshable { await data.refresh() }
        .task(id: store.revision) {
            await data.load(at: store.revision) { try await PendingCommands.playlist(store.app, id: id) }
            order = data.value??.items ?? []
        }
    }

    private func list(_ view: PendingCommands.PlaylistView) -> some View {
        List {
            Section {
                header(view)
                    .listRowInsets(EdgeInsets())
                    .listRowBackground(Color.clear)
                    .listRowSeparator(.hidden)
            }
            Section {
                if order.isEmpty {
                    Text("This playlist is empty.").foregroundStyle(Theme.secondaryText)
                }
                let smart = view.playlist.smart
                ForEach(Array(order.enumerated()), id: \.element.playlistItemID) { i, item in
                    row(i, item, smart: smart)
                }
                .onMove(perform: smart ? nil : { (from: IndexSet, to: Int) in move(from, to) })
            }
        }
        .listStyle(.plain)
        .scrollContentBackground(.hidden)
    }

    private func header(_ view: PendingCommands.PlaylistView) -> some View {
        let p = view.playlist
        let total = view.items.reduce(0) { $0 + ($1.duration ?? 0) }
        let cutOff = view.total > view.items.count
        let regular = sizeClass == .regular
        let art = Artwork(p.composite ?? p.thumb, width: 480, height: 480)
            .frame(width: regular ? 220 : 180, height: regular ? 220 : 180)
            .clipShape(.rect(cornerRadius: 16, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(Theme.hairline))
            .shadow(color: .black.opacity(0.4), radius: 20, y: 10)
        let text = VStack(alignment: regular ? .leading : .center, spacing: 8) {
            Text(p.smart ? "SMART PLAYLIST" : "PLAYLIST")
                .font(.caption.weight(.bold)).kerning(1.1).foregroundStyle(store.accent)
            Text(Metadata.title(p)).font(.largeTitle.bold()).multilineTextAlignment(regular ? .leading : .center)
            Text(
                "\(cutOff ? "Showing the first \(Format.count(view.items.count)) of \(Format.count(view.total)) items" : Format.plural(view.items.count, "item")) · \(Format.duration(ms: Int64(total)))"
            )
            .font(.subheadline)
            .foregroundStyle(Theme.secondaryText)
            if let summary = p.summary, !summary.isEmpty {
                Text(summary).font(.callout).foregroundStyle(Theme.secondaryText).lineLimit(4)
                    .multilineTextAlignment(regular ? .leading : .center)
            }
            HStack(spacing: 10) {
                Button {
                    store.play(p)
                } label: {
                    Label("Play", systemImage: "play.fill").font(.headline).frame(minWidth: 110, minHeight: 34)
                }
                .buttonStyle(.borderedProminent)
                .foregroundStyle(.black)
                Button {
                    store.play(p, .init(shuffle: true))
                } label: {
                    Label("Shuffle", systemImage: "shuffle").font(.headline).frame(minWidth: 110, minHeight: 34)
                }
                .buttonStyle(.bordered)
            }
            .disabled(view.items.isEmpty)
            .padding(.top, 6)
        }
        return Group {
            if regular {
                HStack(alignment: .bottom, spacing: 28) {
                    art
                    text
                    Spacer(minLength: 0)
                }
            } else {
                VStack(spacing: 16) {
                    art
                    text
                }
                .frame(maxWidth: .infinity)
            }
        }
        .padding(.horizontal, Theme.margin(sizeClass))
        .padding(.vertical, 20)
    }

    private func row(_ i: Int, _ item: Metadata, smart: Bool) -> some View {
        NavigationLink(value: PlexUI.route(item)) {
            HStack(spacing: 12) {
                Text("\(i + 1)")
                    .font(.footnote.monospacedDigit())
                    .foregroundStyle(Theme.tertiaryText)
                    .frame(minWidth: 24, alignment: .trailing)
                Artwork(Metadata.poster(item), width: 96, height: 144)
                    .frame(width: 36, height: PlexUI.shape(item) == .square ? 36 : 54)
                    .clipShape(.rect(cornerRadius: 5, style: .continuous))
                VStack(alignment: .leading, spacing: 2) {
                    Text(Metadata.fullTitle(item)).font(.subheadline.weight(.medium)).lineLimit(1)
                    Text(PlexUI.subtitle(item)).font(.caption).foregroundStyle(Theme.secondaryText).lineLimit(1)
                }
                Spacer()
                if let d = item.duration {
                    Text(Format.duration(ms: Int64(d))).font(.caption.monospacedDigit()).foregroundStyle(Theme.secondaryText)
                }
            }
        }
        .listRowBackground(Color.clear)
        .contextMenu {
            ItemMenu(item: item)
            if !smart, let entry = item.playlistItemID {
                Button("Remove from Playlist", systemImage: "minus.circle", role: .destructive) { remove(entry) }
            }
        }
        .swipeActions(edge: .trailing) {
            if !smart, let entry = item.playlistItemID {
                Button("Remove", systemImage: "minus.circle", role: .destructive) { remove(entry) }
            }
        }
        .swipeActions(edge: .leading) {
            Button("Play Next", systemImage: "text.line.first.and.arrowtriangle.forward") { store.enqueue(item, next: true) }
                .tint(store.accent)
        }
    }

    private func menu(_ view: PendingCommands.PlaylistView) -> some View {
        Menu {
            Button("Edit Details…", systemImage: "pencil") {
                editing = (view.playlist.title ?? "", view.playlist.summary ?? "")
            }
            if !view.playlist.smart && !view.items.isEmpty {
                Button("Clear Playlist…", systemImage: "eraser", role: .destructive) { clear() }
            }
            Button("Delete Playlist…", systemImage: "trash", role: .destructive) { delete() }
        } label: {
            Image(systemName: "ellipsis.circle")
        }
    }

    private func remove(_ entry: Int) {
        withAnimation { order.removeAll { $0.playlistItemID == entry } }
        Haptics.tap()
        // The core announces it with a toast that offers Undo.
        store.perform(refresh: true) { try await PendingCommands.removeFromPlaylist(store.app, id: id, itemId: entry) }
    }

    private func move(_ from: IndexSet, _ to: Int) {
        guard let source = from.first else { return }
        var next = order
        next.move(fromOffsets: from, toOffset: to)
        let at = source < to ? to - 1 : to
        guard at != source, let itemId = order[source].playlistItemID else { return }
        order = next
        let after = at > 0 ? next[at - 1].playlistItemID : nil
        Haptics.selection()
        Task {
            _ = await store.act(nil, {
                try await PendingCommands.movePlaylistItem(store.app, id: id, itemId: itemId, after: after)
            })
            await data.refresh()
            order = data.value??.items ?? order
        }
    }

    private func clear() {
        Task {
            let ok = await store.confirm(
                "Clear this playlist?", text: "Every item is removed; the playlist itself stays.", action: "Clear",
                danger: true)
            if ok, await store.act("Playlist cleared", { try await PendingCommands.clearPlaylist(store.app, id: id) }) {
                await data.refresh()
                order = data.value??.items ?? []
            }
        }
    }

    private func delete() {
        Task {
            let ok = await store.confirm(
                "Delete this playlist?", text: "The playlist is removed from the server. Its items are not affected.",
                action: "Delete", danger: true)
            if ok, await store.act("Playlist deleted", refresh: true, { try await PendingCommands.deletePlaylist(store.app, id: id) }) {
                if store.paths[store.selection]?.last == .playlist(id: id) { store.paths[store.selection]?.removeLast() }
            }
        }
    }
}

/// Rename a playlist and change its description.
struct PlaylistEditSheet: View {
    let id: String
    @State var title: String
    @State var summary: String
    let saved: () -> Void

    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Form {
                Section("Title") { TextField("Title", text: $title) }
                Section("Description") { TextEditor(text: $summary).frame(minHeight: 120) }
            }
            .navigationTitle("Edit Playlist")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        let (t, s) = (title, summary)
                        dismiss()
                        Task {
                            if await store.act("Playlist updated", { try await PendingCommands.updatePlaylist(store.app, id: id, title: t, summary: s) }) {
                                saved()
                            }
                        }
                    }
                    .disabled(title.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
        }
        .environment(store)
        .preferredColorScheme(.dark)
    }
}
