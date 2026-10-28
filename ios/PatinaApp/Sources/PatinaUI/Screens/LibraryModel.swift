// Port of web/src/lib/library.svelte.ts and the state of web/src/pages/Library.svelte:
// the sparse page cache of a library section, its query (type, sort, filters, unwatched),
// its tabs, saved views and selection. The store keeps a model per library, so all of it
// (and the scroll position) survives leaving and coming back.

import PatinaCore
import PlexKit
import SwiftUI

/// Sparse page cache: a jump to Z never loads A–Y, and old results survive refresh
/// failures. A change on the server marks cached pages stale instead of dropping them: the
/// pages in view refetch at once, the others when they scroll into view, and everything stays
/// on screen meanwhile.
@MainActor
@Observable
final class LibraryPages {
    static let PAGE = 120
    typealias Load = (Int, Int) async throws -> MediaContainer

    private(set) var pages: [Int: [Metadata]] = [:]
    private(set) var total: Int?
    private(set) var error: String?
    private(set) var loading = 0
    @ObservationIgnored private(set) var key = ""
    @ObservationIgnored private(set) var revision = -1
    @ObservationIgnored private var generation = 0
    @ObservationIgnored private var pending = Set<Int>()
    @ObservationIgnored private var failed = Set<Int>()
    /// Cached pages that may be out of date; they still display until refetched.
    @ObservationIgnored private var stale = Set<Int>()
    @ObservationIgnored private var load: Load?
    @ObservationIgnored private var visible = 0
    @ObservationIgnored private var visibleEnd = 0

    /// `section` is the library the pages belong to: revisions that don't touch it leave the
    /// cache alone.
    func configure(_ key: String, revision: Int, store: AppStore, section: String?, load: @escaping Load) {
        self.load = load
        if self.key != key {
            detach()
            self.load = load
            self.key = key
            pages = [:]
            stale.removeAll()
            total = nil
            error = nil
            self.revision = revision
            ensure(0, Self.PAGE)
            return
        }
        if self.revision != revision {
            let affected = section.map { store.affects(section: $0, since: self.revision) } ?? true
            self.revision = revision
            if affected {
                refresh()
                return
            }
        }
        if total == nil { ensure(0, Self.PAGE) }
    }

    func at(_ index: Int) -> Metadata? {
        let page = pages[index / Self.PAGE * Self.PAGE]
        let i = index % Self.PAGE
        guard let page, i < page.count else { return nil }
        return page[i]
    }

    /// Make sure the pages covering items `start` to `end` are loaded and current.
    func ensure(_ start: Int, _ end: Int) {
        visible = start
        visibleEnd = end
        var offset = start / Self.PAGE * Self.PAGE
        while offset < max(end, start + 1) {
            if pages[offset] == nil || stale.contains(offset) { fetch(offset) }
            offset += Self.PAGE
        }
    }

    /// Everything cached may be out of date: refetch what's in view now and the rest lazily.
    func refresh() {
        generation += 1
        pending.removeAll()
        failed.removeAll()
        loading = 0
        error = nil
        for offset in pages.keys { stale.insert(offset) }
        ensure(visible, visibleEnd)
    }

    func detach() {
        generation += 1
        pending.removeAll()
        failed.removeAll()
        loading = 0
        load = nil
    }

    private func fetch(_ offset: Int) {
        guard let load, !pending.contains(offset), !failed.contains(offset) else { return }
        let generation = self.generation
        pending.insert(offset)
        loading += 1
        Task {
            do {
                let mc = try await load(offset, Self.PAGE)
                guard generation == self.generation else { return }
                total = mc.totalSize ?? (offset + mc.metadata.count)
                pages[offset] = mc.metadata
                stale.remove(offset)
                // Bound memory: keep the pages nearest the viewport.
                let keep = pages.keys.sorted { abs($0 - visible) < abs($1 - visible) }
                for key in keep.dropFirst(12) {
                    pages[key] = nil
                    stale.remove(key)
                }
            } catch {
                guard generation == self.generation else { return }
                self.error = errorText(error)
                failed.insert(offset)
            }
            if generation == self.generation {
                pending.remove(offset)
                loading -= 1
            }
        }
    }
}

/// An active value filter ("Genre: Drama"), as saved views store it.
struct PickedFilter: Hashable {
    var filter: String
    var title: String
    var value: String
    var label: String

    var json: JSONValue {
        ["filter": .string(filter), "title": .string(title), "value": .string(value), "label": .string(label)]
    }

    init(filter: String, title: String, value: String, label: String) {
        self.filter = filter
        self.title = title
        self.value = value
        self.label = label
    }

    init?(json: JSONValue) {
        guard let filter = json["filter"]?.stringValue, let value = json["value"]?.stringValue else { return nil }
        self.filter = filter
        self.value = value
        title = json["title"]?.stringValue ?? filter
        label = json["label"]?.stringValue ?? value
    }
}

