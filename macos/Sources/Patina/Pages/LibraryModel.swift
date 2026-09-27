import Foundation
import Observation

/// A sparse page cache for a library listing (web/src/lib/library.svelte.ts):
/// a jump to Z never loads A–Y, and old results survive a failed refresh.
@MainActor
@Observable
final class LibraryPages {
    static let size = 120
    /// Pages retained around the viewport.
    private static let keep = 12

    private(set) var pages: [Int: [Metadata]] = [:]
    private(set) var total: Int?
    private(set) var error: String?
    private(set) var loading = 0

    @ObservationIgnored private(set) var key = ""
    @ObservationIgnored private var revision = -1
    @ObservationIgnored private var generation = 0
    @ObservationIgnored private var pending = Set<Int>()
    @ObservationIgnored private var failed = Set<Int>()
    @ObservationIgnored private var load: ((Int, Int) async throws -> MediaContainer)?
    @ObservationIgnored private var visible = 0

    /// Point the cache at a query. A new `key` starts over; a new `revision`
    /// refetches the loaded pages in place.
    func configure(key: String, revision: Int, load: @escaping (Int, Int) async throws -> MediaContainer) {
        self.load = load
        if self.key != key {
            generation += 1
            pending.removeAll()
            failed.removeAll()
            loading = 0
            self.key = key
            pages = [:]
            total = nil
            error = nil
        }
        if self.revision != revision {
            self.revision = revision
            refresh()
        } else if total == nil {
            ensure(0, Self.size)
        }
    }

    func at(_ index: Int) -> Metadata? {
        let offset = index / Self.size * Self.size
        guard let page = pages[offset], index - offset < page.count else { return nil }
        return page[index - offset]
    }

    /// Load the pages covering `start..<end`.
    func ensure(_ start: Int, _ end: Int) {
        visible = start
        var offset = start / Self.size * Self.size
        while offset < end {
            if pages[offset] == nil { fetch(offset) }
            offset += Self.size
        }
    }

    func refresh() {
        generation += 1
        pending.removeAll()
        failed.removeAll()
        loading = 0
        error = nil
        let offsets = pages.keys.sorted()
        for offset in offsets.isEmpty ? [0] : offsets { fetch(offset) }
    }

    /// Stop applying results (the page went away).
    func detach() {
        generation += 1
        pending.removeAll()
        failed.removeAll()
        loading = 0
    }

    private func fetch(_ offset: Int) {
        guard let load, !pending.contains(offset), !failed.contains(offset) else { return }
        let generation = generation
        pending.insert(offset)
        loading += 1
        Task { @MainActor in
            defer {
                if generation == self.generation { pending.remove(offset); loading -= 1 }
            }
            do {
                let mc = try await load(offset, Self.size)
                guard generation == self.generation else { return }
                total = mc.totalSize ?? (offset + mc.metadata.count)
                pages[offset] = mc.metadata
                // Bound memory: keep the pages nearest the viewport.
                let far = pages.keys.sorted { abs($0 - visible) < abs($1 - visible) }.dropFirst(Self.keep)
                for key in far { pages[key] = nil }
            } catch {
                guard generation == self.generation else { return }
                self.error = error.localizedDescription
                failed.insert(offset)
            }
        }
    }
}

/// Everything about one library page that should survive leaving it: the
/// query, tab, folder trail, selection and scroll position.
@MainActor
@Observable
final class LibraryModel {
    enum Tab: String, CaseIterable, Identifiable {
        case browse, recommended, collections, folders
        var id: String { rawValue }
        var title: String {
            switch self {
            case .browse: "Library"
            case .recommended: "Recommended"
            case .collections: "Collections"
            case .folders: "Folders"
            }
        }
    }

    struct Crumb: Hashable { var key: String?; var title: String }

