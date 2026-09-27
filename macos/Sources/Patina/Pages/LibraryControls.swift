import SwiftUI

/// The quiet bar under the library title: media type, sort, filters,
/// Unwatched, saved views and selection.
struct LibraryFilterBar: View {
    @Bindable var model: LibraryModel
    /// Scroll back to the top, e.g. after applying a saved view.
    var toTop: () -> Void

    @Environment(AppModel.self) private var app
    @State private var filtering = false

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 12) {
                if model.types.count > 1 {
                    Picker("Show", selection: Binding(get: { model.mediaType ?? 0 }, set: { model.setType($0) })) {
                        ForEach(model.types, id: \.0) { t in Text(t.1).tag(t.0) }
                    }
                    .pickerStyle(.segmented)
                    .labelsHidden()
                    .fixedSize()
                    divider
                }
                sortMenu
                if !model.valueFilters.isEmpty || !model.booleans.isEmpty {
                    Button {
                        filtering.toggle()
                    } label: {
                        Label(model.activeFilterCount > 0 ? "Filter (\(model.activeFilterCount))" : "Filter",
                              systemImage: "line.3.horizontal.decrease.circle")
                    }
                    .buttonStyle(.borderless)
                    .popover(isPresented: $filtering, arrowEdge: .bottom) {
                        LibraryFilterPopover(model: model)
                    }
                    .help("Filter by genre, year and more")
                }
                if model.watchable {
                    Toggle("Unwatched", isOn: $model.unwatched)
                        .toggleStyle(.button)
                        .help("Show only unwatched titles")
                }
                Spacer(minLength: 12)
                if let total = model.pages.total {
                    Text(Fmt.plural(total, model.noun))
                        .font(Theme.caption.monospacedDigit())
                        .foregroundStyle(.secondary)
                    if model.pages.loading > 0 { ProgressView().controlSize(.mini) }
                }
                SavedViewsMenu(model: model, toTop: toTop)
                Toggle(isOn: Binding(get: { model.selecting }, set: { on in
                    model.selecting = on
                    if !on { model.clearSelection() }
                })) {
                    Label("Select", systemImage: "checkmark.circle")
                }
                .toggleStyle(.button)
                .help("Select titles for bulk actions")
            }
            .controlSize(.small)
            if !model.toggles.isEmpty || !model.picked.isEmpty {
                activeFilters
            }
        }
        .padding(.horizontal, Theme.gutter)
        .padding(.bottom, 12)
        .overlay(alignment: .bottom) { Hairline().padding(.horizontal, Theme.gutter) }
        .padding(.bottom, 22)
    }

    private var divider: some View {
        Rectangle().fill(Theme.hairline).frame(width: 1, height: 16)
    }

    private var sortMenu: some View {
        Menu {
            Picker("Sort By", selection: $model.sort) {
                ForEach(model.sortOptions, id: \.0) { s in Text(s.1).tag(s.0) }
            }
            .pickerStyle(.inline)
            Divider()
            Picker("Order", selection: $model.desc) {
                Text("Ascending").tag(false)
                Text("Descending").tag(true)
            }
            .pickerStyle(.inline)
        } label: {
            Label(model.sortTitle, systemImage: model.desc ? "arrow.down" : "arrow.up")
                .labelStyle(.titleAndIcon)
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
        .help("Sort")
    }

    private var activeFilters: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                ForEach(model.toggles, id: \.self) { key in
                    FilterChip(title: nil, label: model.filters.first { $0.filter == key }?.title ?? key) {
                        model.toggles.removeAll { $0 == key }
                    }
                }
                ForEach(Array(model.picked.enumerated()), id: \.offset) { i, p in
                    FilterChip(title: p.title, label: p.label) { model.picked.remove(at: i) }
                }
                Button("Clear") { model.toggles = []; model.picked = [] }
                    .buttonStyle(.link)
                    .font(Theme.caption)
                    .padding(.leading, 4)
            }
        }
    }
}

/// An active filter; clicking removes it.
private struct FilterChip: View {
    let title: String?
    let label: String
    let remove: () -> Void

