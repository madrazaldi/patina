import SwiftUI
import UniformTypeIdentifiers

/// Collection display mode, sort order and delete, for server owners.
struct CollectionTools: View {
    let item: Metadata
    @Environment(AppModel.self) private var app

    var body: some View {
        let id = item.ratingKey ?? ""
        HStack(alignment: .bottom, spacing: 16) {
            VStack(alignment: .leading, spacing: 5) {
                Eyebrow("Show in Library")
                Picker("Show in Library", selection: Binding(
                    get: { item.collectionMode ?? -1 },
                    set: { mode in app.perform("Collection updated", refresh: true) { try await API.collectionSettings(id, mode: mode) } }
                )) {
                    Text("Library Default").tag(-1)
                    Text("Hide Collection").tag(0)
                    Text("Hide Items").tag(1)
                    Text("Show Both").tag(2)
                }
            }
            .frame(width: 190)
            VStack(alignment: .leading, spacing: 5) {
                Eyebrow("Sort")
                Picker("Sort", selection: Binding(
                    get: { item.collectionSort ?? 0 },
                    set: { sort in app.perform("Collection updated", refresh: true) { try await API.collectionSettings(id, sort: sort) } }
                )) {
                    Text("Release Date").tag(0)
                    Text("Title").tag(1)
                    Text("Custom").tag(2)
                }
            }
            .frame(width: 160)
            Spacer()
            Button("Delete Collection…", systemImage: "trash", role: .destructive) { Actions.delete(item, then: popPage) }
                .buttonStyle(.borderless)
                .foregroundStyle(.red)
        }
        .labelsHidden()
    }
}

/// A collection's items as a grid. Owners can remove items, and reorder them
/// by dragging when the collection is custom-sorted.
struct CollectionItems: View {
    let collection: Metadata
    let items: [Metadata]
    @Environment(AppModel.self) private var app
    /// Optimistic order after a remove or move, until the refetch lands.
    @State private var local: [Metadata]?
    @State private var target: String?
    @State private var hovered: String?

    private var shown: [Metadata] { local ?? items }
    private var editable: Bool { app.owned }
    private var reorderable: Bool { editable && collection.collectionSort == 2 && shown.count > 1 }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .firstTextBaseline) {
                Text(Fmt.plural(shown.count, "item")).font(Theme.sectionTitle)
                if reorderable { Text("Drag to reorder").font(Theme.caption).foregroundStyle(.tertiary) }
            }
            .padding(.horizontal, Theme.gutter)
            LazyVGrid(columns: [GridItem(.adaptive(minimum: app.posterWidth, maximum: app.posterWidth * 1.4), spacing: 18, alignment: .top)],
                      alignment: .leading, spacing: 22) {
                ForEach(shown, id: \.id) { m in cell(m) }
            }
            .padding(.horizontal, Theme.gutter)
        }
        .onChange(of: items) { _, _ in local = nil }
    }

    @ViewBuilder private func cell(_ m: Metadata) -> some View {
        let rk = m.ratingKey ?? ""
        let base = Card(item: m)
            .overlay(alignment: .leading) {
                if target == rk {
                    Rectangle().fill(app.accent).frame(width: 2).offset(x: -10)
                }
            }
            .overlay(alignment: .topTrailing) {
                if editable && hovered == rk { RemoveButton { remove(m) }.padding(6) }
            }
            .onHover { on in hovered = on ? rk : (hovered == rk ? nil : hovered) }
            .contextMenu {
                ItemMenu(item: m)
                if editable {
                    Divider()
                    Button("Remove from Collection", systemImage: "minus.circle") { remove(m) }
                }
            }
        if reorderable {
            base
                .draggable(rk) { Card(item: m, width: 90).opacity(0.8) }
                .dropDestination(for: String.self) { dropped, _ in
                    target = nil
                    guard let from = dropped.first else { return false }
                    return move(from, before: rk)
                } isTargeted: { on in
                    if on { target = rk } else if target == rk { target = nil }
                }
        } else {
            base
        }
    }

    private func remove(_ m: Metadata) {
        let rk = m.ratingKey ?? "", id = collection.ratingKey ?? ""
        local = shown.filter { $0.ratingKey != rk }
        Task {
            if !(await app.act("Removed from collection", refresh: true) { try await API.removeFromCollection(id, ratingKey: rk) }) { local = nil }
        }
    }

    /// Move `rk` before `before`; Plex wants the item it lands after.
    private func move(_ rk: String, before: String) -> Bool {
        guard rk != before, let dragged = shown.first(where: { $0.ratingKey == rk }) else { return false }
        var rest = shown.filter { $0.ratingKey != rk }
        guard let at = rest.firstIndex(where: { $0.ratingKey == before }) else { return false }
        let after = at > 0 ? rest[at - 1].ratingKey : nil
        rest.insert(dragged, at: at)
        local = rest
        let id = collection.ratingKey ?? ""
        Task {
            if !(await app.act(refresh: true) { try await API.moveCollectionItem(id, ratingKey: rk, after: after) }) { local = nil }
        }
        return true
    }
}

/// A small × that appears over a card on hover.
private struct RemoveButton: View {
    let action: () -> Void
    @State private var hovering = false

    var body: some View {
        Button(action: action) {
            Image(systemName: "xmark")
                .font(.system(size: 9, weight: .bold))
                .foregroundStyle(.white)
                .frame(width: 20, height: 20)
                .background(RoundedRectangle(cornerRadius: 3).fill(.black.opacity(hovering ? 0.85 : 0.6)))
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .help("Remove from Collection")
    }
}
