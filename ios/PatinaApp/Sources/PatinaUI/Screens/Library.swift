// Port of web/src/pages/Library.svelte (and components/BulkActions.svelte): a library
// section as an adaptive grid that loads pages as it scrolls, with the server's sorts and
// filters, type switching, an A–Z rail, saved views, Select mode, and the Recommended,
// Collections and Folders tabs. The model lives in the store (LibraryModel.swift), so
// filters and scroll survive a visit to an item.

import PatinaCore
import PlexKit
import SwiftUI

struct LibraryScreen: View {
    let id: String

    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var model: LibraryModel?
    @State private var filtersOpen = false
    @State private var naming = false
    @State private var viewName = ""

    var body: some View {
        Group {
            if let model {
                if model.section == nil {
                    StatusView(title: "Library not found", systemImage: "square.stack")
                } else {
                    content(model)
                }
            } else {
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .navigationTitle(model?.section.map(Directory.title) ?? "Library")
        .task {
            if model == nil { model = store.libraryModel(id) }
            guard let model else { return }
            await model.loadMeta()
            await model.loadTab()
        }
    }

    // MARK: Layout

    private func content(_ model: LibraryModel) -> some View {
        @Bindable var model = model
        return ScrollView {
            LazyVStack(alignment: .leading, spacing: 16, pinnedViews: []) {
                header(model)
                switch model.tab {
                case .browse: browse(model)
                case .recommended: recommended(model)
                case .collections: collections(model)
                case .folders: folders(model)
                }
            }
            .padding(.bottom, 24)
        }
        .scrollPosition(id: $model.position, anchor: .top)
        .overlay(alignment: .trailing) {
            if model.tab == .browse && model.railOn && !model.chars.isEmpty && (model.pages.total ?? 0) > 30 {
                LetterRail(model: model)
            }
        }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            if model.selecting { BulkBar(model: model) }
        }
        .refreshable {
            if model.tab == .browse { model.pages.refresh() } else { await model.loadTab() }
        }
        .toolbar { toolbar(model) }
        .sheet(isPresented: $filtersOpen) {
            LibraryFiltersSheet(model: model)
                .environment(store)
                .tint(store.accent)
                .preferredColorScheme(.dark)
        }
        .alert("Save View", isPresented: $naming) {
            TextField("Name", text: $viewName)
            Button("Cancel", role: .cancel) {}
            Button("Save") { model.saveView(viewName) }
        } message: {
            Text("Saves the type, sort and filters of this library.")
        }
        .onChange(of: store.revision) { Task { await model.loadTab() } }
        .onChange(of: model.tab) { Task { await model.loadTab() } }
        .onChange(of: "\(model.sortValue)|\(model.unwatched)|\(model.pairs.map { "\($0.0)=\($0.1)" })|\(model.mediaType ?? 0)") {
            model.sync()
        }
    }

    private func header(_ model: LibraryModel) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            if let scan = store.activities.first(where: { $0.sectionId == id }) {
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small)
                    Text("Scanning… \(scan.progress)%").font(.footnote.monospacedDigit()).foregroundStyle(Theme.secondaryText)
                }
            }
            PillTabs(selection: Bindable(model).tab, options: LibraryTab.allCases.map { ($0, $0.title) })
            if model.tab == .browse {
                if model.types.count > 1 {
                    Picker("Type", selection: Binding(get: { model.mediaType ?? 0 }, set: { model.setType($0) })) {
                        ForEach(model.types, id: \.0) { Text($0.1).tag($0.0) }
                    }
                    .pickerStyle(.segmented)
                    .frame(maxWidth: 420)
                }
                chips(model)
            }
        }
        .padding(.horizontal, Theme.margin(sizeClass))
    }

    /// The active filters, removable, and the count.
    private func chips(_ model: LibraryModel) -> some View {
        ScrollView(.horizontal) {
            HStack(spacing: 8) {
                if let total = model.pages.total {
                    Text(Format.plural(total, model.noun))
                        .font(.subheadline.monospacedDigit())
                        .foregroundStyle(Theme.secondaryText)
                        .padding(.trailing, 4)
                }
                if model.unwatched {
                    chip("Unwatched") { model.unwatched = false }
                }
                ForEach(model.toggles, id: \.self) { key in
                    chip(model.filters.first { $0.filter == key }?.title ?? key) { model.toggle(key) }
                }
                ForEach(model.picked, id: \.self) { p in
                    chip("\(p.title): \(p.label)") { model.picked.removeAll { $0 == p } }
                }
                if model.activeFilters > 1 {
                    Button("Clear") { withAnimation { model.clearFilters() } }.font(.subheadline)
                }
            }
        }
        .scrollIndicators(.hidden)
    }

    private func chip(_ text: String, remove: @escaping () -> Void) -> some View {
        Button {
            Haptics.selection()
            withAnimation { remove() }
        } label: {
            HStack(spacing: 5) {
                Text(text).lineLimit(1)
                Image(systemName: "xmark").font(.caption2.weight(.bold)).foregroundStyle(Theme.secondaryText)
            }
            .font(.subheadline.weight(.medium))
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background(store.accent.opacity(0.16), in: .capsule)
            .overlay(Capsule().strokeBorder(store.accent.opacity(0.35)))
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Remove filter \(text)")
    }

    @ToolbarContentBuilder
    private func toolbar(_ model: LibraryModel) -> some ToolbarContent {
        ToolbarItemGroup(placement: .topBarTrailing) {
            if model.tab == .browse {
                Menu {
                    Picker("Sort", selection: Bindable(model).sort) {
                        if model.sorts.isEmpty {
                            Text("Title").tag("titleSort")
                            Text("Date Added").tag("addedAt")
                            Text("Release Date").tag("originallyAvailableAt")
                            Text("Rating").tag("rating")
                        }
                        ForEach(model.sorts, id: \.key) { s in Text(Directory.title(s)).tag(s.key ?? "") }
                    }
                    Picker("Order", selection: Bindable(model).desc) {
                        Label("Ascending", systemImage: "arrow.up").tag(false)
                        Label("Descending", systemImage: "arrow.down").tag(true)
                    }
                } label: {
                    Image(systemName: "arrow.up.arrow.down")
                }
                .accessibilityLabel("Sort")
                Button {
                    filtersOpen = true
                } label: {
                    Image(systemName: model.activeFilters > 0 ? "line.3.horizontal.decrease.circle.fill" : "line.3.horizontal.decrease.circle")
                }
                .accessibilityLabel("Filter")
            }
            Menu {
                if model.tab == .browse {
                    Button(model.selecting ? "Done Selecting" : "Select", systemImage: "checkmark.circle") {
                        withAnimation {
                            model.selecting.toggle()
                            if !model.selecting { model.selection = [] }
                        }
                    }
                    Section("Saved Views") {
                        ForEach(model.savedViews) { v in
                            Button(v.name, systemImage: "bookmark") { model.useView(v) }
                        }
                        Button("Save Current View…", systemImage: "plus") {
                            viewName = ""
                            naming = true
                        }
                        if !model.savedViews.isEmpty {
                            Menu("Remove View", systemImage: "trash") {
                                ForEach(model.savedViews) { v in
                                    Button(v.name, role: .destructive) { model.removeView(v) }
                                }
                            }
                        }
                    }
                }
                if store.owned {
                    Button("Scan Library Files", systemImage: "arrow.clockwise") {
                        store.perform("Scanning library…") { try await PendingCommands.scanSection(store.app, id: id) }
                    }
                }
            } label: {
                Image(systemName: "ellipsis.circle")
            }
        }
    }

    // MARK: Tabs

    @ViewBuilder
    private func browse(_ model: LibraryModel) -> some View {
        let pages = model.pages
        if let error = pages.error {
            if (pages.total ?? 0) > 0 {
                StaleBanner(error: error) { pages.refresh() }.padding(.horizontal, Theme.margin(sizeClass))
            } else if pages.total == nil {
                StatusView(error: error) { pages.refresh() }
            }
        }
        if let total = pages.total {
            if total == 0 && pages.loading == 0 {
                StatusView(title: "No matches", text: "Try a different filter.", systemImage: "line.3.horizontal.decrease.circle")
            } else {
                let shape = model.shape
                LazyVGrid(columns: cardColumns(shape, sizeClass), spacing: 22) {
                    ForEach(0..<total, id: \.self) { i in
                        Group {
                            if let item = pages.at(i) {
                                Card(
                                    item, shape: shape, selecting: model.selecting, selected: model.isSelected(item),
                                    onToggle: { model.toggleSelection(item) })
                            } else {
                                SkeletonCard(shape: shape)
                            }
                        }
                        .onAppear { pages.ensure(i, i + 1) }
                    }
                }
                .scrollTargetLayout()
                .padding(.horizontal, Theme.margin(sizeClass))
                .padding(.trailing, model.railOn && !model.chars.isEmpty ? 14 : 0)
            }
        } else if pages.error == nil {
            LazyVGrid(columns: cardColumns(model.shape, sizeClass), spacing: 22) {
                ForEach(0..<12, id: \.self) { _ in SkeletonCard(shape: model.shape) }
            }
            .padding(.horizontal, Theme.margin(sizeClass))
        }
    }

    @ViewBuilder
    private func recommended(_ model: LibraryModel) -> some View {
        let hubs = model.hubs
        if let value = hubs.value {
            let rows = value.filter { !$0.metadata.isEmpty }
            if rows.isEmpty {
                StatusView(title: "No recommendations yet", systemImage: "sparkles")
            } else {
                ForEach(Array(rows.enumerated()), id: \.offset) { _, hub in HubRow(hub: hub).padding(.top, 8) }
            }
        } else if let error = hubs.error {
            StatusView(error: error) { hubs.reload() }
        } else {
            SkeletonRow(width: cardWidth(.poster, sizeClass)).padding(.leading, Theme.margin(sizeClass))
        }
    }

    @ViewBuilder
    private func collections(_ model: LibraryModel) -> some View {
        let list = model.collections
        if let value = list.value {
            if value.isEmpty {
                StatusView(title: "No collections", text: "Collections you create in Plex show up here.", systemImage: "rectangle.stack")
            } else {
                LazyVGrid(columns: cardColumns(.poster, sizeClass), spacing: 22) {
                    ForEach(value, id: \.ratingKey) { Card($0) }
                }
                .padding(.horizontal, Theme.margin(sizeClass))
            }
        } else if let error = list.error {
            StatusView(error: error) { list.reload() }
        } else {
            ProgressView().frame(maxWidth: .infinity).padding(.top, 40)
        }
    }

    @ViewBuilder
    private func folders(_ model: LibraryModel) -> some View {
        // The path below the section.
        ScrollView(.horizontal) {
            HStack(spacing: 4) {
                crumb(model.section.map(Directory.title) ?? "", current: model.trail.isEmpty) {
                    model.trail = []
                    Task { await model.loadTab() }
                }
                ForEach(Array(model.trail.enumerated()), id: \.offset) { i, step in
                    Image(systemName: "chevron.right").font(.caption2).foregroundStyle(Theme.tertiaryText)
                    crumb(step.title, current: i == model.trail.count - 1) {
                        model.trail = Array(model.trail.prefix(i + 1))
                        Task { await model.loadTab() }
                    }
                }
            }
        }
        .scrollIndicators(.hidden)
        .contentMargins(.horizontal, Theme.margin(sizeClass), for: .scrollContent)
        if let value = model.folders.value {
            if value.directories.isEmpty && value.items.isEmpty {
                StatusView(title: "Empty folder", systemImage: "folder")
            }
            if !value.directories.isEmpty {
                VStack(spacing: 0) {
                    ForEach(Array(value.directories.enumerated()), id: \.offset) { i, d in
                        if i > 0 { Divider().overlay(Theme.hairline) }
                        Button {
                            model.trail.append((d.key, d.title ?? "Folder"))
                            Task { await model.loadTab() }
                        } label: {
                            HStack(spacing: 12) {
                                Image(systemName: "folder.fill").foregroundStyle(store.accent)
                                Text(d.title ?? "Folder").foregroundStyle(.primary).lineLimit(1)
                                Spacer()
                                Image(systemName: "chevron.right").font(.caption).foregroundStyle(Theme.tertiaryText)
                            }
                            .padding(.horizontal, 14)
                            .padding(.vertical, 13)
                            .contentShape(.rect)
                        }
                        .buttonStyle(.plain)
                    }
                }
                .panel()
                .padding(.horizontal, Theme.margin(sizeClass))
            }
            if !value.items.isEmpty {
                LazyVGrid(columns: cardColumns(PlexUI.commonShape(value.items), sizeClass), spacing: 22) {
                    ForEach(Array(value.items.enumerated()), id: \.offset) { _, item in
                        Card(item, shape: PlexUI.commonShape(value.items))
                    }
                }
                .padding(.horizontal, Theme.margin(sizeClass))
            }
        } else if let error = model.folders.error {
            StatusView(error: error) { model.folders.reload() }
        } else {
            ProgressView().frame(maxWidth: .infinity).padding(.top, 40)
        }
    }

    private func crumb(_ title: String, current: Bool, action: @escaping () -> Void) -> some View {
        Button(title, action: action)
            .font(.subheadline.weight(current ? .semibold : .regular))
            .foregroundStyle(current ? .primary : Theme.secondaryText)
            .buttonStyle(.plain)
    }
}