    var body: some View {
        Button(action: remove) {
            HStack(spacing: 4) {
                if let title { Text("\(title):").foregroundStyle(.secondary) }
                Text(label)
                Image(systemName: "xmark").font(.system(size: 8, weight: .semibold)).foregroundStyle(.secondary)
            }
            .font(Theme.caption)
            .padding(.horizontal, 8).padding(.vertical, 3)
            .overlay(Capsule().strokeBorder(Theme.hairline.opacity(2)))
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .help("Remove filter")
    }
}

/// Filter names, then the chosen filter's values; boolean filters as checkboxes.
private struct LibraryFilterPopover: View {
    @Bindable var model: LibraryModel
    @State private var choosing: FilterView?
    @State private var values: Phase<[Directory]> = .loading
    @State private var query = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let f = choosing { valueList(f) } else { filterList }
        }
        .frame(width: 270)
        .padding(.vertical, 8)
    }

    private var filterList: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(model.valueFilters) { f in
                row {
                    choosing = f
                    query = ""
                } label: {
                    Text(f.title)
                    Spacer()
                    let n = model.picked.filter { $0.filter == f.filter }.count
                    if n > 0 { Text("\(n)").font(Theme.caption.monospacedDigit()).foregroundStyle(.secondary) }
                    Image(systemName: "chevron.right").font(.system(size: 10, weight: .semibold)).foregroundStyle(.tertiary)
                }
            }
            if !model.booleans.isEmpty {
                if !model.valueFilters.isEmpty { Divider().padding(.vertical, 6) }
                Eyebrow("Show only").padding(.horizontal, 14).padding(.bottom, 4)
                ForEach(model.booleans) { f in
                    Toggle(f.title, isOn: Binding(get: { model.toggles.contains(f.filter) }, set: { _ in model.toggle(f) }))
                        .toggleStyle(.checkbox)
                        .padding(.horizontal, 14).padding(.vertical, 3)
                }
            }
        }
    }

    private func valueList(_ f: FilterView) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Button {
                choosing = nil
            } label: {
                Label(f.title, systemImage: "chevron.left").font(.system(size: 12, weight: .medium))
            }
            .buttonStyle(.borderless)
            .padding(.horizontal, 10).padding(.bottom, 6)
            Divider()
            switch values {
            case .loading:
                ProgressView().controlSize(.small).frame(maxWidth: .infinity, minHeight: 60)
            case .failed(let message):
                Text(message).font(Theme.caption).foregroundStyle(.secondary).padding(14)
            case .loaded(let all):
                if all.count > 12 {
                    TextField("Search \(f.title.lowercased())", text: $query)
                        .textFieldStyle(.roundedBorder)
                        .controlSize(.small)
                        .padding(.horizontal, 10).padding(.vertical, 6)
                }
                let q = query.trimmingCharacters(in: .whitespaces).lowercased()
                let shown = q.isEmpty ? all : all.filter { ($0.title ?? "").lowercased().contains(q) }
                if shown.isEmpty {
                    Text("Nothing to choose").font(Theme.caption).foregroundStyle(.secondary).padding(14)
                } else {
                    ScrollView {
                        LazyVStack(alignment: .leading, spacing: 0) {
                            ForEach(Array(shown.enumerated()), id: \.offset) { _, v in
                                if let key = v.key {
                                    let on = model.picked.contains { $0.filter == f.filter && $0.value == key }
                                    row {
                                        model.pick(f, value: key, label: v.title ?? key)
                                    } label: {
                                        Text(v.title ?? key).lineLimit(1)
                                        Spacer()
                                        if on { Image(systemName: "checkmark").font(.system(size: 11, weight: .semibold)) }
                                    }
                                }
                            }
                        }
                    }
                    .frame(maxHeight: 320)
                    .fixedSize(horizontal: false, vertical: shown.count < 14)
                }
            }
        }
        .task(id: f.filter) {
            values = .loading
            do { values = .loaded(try await API.sectionFilterValues(model.id, filter: f.filter, kind: model.mediaType)) } catch is CancellationError {
            } catch { values = .failed(error.localizedDescription) }
        }
    }

    private func row<L: View>(_ action: @escaping () -> Void, @ViewBuilder label: () -> L) -> some View {
        Button(action: action) {
            HStack(spacing: 6) { label() }
                .font(.system(size: 12.5))
                .padding(.horizontal, 14).padding(.vertical, 5)
                .contentShape(Rectangle())
        }
        .buttonStyle(HoverRowStyle())
    }
}

/// A plain row that highlights under the pointer, like a menu item.
private struct HoverRowStyle: ButtonStyle {
    @State private var hovering = false
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .background(hovering || configuration.isPressed ? Color.primary.opacity(0.07) : .clear)
            .onHover { hovering = $0 }
    }
}

/// Named sort/filter combinations, per library and account/server.
private struct SavedViewsMenu: View {
    let model: LibraryModel
    var toTop: () -> Void
    @Environment(AppModel.self) private var app
    @State private var saving = false
    @State private var name = ""

    var body: some View {
        let views = model.savedViews(app.settings)
        Menu {
            if views.isEmpty {
                Text("No saved views")
            } else {
                ForEach(views) { v in
                    Button(v.name) { model.apply(v); toTop() }
                }
            }
            Divider()
            Button("Save Current View…") { name = ""; saving = true }
            if !views.isEmpty {
                Menu("Delete View") {
                    ForEach(views) { v in
                        Button(v.name, role: .destructive) { remove(v) }
                    }
                }
            }
        } label: {
            Label("Views", systemImage: "bookmark")
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
        .help("Saved views")
        .alert("Save View", isPresented: $saving) {
            TextField("Name", text: $name)
            Button("Save") { save() }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Saves the current type, sort and filters for this library.")
        }
    }

    private func save() {
        let trimmed = name.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return }
        let all = (app.settings?.ui.saved_views ?? []) + [model.makeView(named: trimmed)]
        app.updateUi(["saved_views": JSON.from(all)]) { $0.saved_views = all }
        app.toast("Saved “\(trimmed)”")
    }

