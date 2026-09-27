import SwiftUI

/// "Add to Playlist": an existing playlist of the item's kind, or a new one.
struct PlaylistSheet: View {
    let item: Metadata
    @Environment(AppModel.self) private var app
    @Environment(\.dismiss) private var dismiss
    @State private var phase: Phase<[Metadata]> = .loading
    @State private var name = ""
    /// Set on the first add, so a double click or Return doesn't add twice while the sheet closes.
    @State private var submitted = false

    var body: some View {
        SheetFrame(title: "Add to Playlist", subtitle: item.fullTitle, width: 440, height: 460) {
            switch phase {
            case .loading: SheetMessage(text: "", spinner: true)
            case .failed(let e): SheetMessage(text: e, error: true)
            case .loaded(let lists) where lists.isEmpty:
                SheetMessage(text: "No \(kindName) playlists yet.")
            case .loaded(let lists):
                List(lists) { p in
                    Button { add(playlist: p.ratingKey, title: nil, ok: "Added to “\(p.name)”") } label: {
                        SheetRow(thumb: p.composite ?? p.thumb, title: p.name, subtitle: p.subtitle) {
                            Image(systemName: "plus").foregroundStyle(.secondary)
                        }
                    }
                    .buttonStyle(.plain)
                }
                .listStyle(.inset)
            }
        } footer: {
            TextField("New playlist", text: $name)
                .textFieldStyle(.roundedBorder)
                .onSubmit(create)
            Button("Create", action: create)
                .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
                .keyboardShortcut(.defaultAction)
            Button("Cancel", role: .cancel) { dismiss() }
                .keyboardShortcut(.cancelAction)
        }
        .task {
            do { phase = .loaded(try await API.playlists(kind: item.playlistKind)) } catch { phase = .failed(error.localizedDescription) }
        }
    }

    private var kindName: String {
        switch item.playlistKind {
        case "audio": "music"
        case "photo": "photo"
        default: "video"
        }
    }

    private func create() {
        let title = name.trimmingCharacters(in: .whitespaces)
        guard !title.isEmpty else { return }
        add(playlist: nil, title: title, ok: "Created playlist “\(title)”")
    }

    private func add(playlist: String?, title: String?, ok: String) {
        guard !submitted else { return }
        submitted = true
        let rk = item.ratingKey ?? "", kind = item.playlistKind
        dismiss()
        app.perform(ok, refresh: true) { try await API.addToPlaylist(rk, kind: kind, playlist: playlist, title: title) }
    }
}