/// Capsule tabs that scroll when they don't fit.
struct PillTabs<T: Hashable>: View {
    @Binding var selection: T
    let options: [(T, String)]
    @Environment(AppStore.self) private var store
    @Namespace private var pill

    var body: some View {
        ScrollView(.horizontal) {
            HStack(spacing: 6) {
                ForEach(options, id: \.0) { value, title in
                    let on = value == selection
                    Button {
                        Haptics.selection()
                        withAnimation(.spring(duration: 0.3)) { selection = value }
                    } label: {
                        Text(title)
                            .font(.subheadline.weight(on ? .semibold : .medium))
                            .foregroundStyle(on ? Color.black : Theme.secondaryText)
                            .padding(.horizontal, 14)
                            .padding(.vertical, 7)
                            .background {
                                if on {
                                    Capsule().fill(store.accent).matchedGeometryEffect(id: "pill", in: pill)
                                } else {
                                    Capsule().fill(Theme.raised)
                                }
                            }
                    }
                    .buttonStyle(.plain)
                    .accessibilityAddTraits(on ? .isSelected : [])
                }
            }
        }
        .scrollIndicators(.hidden)
    }
}

/// The A–Z rail on the trailing edge: tap or scrub like Contacts to jump to a letter.
struct LetterRail: View {
    let model: LibraryModel
    @Environment(AppStore.self) private var store
    @State private var active: String?