    /// Media types a section can list, by section type.
    static let types: [String: [(Int, String)]] = [
        "show": [(2, "Shows"), (3, "Seasons"), (4, "Episodes")],
        "artist": [(8, "Artists"), (9, "Albums"), (10, "Tracks")],
    ]
    static let letters = ["#"] + "ABCDEFGHIJKLMNOPQRSTUVWXYZ".map(String.init)

    let id: String
    let scope: String
    let kind: String

    var tab: Tab = .browse
    var mediaType: Int?
    var sort = "titleSort"
    var desc = false
    var unwatched = false
    /// Active boolean filters, by key.
    var toggles: [String] = []
    /// Active value filters ("Genre: Drama").
    var picked: [SavedView.Picked] = []

    var sorts: [Directory] = []
    var filters: [FilterView] = []
    var chars: [Directory] = []
    var trail: [Crumb] = []

    var selecting = false
    /// Selected items by ratingKey, and the order they were picked in.
    private(set) var selection: [String: Metadata] = [:]
    private(set) var selectionOrder: [String] = []

    let pages = LibraryPages()

    /// First visible grid index, to restore the scroll position.
    @ObservationIgnored var focus: Int?
    @ObservationIgnored private var visibleIndexes = Set<Int>()

    init(id: String, scope: String, kind: String) {
        self.id = id
        self.scope = scope
        self.kind = kind
        mediaType = Self.types[kind]?.first?.0
    }

    // MARK: Cache

    @ObservationIgnored private static var cache: [String: LibraryModel] = [:]
    @ObservationIgnored private static var recent: [String] = []

    /// The remembered model for a section, or a new one.
    static func model(scope: String, section: Directory) -> LibraryModel {
        let id = section.key ?? ""
        let key = "\(scope):\(id)"
        recent.removeAll { $0 == key }
        recent.append(key)
        if let hit = cache[key] { return hit }
        let model = LibraryModel(id: id, scope: scope, kind: section.type ?? "")
        cache[key] = model
        while recent.count > 8 { cache[recent.removeFirst()] = nil }
        return model
    }

    // MARK: Derived

    var types: [(Int, String)] { Self.types[kind] ?? [] }
    var watchable: Bool { kind != "artist" && kind != "photo" }

    /// The "unwatched" boolean filter is the Unwatched toggle.
    var booleans: [FilterView] { filters.filter { $0.kind == "boolean" && $0.filter != "unwatched" } }
    var valueFilters: [FilterView] { filters.filter { $0.kind != "boolean" } }

    var pairs: [(String, String)] { toggles.map { ($0, "1") } + picked.map { ($0.filter, $0.value) } }
    var filtered: Bool { unwatched || !toggles.isEmpty || !picked.isEmpty }
    var activeFilterCount: Int { toggles.count + picked.count }

    var sortValue: String {
        guard desc else { return sort }
        return sorts.first { $0.key == sort }?.descKey ?? "\(sort):desc"
    }

    var sortOptions: [(String, String)] {
        let server = sorts.compactMap { d in d.key.map { ($0, d.title ?? $0) } }
        return server.isEmpty
            ? [("titleSort", "Title"), ("addedAt", "Date Added"), ("originallyAvailableAt", "Release Date"), ("rating", "Rating")]
            : server
    }

    var sortTitle: String { sortOptions.first { $0.0 == sort }?.1 ?? "Title" }

    /// Identity of the listing query.
    var requestKey: String {
        let filters = pairs.map { "\($0.0)=\($0.1)" }.joined(separator: "&")
        return "\(mediaType ?? 0)|\(sortValue)|\(unwatched)|\(filters)"
    }

    /// The A–Z rail only makes sense for an unfiltered title-ascending list.
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

    var shape: CardShape? {
        guard let t = mediaType else { return nil }
        return t == 4 ? .landscape : t >= 8 ? .square : nil
    }

    var noun: String {
        guard let label = types.first(where: { $0.0 == mediaType })?.1 else {
            return kind == "movie" ? "movie" : "item"
        }
        let lower = label.lowercased()
        return lower.hasSuffix("s") ? String(lower.dropLast()) : lower
    }

