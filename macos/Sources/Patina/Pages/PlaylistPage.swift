import SwiftUI

/// One playlist: header, play/shuffle, and its items, which can be reordered
/// by dragging and removed (web/src/pages/Playlist.svelte).
struct PlaylistPage: View {
    let id: String
    @Environment(AppModel.self) private var app

    var body: some View {
        Loading(id: [AnyHashable(id), AnyHashable(app.revision)], { try await API.playlist(id) }) { view in
            if let view {
                PlaylistDetail(id: id, playlist: view.playlist, items: view.items)
            } else {
                EmptyState(title: "Playlist not found")
            }
        }
    }
}

struct PlaylistDetail: View {
    let id: String
    let playlist: Metadata
    let items: [Metadata]
    @Environment(AppModel.self) private var app
    /// Optimistic order while a move or remove is saved.
    @State private var local: [Metadata]?
    @State private var selection = Set<Int>()
    @State private var editing = false

    private var order: [Metadata] { local ?? items }
    private var smart: Bool { playlist.smart }

    var body: some View {
        List(selection: $selection) {
            PlaylistHeader(playlist: playlist, items: items, editing: $editing, clear: clear, delete: delete)
                .listRowSeparator(.hidden)
                .listRowInsets(EdgeInsets(top: 0, leading: 0, bottom: 0, trailing: 0))
                .selectionDisabled()

            if order.isEmpty {
                EmptyState(title: "This playlist is empty", message: smart ? nil : "Right-click anything and choose Add to Playlist….")
                    .listRowSeparator(.hidden)
                    .selectionDisabled()
            } else {
                ForEach(Array(order.enumerated()), id: \.element.rowId) { i, item in
                    PlaylistRow(number: i + 1, item: item, removable: !smart && item.playlistItemID != nil) { remove([item]) }
                        .tag(item.rowId)
                        .listRowSeparatorTint(Theme.hairline)
                        .listRowInsets(EdgeInsets(top: 0, leading: Theme.gutter - 8, bottom: 0, trailing: Theme.gutter - 8))
                }
                .onMove(perform: smart ? nil : move)
            }
        }
        .listStyle(.plain)
        .scrollContentBackground(.hidden)
        .contextMenu(forSelectionType: Int.self) { ids in
            let picked = order.filter { ids.contains($0.rowId) }
            if picked.count == 1, let m = picked.first {
                ItemMenu(item: m)
            }
            if !smart && !picked.isEmpty {
                Divider()
                Button(picked.count == 1 ? "Remove from Playlist" : "Remove \(picked.count) Items from Playlist", systemImage: "minus.circle") {
                    remove(picked)
                }
            }
        } primaryAction: { ids in
            if let m = order.first(where: { ids.contains($0.rowId) }) { app.open(m) }
        }
        .onDeleteCommand {
            guard !smart else { return }
            remove(order.filter { selection.contains($0.rowId) })
        }
        .onChange(of: items) { _, _ in local = nil }
        .navigationTitle(playlist.name)
        .sheet(isPresented: $editing) {
            PlaylistEditSheet(id: id, title: playlist.title ?? "", summary: playlist.summary ?? "")
        }
    }

    private func move(_ from: IndexSet, _ to: Int) {
        var next = order
        next.move(fromOffsets: from, toOffset: to)
        guard let first = from.first, let moved = order[first].playlistItemID,
              let at = next.firstIndex(where: { $0.playlistItemID == moved }) else { return }
        local = next
        let after = at > 0 ? next[at - 1].playlistItemID : nil
        let pid = id
        Task {
            // Put the server's order back if the move didn't save.
            if !(await app.act(refresh: true) { try await API.movePlaylistItem(pid, itemId: moved, after: after) }) { local = nil }
        }
    }

    private func remove(_ picked: [Metadata]) {
        let ids = picked.compactMap(\.playlistItemID)
        guard !ids.isEmpty else { return }
        local = order.filter { !ids.contains($0.playlistItemID ?? -1) }
        selection.subtract(ids)
        let pid = id
        Task {
            let ok = await app.act(ids.count > 1 ? "Removed \(ids.count) items" : nil, refresh: true) {
                for itemId in ids { try await API.removeFromPlaylist(pid, itemId: itemId) }
            }
            // Some may have been removed before the failure; refetch to show what's left.
            if !ok { local = nil; app.refresh() }
        }
    }

    private func clear() {
        let pid = id
        app.confirm("Clear this playlist?", message: "Every item is removed; the playlist itself stays.", action: "Clear", destructive: true) {
            await app.act("Playlist cleared", refresh: true) { try await API.clearPlaylist(pid) }
        }
    }

    private func delete() {
        let pid = id
        app.confirm("Delete this playlist?", message: "The playlist is removed from the server. Its items are not affected.",
                    action: "Delete", destructive: true) {
            if await app.act("Playlist deleted", refresh: true, { try await API.deletePlaylist(pid) }) { popPage() }
        }
    }
}

private extension Metadata {
    /// Stable row identity within a playlist.
    var rowId: Int { playlistItemID ?? id.hashValue }
}