    var body: some View {
        let starts = model.starts
        let letters = LibraryModel.letters
        GeometryReader { proxy in
            let step = min(18, proxy.size.height / CGFloat(letters.count))
            VStack(spacing: 0) {
                ForEach(letters, id: \.self) { l in
                    Text(l)
                        .font(.system(size: 10.5, weight: .semibold, design: .rounded))
                        .foregroundStyle(starts[l] == nil ? Theme.tertiaryText.opacity(0.5) : (active == l ? store.accent : store.accent.opacity(0.85)))
                        .frame(width: 20, height: step)
                }
            }
            .frame(maxHeight: .infinity)
            .contentShape(.rect)
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { g in
                        let top = (proxy.size.height - step * CGFloat(letters.count)) / 2
                        let i = Int((g.location.y - top) / step)
                        guard letters.indices.contains(i) else { return }
                        // The letter, or the next one that has titles.
                        guard let l = letters[i...].first(where: { starts[$0] != nil }), l != active else { return }
                        active = l
                        Haptics.selection()
                        if let at = starts[l] { model.position = at }
                    }
                    .onEnded { _ in withAnimation(.easeOut(duration: 0.3).delay(0.4)) { active = nil } }
            )
            .overlay(alignment: .center) {
                if let active {
                    Text(active)
                        .font(.system(size: 44, weight: .bold, design: .rounded))
                        .frame(width: 84, height: 84)
                        .background(.ultraThinMaterial, in: .rect(cornerRadius: 20, style: .continuous))
                        .offset(x: -80)
                        .transition(.scale.combined(with: .opacity))
                }
            }
        }
        .frame(width: 22)
        .padding(.vertical, 24)
        .padding(.trailing, 2)
        .accessibilityElement()
        .accessibilityLabel("Jump to letter")
        .accessibilityAdjustableAction { direction in
            let present = letters.filter { starts[$0] != nil }
            let i = active.flatMap { present.firstIndex(of: $0) } ?? -1
            let next = direction == .increment ? min(present.count - 1, i + 1) : max(0, i - 1)
            guard present.indices.contains(next) else { return }
            active = present[next]
            model.position = starts[present[next]]
        }
    }
}

