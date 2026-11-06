// Ports of web/src/components/PlaylistDialog.svelte and CollectionDialog.svelte: add an
// item (or a selection) to a playlist or collection, or to a new one.

import PatinaCore
import PlexKit
import SwiftUI

struct PlaylistSheet: View {
    let item: Metadata
    /// A selection added together; empty for `item` alone.
    var items: [Metadata] = []

    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var lists: [Metadata]?
    @State private var error: String?
    @State private var name = ""
    @State private var scope = ""

    var body: some View {
        NavigationStack {
            List {
                Section {
                    HStack {
                        TextField("New Playlist", text: $name)
                            .submitLabel(.done)
                            .onSubmit(create)
                        Button("Create", action: create)
                            .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
                    }
                }
                Section("Playlists") {
                    if let error {
                        Text(error).foregroundStyle(Theme.danger).textSelection(.enabled)
                    } else if let lists {
                        if lists.isEmpty {
                            Text("No playlists of this kind yet.").foregroundStyle(Theme.secondaryText)
                        }
                        ForEach(lists, id: \.ratingKey) { p in
                            Button {
                                add(playlist: p.ratingKey, title: nil, ok: "Added to “\(Metadata.title(p))”")
                            } label: {
                                HStack(spacing: 12) {
                                    Artwork(p.composite ?? p.thumb, width: 96, height: 96)
                                        .frame(width: 44, height: 44)
                                        .clipShape(.rect(cornerRadius: 6, style: .continuous))
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(Metadata.title(p)).foregroundStyle(.primary)
                                        Text(PlexUI.subtitle(p)).font(.caption).foregroundStyle(Theme.secondaryText)
                                    }
                                    Spacer()
                                    Image(systemName: "plus.circle").foregroundStyle(store.accent)
                                }
                            }
                        }
                    } else {
                        ProgressView().frame(maxWidth: .infinity)
                    }
                }
            }
            .navigationTitle("Add to Playlist")
            .navigationBarTitleDisplayMode(.inline)
            .safeAreaInset(edge: .top) {
                Text(items.isEmpty ? Metadata.fullTitle(item) : "\(items.count) selected titles")
                    .font(.footnote)
                    .foregroundStyle(Theme.secondaryText)
                    .lineLimit(1)
                    .padding(.horizontal)
            }
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
            }
            .task {
                scope = store.scope
                do {
                    lists = try await PendingCommands.playlists(store.app, kind: PlexUI.playlistKind(item))
                } catch {
                    self.error = errorText(error)
                }
            }
        }
    }

    private func create() {
        let title = name.trimmingCharacters(in: .whitespacesAndNewlines)
        if !title.isEmpty { add(playlist: nil, title: title, ok: "Created playlist “\(title)”") }
    }

    private func add(playlist: String?, title: String?, ok: String) {
        dismiss()
        // The rating keys belong to the server the dialog opened on.
        guard scope == store.scope else { return }
        let keys = items.isEmpty ? Metadata.ratingKey(item) : items.map(Metadata.ratingKey).joined(separator: ",")
        let kind = PlexUI.playlistKind(item)
        Haptics.success()
        store.perform(ok) {
            try await PendingCommands.addToPlaylist(store.app, ratingKey: keys, kind: kind, playlist: playlist, title: title)
        }
    }
}

struct CollectionSheet: View {
    let item: Metadata
    /// A selection added together; empty for `item` alone.
    var items: [Metadata] = []

    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var lists: [Metadata]?
    /// Titles of collections the item is already in.
    @State private var member: [String] = []
    @State private var error: String?
    @State private var name = ""
    @State private var scope = ""

    var body: some View {
        NavigationStack {
            List {
                Section {
                    HStack {
                        TextField("New Collection", text: $name)
                            .submitLabel(.done)
                            .onSubmit(create)
                        Button("Create", action: create)
                            .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
                    }
                }
                Section("Collections") {
                    if let error {
                        Text(error).foregroundStyle(Theme.danger).textSelection(.enabled)
                    } else if let lists {
                        if lists.isEmpty {
                            Text("This library has no collections yet.").foregroundStyle(Theme.secondaryText)
                        }
                        ForEach(lists, id: \.ratingKey) { c in
                            let has = items.isEmpty && member.contains(Metadata.title(c))
                            Button {
                                add(collection: c.ratingKey, title: nil, ok: "Added to “\(Metadata.title(c))”")
                            } label: {
                                HStack(spacing: 12) {
                                    Artwork(c.thumb ?? c.composite, width: 96, height: 144)
                                        .frame(width: 32, height: 48)
                                        .clipShape(.rect(cornerRadius: 5, style: .continuous))
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(Metadata.title(c)).foregroundStyle(.primary)
                                        Text(has ? "Already in this collection" : PlexUI.subtitle(c))
                                            .font(.caption).foregroundStyle(Theme.secondaryText)
                                    }
                                    Spacer()
                                    Image(systemName: has ? "checkmark" : "plus.circle")
                                        .foregroundStyle(has ? Theme.secondaryText : store.accent)
                                }
                            }
                            .disabled(has)
                        }
                    } else {
                        ProgressView().frame(maxWidth: .infinity)
                    }
                }
            }
            .navigationTitle("Add to Collection")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
            }
            .task {
                scope = store.scope
                do {
                    // Menu items may be partial; the full item knows its section and collections.
                    let full =
                        item.librarySectionID != nil && !item.collection.isEmpty
                        ? item : try await Commands.metadata(store.app, ratingKey: Metadata.ratingKey(item))
                    guard let section = full?.librarySectionID ?? item.librarySectionID else {
                        throw CoreError("This item isn't in a library.")
                    }
                    lists = try await Commands.collections(store.app, id: String(section))
                    member = (full?.collection ?? []).map(Tag.name)
                } catch {
                    self.error = errorText(error)
                }
            }
        }
    }

    private func create() {
        let title = name.trimmingCharacters(in: .whitespacesAndNewlines)
        if !title.isEmpty { add(collection: nil, title: title, ok: "Created collection “\(title)”") }
    }

    private func add(collection: String?, title: String?, ok: String) {
        dismiss()
        guard scope == store.scope else { return }
        let targets = items.isEmpty ? [item] : items
        Haptics.success()
        store.perform(ok, refresh: true) {
            var into = collection
            for (i, t) in targets.enumerated() {
                // A new collection is created by the first item; the rest join it by title.
                if i == 0 || into != nil {
                    try await PendingCommands.addToCollection(
                        store.app, ratingKey: Metadata.ratingKey(t), collection: into, title: into == nil ? title : nil)
                }
                if into == nil, let title, let section = t.librarySectionID {
                    into = try await Commands.collections(store.app, id: String(section))
                        .first { Metadata.title($0) == title }?.ratingKey
                }
            }
        }
    }
}