    // MARK: Changes

    func setType(_ t: Int) {
        guard t != mediaType else { return }
        mediaType = t
        toggles = []
        picked = []
    }

    func toggle(_ f: FilterView) {
        if toggles.contains(f.filter) { toggles.removeAll { $0 == f.filter } } else { toggles.append(f.filter) }
    }

    func pick(_ f: FilterView, value: String, label: String) {
        if let i = picked.firstIndex(where: { $0.filter == f.filter && $0.value == value }) {
            picked.remove(at: i)
        } else {
            picked.append(.init(filter: f.filter, title: f.title, value: value, label: label))
        }
    }

    func clearFilters() {
        toggles = []
        picked = []
        unwatched = false
    }

    /// Called when the query changes: the old selection and position no longer apply.
    func queryChanged() {
        clearSelection()
        focus = nil
        visibleIndexes.removeAll()
    }

    // MARK: Grid

    func appeared(_ index: Int) {
        visibleIndexes.insert(index)
        focus = visibleIndexes.min()
        // Load this page, and the next one when close to its end.
        pages.ensure(index, index + 40)
    }

    func disappeared(_ index: Int) {
        visibleIndexes.remove(index)
        if let min = visibleIndexes.min() { focus = min }
    }

    // MARK: Selection

    func isSelected(_ item: Metadata) -> Bool { selection[item.ratingKey ?? ""] != nil }

    func setSelected(_ item: Metadata, _ on: Bool) {
        guard let rk = item.ratingKey else { return }
        if on {
            if selection[rk] == nil { selectionOrder.append(rk) }
            selection[rk] = item
        } else {
            selection[rk] = nil
            selectionOrder.removeAll { $0 == rk }
        }
    }

    var selectedItems: [Metadata] { selectionOrder.compactMap { selection[$0] } }

    /// Select everything that is loaded.
    func selectLoaded() {
        for offset in pages.pages.keys.sorted() {
            for item in pages.pages[offset] ?? [] { setSelected(item, true) }
        }
    }

    func clearSelection() {
        selection = [:]
        selectionOrder = []
    }

    // MARK: Saved views

    func savedViews(_ settings: Settings?) -> [SavedView] {
        (settings?.ui.saved_views ?? []).filter { $0.scope == scope && $0.library == id }
    }

    func makeView(named name: String) -> SavedView {
        SavedView(
            id: UUID().uuidString, name: name, scope: scope, library: id,
            mediaType: mediaType ?? 0, sort: sort, desc: desc, unwatched: unwatched,
            toggles: toggles, picked: picked
        )
    }

    func apply(_ view: SavedView) {
        mediaType = view.mediaType == 0 ? nil : view.mediaType
        sort = view.sort
        desc = view.desc
        unwatched = view.unwatched
        toggles = view.toggles
        picked = view.picked
        tab = .browse
        clearSelection()
    }

    // MARK: Loading

    func loadSorts() async {
        guard sorts.isEmpty, let s = try? await API.sectionSorts(id) else { return }
        sorts = s
    }

    func loadFilters() async {
        let t = mediaType
        do {
            let f = try await API.sectionFilters(id, kind: t)
            if t == mediaType { filters = f }
        } catch {
            if t == mediaType { filters = [] }
        }
    }

    func loadChars() async {
        guard tab == .browse, railOn else { chars = []; return }
        let t = mediaType
        let c = (try? await API.firstCharacters(id, kind: t)) ?? []
        if t == mediaType { chars = c }
    }

    /// Point the page cache at the current query (and revision).
    func configurePages(revision: Int) {
        let key = requestKey
        if !pages.key.isEmpty && pages.key != key { queryChanged() }
        let id = id, kind = mediaType, sort = sortValue, unwatched = unwatched, filters = pairs
        pages.configure(key: key, revision: revision) { start, size in
            try await API.sectionItems(id, kind: kind, sort: sort, unwatched: unwatched, filters: filters, start: start, size: size)
        }
    }
}