/// Port of web/src/components/BulkActions.svelte: what to do with the selected titles.
struct BulkBar: View {
    let model: LibraryModel
    @Environment(AppStore.self) private var store
    @State private var busy = false

    var body: some View {
        let items = model.selection
        HStack(spacing: 14) {
            Text("\(items.count) selected")
                .font(.subheadline.weight(.semibold).monospacedDigit())
            if busy { ProgressView().controlSize(.small) }
            Spacer()
            Button("Mark Watched", systemImage: "checkmark.circle") { watched(items) }
                .labelStyle(.iconOnly)
            Button("Add to Queue", systemImage: "text.line.last.and.arrowtriangle.forward") {
                store.perform { try await PendingCommands.queueItems(store.app, items: items) }
            }
            .labelStyle(.iconOnly)
            Menu {
                Button("Download", systemImage: "arrow.down.circle") { download(items) }
                Button("Add to Playlist…", systemImage: "text.badge.plus") { playlist(items) }
                if store.owned {
                    Button("Add to Collection…", systemImage: "rectangle.stack.badge.plus") {
                        if let first = items.first { store.sheet = .collection(first, items: items) }
                    }
                }
                Divider()
                Button("Clear Selection", systemImage: "xmark") { model.selection = [] }
            } label: {
                Image(systemName: "ellipsis.circle")
            }
        }
        .font(.title3)
        .disabled(busy || items.isEmpty)
        .padding(.horizontal, 18)
        .padding(.vertical, 12)
        .background(.ultraThinMaterial, in: .rect(cornerRadius: 20, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 20, style: .continuous).strokeBorder(Theme.hairline))
        .padding(.horizontal, 12)
        .padding(.bottom, 6)
        .transition(.move(edge: .bottom).combined(with: .opacity))
    }