    private func remove(_ view: SavedView) {
        let all = (app.settings?.ui.saved_views ?? []).filter { $0.id != view.id }
        app.updateUi(["saved_views": JSON.from(all)]) { $0.saved_views = all }
    }
}

/// Actions on the selected titles (web/src/components/BulkActions.svelte).
struct LibraryBulkBar: View {
    let model: LibraryModel
    @Environment(AppModel.self) private var app
    @State private var busy = false
    @State private var done = 0
    @State private var playlists: [Metadata] = []
    @State private var naming = false
    @State private var playlistName = ""

    private enum Action { case queue, download, watched, unwatched }

    private var items: [Metadata] { model.selectedItems }

    /// The playlist kind every selected item shares, if any.
    private var playlistKind: String? {
        let kinds = Set(items.map(\.playlistKind))
        return kinds.count == 1 ? kinds.first : nil
    }

    var body: some View {
        let empty = items.isEmpty
        HStack(spacing: 10) {
            Text(busy ? "\(items.count) selected · \(done) done" : "\(items.count) selected")
                .font(Theme.caption.monospacedDigit())
                .foregroundStyle(.secondary)
                .frame(minWidth: 80, alignment: .leading)
            Group {
                Button("Add to Queue", systemImage: "text.line.last.and.arrowtriangle.forward") { run(.queue) }
                Button("Download", systemImage: "arrow.down.circle") { run(.download) }
                if model.watchable {
                    Button("Mark Watched", systemImage: "checkmark.circle") { run(.watched) }
                    Button("Mark Unwatched", systemImage: "circle") { run(.unwatched) }
                }
                playlistMenu
            }
            .disabled(busy || empty)
            Spacer()
            Button("Select Loaded") { model.selectLoaded() }
                .buttonStyle(.borderless)
            Button("Done") {
                model.selecting = false
                model.clearSelection()
            }
            .keyboardShortcut(.cancelAction)
        }
        .labelStyle(.titleAndIcon)
        .controlSize(.small)
        .padding(.horizontal, Theme.gutter)
        .padding(.vertical, 10)
        .background(Color(nsColor: .windowBackgroundColor))
        .overlay(alignment: .top) { Hairline() }
        .task(id: playlistKind) {
            guard let kind = playlistKind else { playlists = []; return }
            playlists = ((try? await API.playlists(kind: kind)) ?? []).filter { !$0.smart }
        }
        .alert("New Playlist", isPresented: $naming) {
            TextField("Name", text: $playlistName)
            Button("Create") { addToPlaylist(id: nil, title: playlistName) }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Adds \(Fmt.plural(items.count, "title")) to a new playlist.")
        }
    }

    private var playlistMenu: some View {
        Menu {
            ForEach(playlists) { p in
                Button(p.name) { addToPlaylist(id: p.ratingKey, title: nil) }
            }
            if !playlists.isEmpty { Divider() }
            Button("New Playlist…") { playlistName = ""; naming = true }
        } label: {
            Label("Add to Playlist", systemImage: "text.badge.plus")
        }
        .fixedSize()
        .disabled(items.isEmpty || playlistKind == nil)
        .help(playlistKind == nil && !items.isEmpty ? "Select only video or only music for a playlist" : "Add to a playlist")
    }

    private func addToPlaylist(id: String?, title: String?) {
        guard let kind = playlistKind else { return }
        let keys = items.compactMap(\.ratingKey).joined(separator: ",")
        if id == nil && (title ?? "").trimmingCharacters(in: .whitespaces).isEmpty { return }
        Task {
            let ok = await app.act("Added \(Fmt.plural(items.count, "title")) to playlist") {
                try await API.addToPlaylist(keys, kind: kind, playlist: id, title: title?.trimmingCharacters(in: .whitespaces))
            }
            if ok { model.clearSelection() }
        }
    }

    private func run(_ action: Action) {
        let list = items
        guard !list.isEmpty else { return }
        let scope = app.scope
        busy = true
        done = 0
        Task {
            var failures: [String] = []
            defer { busy = false }
            if action == .queue {
                do { try await API.queueItems(list); done = list.count } catch { failures.append(error.localizedDescription) }
            } else {
                for item in list {
                    if app.scope != scope { failures.append("Stopped because the account or server changed."); break }
                    do {
                        switch action {
                        case .download: _ = try await API.download(item.ratingKey ?? "")
                        case .watched: try await API.setWatched(item.ratingKey ?? "", true)
                        case .unwatched: try await API.setWatched(item.ratingKey ?? "", false)
                        case .queue: break
                        }
                        done += 1
                    } catch {
                        failures.append("\(item.name): \(error.localizedDescription)")
                    }
                }
            }
            if action == .watched || action == .unwatched { app.refresh() }
            if failures.isEmpty {
                let verb = switch action {
                case .queue: "queued"
                case .download: "downloading"
                case .watched: "marked watched"
                case .unwatched: "marked unwatched"
                }
                app.toast("\(Fmt.plural(done, "title")) \(verb)")
                model.clearSelection()
            } else {
                app.error(failures.prefix(3).joined(separator: "\n") + (failures.count > 3 ? "\n…and \(failures.count - 3) more" : ""))
            }
        }
    }
}
