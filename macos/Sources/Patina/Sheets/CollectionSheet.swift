import SwiftUI

/// "Add to Collection": an existing collection in the item's library, or a new one.
struct CollectionSheet: View {
    let item: Metadata
    @Environment(AppModel.self) private var app
    @Environment(\.dismiss) private var dismiss
    @State private var phase: Phase<(lists: [Metadata], member: Set<String>)> = .loading
    @State private var name = ""
    /// Set on the first add, so a double click or Return doesn't add twice while the sheet closes.
    @State private var submitted = false

    var body: some View {
        SheetFrame(title: "Add to Collection", subtitle: item.fullTitle, width: 440, height: 460) {
            switch phase {
            case .loading: SheetMessage(text: "", spinner: true)
            case .failed(let e): SheetMessage(text: e, error: true)
            case .loaded(let r) where r.lists.isEmpty:
                SheetMessage(text: "This library has no collections yet.")
            case .loaded(let r):
                List(r.lists) { c in
                    let has = r.member.contains(c.name)
                    Button { add(collection: c.ratingKey, title: nil, ok: "Added to “\(c.name)”") } label: {
                        SheetRow(
                            thumb: c.thumb ?? c.composite, shape: .poster, title: c.name,
                            subtitle: has ? "Already in this collection" : c.subtitle, thumbWidth: 28
                        ) {
                            Image(systemName: has ? "checkmark" : "plus")
                                .foregroundStyle(has ? AnyShapeStyle(app.accent) : AnyShapeStyle(.secondary))
                        }
                    }
                    .buttonStyle(.plain)
                    .disabled(has)
                }
                .listStyle(.inset)
            }
        } footer: {
            TextField("New collection", text: $name)
                .textFieldStyle(.roundedBorder)
                .onSubmit(create)
            Button("Create", action: create)
                .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
                .keyboardShortcut(.defaultAction)
            Button("Cancel", role: .cancel) { dismiss() }
                .keyboardShortcut(.cancelAction)
        }
        .task { await load() }
    }

    private func load() async {
        do {
            // Menu items may be partial; the full item knows its section and collections.
            let full = item.librarySectionID != nil && !item.collections.isEmpty ? item : try await API.metadata(item.ratingKey ?? "")
            guard let section = full?.librarySectionID ?? item.librarySectionID else {
                phase = .failed("This item isn't in a library.")
                return
            }
            let lists = try await API.collections(String(section))
            phase = .loaded((lists, Set((full?.collections ?? []).compactMap(\.tag))))
        } catch {
            phase = .failed(error.localizedDescription)
        }
    }

    private func create() {
        let title = name.trimmingCharacters(in: .whitespaces)
        guard !title.isEmpty else { return }
        add(collection: nil, title: title, ok: "Created collection “\(title)”")
    }

    private func add(collection: String?, title: String?, ok: String) {
        guard !submitted else { return }
        submitted = true
        let rk = item.ratingKey ?? ""
        dismiss()
        app.perform(ok, refresh: true) { try await API.addToCollection(rk, collection: collection, title: title) }
    }
}