/// A named library query (`SavedView`), stored in `ui.saved_views` in the web's format.
struct SavedView: Identifiable {
    var id: String
    var name: String
    var scope: String
    var library: String
    var mediaType: Int
    var sort: String
    var desc: Bool
    var unwatched: Bool
    var toggles: [String]
    var picked: [PickedFilter]

    init?(json: JSONValue) {
        guard let id = json["id"]?.stringValue, let name = json["name"]?.stringValue,
            let scope = json["scope"]?.stringValue, let library = json["library"]?.stringValue
        else { return nil }
        self.id = id
        self.name = name
        self.scope = scope
        self.library = library
        mediaType = json["mediaType"]?.intValue ?? 0
        sort = json["sort"]?.stringValue ?? "titleSort"
        desc = json["desc"]?.boolValue ?? false
        unwatched = json["unwatched"]?.boolValue ?? false
        toggles = json["toggles"]?.arrayValue?.compactMap(\.stringValue) ?? []
        picked = json["picked"]?.arrayValue?.compactMap(PickedFilter.init(json:)) ?? []
    }

    init(
        id: String, name: String, scope: String, library: String, mediaType: Int, sort: String, desc: Bool,
        unwatched: Bool, toggles: [String], picked: [PickedFilter]
    ) {
        self.id = id
        self.name = name
        self.scope = scope
        self.library = library
        self.mediaType = mediaType
        self.sort = sort
        self.desc = desc
        self.unwatched = unwatched
        self.toggles = toggles
        self.picked = picked
    }

    var json: JSONValue {
        [
            "id": .string(id), "name": .string(name), "scope": .string(scope), "library": .string(library),
            "mediaType": .number(Double(mediaType)), "sort": .string(sort), "desc": .bool(desc),
            "unwatched": .bool(unwatched), "toggles": .array(toggles.map { .string($0) }),
            "picked": .array(picked.map(\.json)),
        ]
    }
}

enum LibraryTab: String, CaseIterable, Hashable {
    case browse, recommended, collections, folders

    var title: String {
        switch self {
        case .browse: "Library"
        case .recommended: "Recommended"
        case .collections: "Collections"
        case .folders: "Folders"
        }
    }
}

@MainActor
@Observable
final class LibraryModel {
    let id: String
    @ObservationIgnored private unowned let store: AppStore

    static let types: [String: [(Int, String)]] = [
        "show": [(2, "Shows"), (3, "Seasons"), (4, "Episodes")],
        "artist": [(8, "Artists"), (9, "Albums"), (10, "Tracks")],
    ]
    static let letters = ["#"] + "ABCDEFGHIJKLMNOPQRSTUVWXYZ".map(String.init)

    var tab = LibraryTab.browse
    var mediaType: Int?
    var sort = "titleSort"
    var desc = false
    var unwatched = false
    /// Active boolean filters, by key.
    var toggles: [String] = []
    var picked: [PickedFilter] = []

    let pages = LibraryPages()
    var sorts: [Directory] = []
    var filters: [PendingCommands.FilterView] = []
    /// First characters of the titles, for the A–Z rail.
    var chars: [Directory] = []
    @ObservationIgnored private var charsKey = ""

    var selecting = false
    var selection: [Metadata] = []
    /// The grid's scroll position (an item index), kept across visits.
    var position: Int?
    /// Folders tab: the path below the section.
    var trail: [(key: String?, title: String)] = []

    let hubs = Loader<[Hub]>()
    let collections = Loader<[Metadata]>()
    let folders = Loader<PendingCommands.FolderView>()

    init(id: String, store: AppStore) {
        self.id = id
        self.store = store
        mediaType = Self.types[section?.type ?? ""]?.first?.0
    }

    var section: Directory? { store.sections.first { $0.key == id } }
    var kind: String { section?.type ?? "" }
    var types: [(Int, String)] { Self.types[kind] ?? [] }
    var watchable: Bool { kind != "artist" && kind != "photo" }
    var booleans: [PendingCommands.FilterView] { filters.filter { $0.kind == "boolean" && $0.filter != "unwatched" } }
    var valueFilters: [PendingCommands.FilterView] { filters.filter { $0.kind != "boolean" } }
    var pairs: [(String, String)] { toggles.map { ($0, "1") } + picked.map { ($0.filter, $0.value) } }
    var filtered: Bool { unwatched || !pairs.isEmpty }
    var activeFilters: Int { (unwatched ? 1 : 0) + toggles.count + picked.count }

    var sortValue: String {
        guard desc else { return sort }
        return sorts.first { $0.key == sort }?.descKey ?? "\(sort):desc"
    }

    var shape: CardShape {
        if mediaType == 4 { return .landscape }
        if let t = mediaType, t >= 8 { return .square }
        return kind == "photo" ? .square : .poster
    }