    private func watched(_ items: [Metadata]) {
        let scope = store.scope
        busy = true
        Task {
            // Undone together; the core toasts it with Undo.
            if await store.act(nil, { try await Commands.setWatchedMany(store.app, ratingKeys: items.map(Metadata.ratingKey), watched: true) }),
                scope == store.scope
            {
                model.selection = []
            }
            busy = false
        }
    }

    private func download(_ items: [Metadata]) {
        let scope = store.scope
        busy = true
        Task {
            var failures: [String] = []
            for item in items {
                guard scope == store.scope else { break }
                do {
                    _ = try await Commands.download(store.app, ratingKey: Metadata.ratingKey(item))
                } catch {
                    failures.append("\(Metadata.title(item)): \(errorText(error))")
                    if failures.count == 1 && failures.count == items.count { break }
                }
            }
            if failures.isEmpty {
                store.toast("\(items.count) titles queued for download")
                model.selection = []
            } else {
                store.error(failures.prefix(3).joined(separator: "\n"))
            }
            busy = false
        }
    }

    private func playlist(_ items: [Metadata]) {
        guard let first = items.first else { return }
        if Set(items.map(PlexUI.playlistKind)).count != 1 {
            store.error("Select only video or only music for a playlist.")
            return
        }
        store.sheet = .playlist(first, items: items)
    }
}