    var noun: String {
        let label = types.first { $0.0 == mediaType }?.1.lowercased() ?? "items"
        return label.hasSuffix("s") ? String(label.dropLast()) : label
    }

    /// The A–Z rail jumps when the grid is in title order and unfiltered.
    var railOn: Bool { sort.hasPrefix("titleSort") && !desc && !filtered }

    /// Index of the first item under each letter present.
    var starts: [String: Int] {
        var out: [String: Int] = [:]
        var n = 0
        for c in chars {
            let letter = (c.title ?? c.key ?? "").uppercased()
            if out[letter] == nil { out[letter] = n }
            n += c.size ?? 0
        }
        return out
    }

    private var queryKey: String {
        let filters = pairs.map { "\($0.0)=\($0.1)" }.joined(separator: "&")
        return "\(mediaType.map(String.init) ?? "")|\(sortValue)|\(unwatched)|\(filters)"
    }

    // MARK: Loading

    /// Load what the browse tab shows, or refetch what a revision changed.
    func sync() {
        guard tab == .browse else { return }
        let key = queryKey
        if !pages.key.isEmpty && pages.key != key {
            selection = []
            position = nil
        }
        let (app, id, kind, sort, unwatched, filters) = (store.app!, id, mediaType, sortValue, unwatched, pairs)
        pages.configure(key, revision: store.revision, store: store, section: id) { start, size in
            try await Commands.sectionItems(
                app, id: id, kind: kind, sort: sort, unwatched: unwatched, filters: filters, start: start, size: size)
        }
        loadChars()
    }

    func loadMeta() async {
        let app = store.app!
        if sorts.isEmpty { sorts = (try? await Commands.sectionSorts(app, id: id)) ?? [] }
        filters = (try? await PendingCommands.sectionFilters(app, id: id, kind: mediaType)) ?? []
    }

    private func loadChars() {
        let key = "\(mediaType.map(String.init) ?? "")|\(store.revision)"
        guard railOn, charsKey != key else { return }
        charsKey = key
        let (app, id, kind) = (store.app!, id, mediaType)
        Task { chars = (try? await PendingCommands.firstCharacters(app, id: id, kind: kind)) ?? [] }
    }

    func loadTab() async {
        let app = store.app!
        let revision = store.revision
        switch tab {
        case .browse:
            sync()
        case .recommended:
            await hubs.load(at: revision) { try await Commands.sectionHubs(app, id: self.id) }
        case .collections:
            await collections.load(at: revision) { try await Commands.collections(app, id: self.id) }
        case .folders:
            let key = trail.last?.key
            await folders.load { try await PendingCommands.sectionFolders(app, id: self.id, key: key) }
        }
    }

    // MARK: Query

    func setType(_ t: Int) {
        mediaType = t
        toggles = []
        picked = []
        charsKey = ""
        Task { await loadMeta() }
    }

    func toggle(_ filter: String) {
        if toggles.contains(filter) { toggles.removeAll { $0 == filter } } else { toggles.append(filter) }
    }

    func togglePick(_ f: PendingCommands.FilterView, value: String, label: String) {
        if let at = picked.firstIndex(where: { $0.filter == f.filter && $0.value == value }) {
            picked.remove(at: at)
        } else {
            picked.append(PickedFilter(filter: f.filter, title: f.title, value: value, label: label))
        }
    }

    func clearFilters() {
        unwatched = false
        toggles = []
        picked = []
    }

    // MARK: Saved views

    var savedViews: [SavedView] {
        (store.settings?.ui.savedViews ?? []).compactMap(SavedView.init(json:))
            .filter { $0.scope == store.scope && $0.library == id }
    }

    func saveView(_ name: String) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        let view = SavedView(
            id: UUID().uuidString.lowercased(), name: trimmed, scope: store.scope, library: id, mediaType: mediaType ?? 0,
            sort: sort, desc: desc, unwatched: unwatched, toggles: toggles, picked: picked)
        store.updateUi(["saved_views": .array((store.settings?.ui.savedViews ?? []) + [view.json])])
        store.toast("Saved “\(trimmed)”")
    }

    func useView(_ view: SavedView) {
        tab = .browse
        if view.mediaType != 0 { mediaType = view.mediaType }
        sort = view.sort
        desc = view.desc
        unwatched = view.unwatched
        toggles = view.toggles
        picked = view.picked
        selection = []
        position = 0
        Task { await loadMeta() }
    }

    func removeView(_ view: SavedView) {
        let all = (store.settings?.ui.savedViews ?? []).filter { $0["id"]?.stringValue != view.id }
        store.updateUi(["saved_views": .array(all)])
    }

    // MARK: Selection

    func isSelected(_ item: Metadata) -> Bool {
        selection.contains { $0.ratingKey == item.ratingKey }
    }

    func toggleSelection(_ item: Metadata) {
        if isSelected(item) {
            selection.removeAll { $0.ratingKey == item.ratingKey }
        } else {
            selection.append(item)
        }
    }
}