/// Filters and toggles of a library, in a sheet.
struct LibraryFiltersSheet: View {
    let model: LibraryModel
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        @Bindable var model = model
        NavigationStack {
            List {
                if model.watchable || !model.booleans.isEmpty {
                    Section {
                        if model.watchable { Toggle("Unwatched", isOn: $model.unwatched) }
                        ForEach(model.booleans, id: \.filter) { f in
                            Toggle(f.title, isOn: Binding(get: { model.toggles.contains(f.filter) }, set: { _ in model.toggle(f.filter) }))
                        }
                    }
                }
                if !model.valueFilters.isEmpty {
                    Section("Filter By") {
                        ForEach(model.valueFilters, id: \.filter) { f in
                            NavigationLink {
                                FilterValuesView(model: model, filter: f)
                            } label: {
                                LabeledContent(f.title) {
                                    let chosen = model.picked.filter { $0.filter == f.filter }.map(\.label)
                                    Text(chosen.isEmpty ? "Any" : chosen.joined(separator: ", ")).lineLimit(1)
                                }
                            }
                        }
                    }
                }
                if model.activeFilters > 0 {
                    Section {
                        Button("Clear All Filters", role: .destructive) { model.clearFilters() }
                    }
                }
            }
            .navigationTitle("Filters")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } }
            }
        }
        .presentationDetents([.medium, .large])
    }
}

/// The values of one filter, several of which may be chosen.
struct FilterValuesView: View {
    let model: LibraryModel
    let filter: PendingCommands.FilterView

    @Environment(AppStore.self) private var store
    @State private var values = Loader<[Directory]>()
    @State private var query = ""

    var body: some View {
        let q = query.trimmingCharacters(in: .whitespaces).lowercased()
        let shown = (values.value ?? []).filter { q.isEmpty || ($0.title ?? "").lowercased().contains(q) }
        List {
            if let error = values.error {
                Text(error).foregroundStyle(Theme.danger)
            } else if values.value == nil {
                ProgressView().frame(maxWidth: .infinity)
            } else if shown.isEmpty {
                Text("Nothing to choose").foregroundStyle(Theme.secondaryText)
            }
            ForEach(Array(shown.enumerated()), id: \.offset) { _, v in
                if let key = v.key {
                    let on = model.picked.contains { $0.filter == filter.filter && $0.value == key }
                    Button {
                        Haptics.selection()
                        model.togglePick(filter, value: key, label: v.title ?? key)
                    } label: {
                        HStack {
                            Text(v.title ?? key).foregroundStyle(.primary)
                            Spacer()
                            if on { Image(systemName: "checkmark").foregroundStyle(store.accent).fontWeight(.semibold) }
                        }
                    }
                }
            }
        }
        .searchable(text: $query, placement: .navigationBarDrawer(displayMode: .automatic), prompt: "Search \(filter.title.lowercased())")
        .navigationTitle(filter.title)
        .task {
            let (app, id, kind, f) = (store.app!, model.id, model.mediaType, filter.filter)
            await values.load { try await Commands.sectionFilterValues(app, id: id, filter: f, kind: kind) }
        }
    }
}
