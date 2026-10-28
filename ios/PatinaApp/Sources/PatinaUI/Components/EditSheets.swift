// Ports of web/src/components/EditDialog.svelte, ArtworkDialog.svelte, MatchDialog.svelte
// and SubtitleDialog.svelte: the owner's editors of an item.

import PatinaCore
import PlexKit
import SwiftUI

// MARK: Edit

struct EditSheet: View {
    let item: Metadata

    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var full: Metadata?
    @State private var error: String?
    @State private var busy = false
    @State private var form: [String: String] = [:]
    @State private var original: [String: String] = [:]
    @State private var scope = ""

    private static let labels: [String: String] = [
        "title": "Title", "titleSort": "Sort Title", "originalTitle": "Original Title", "editionTitle": "Edition",
        "year": "Year", "originallyAvailableAt": "Release Date", "contentRating": "Content Rating", "studio": "Studio",
        "tagline": "Tagline", "summary": "Summary",
    ]
    private static let fieldsByKind: [String: [String]] = [
        "movie": [
            "title", "titleSort", "originalTitle", "editionTitle", "year", "originallyAvailableAt", "contentRating",
            "studio", "tagline", "summary",
        ],
        "show": [
            "title", "titleSort", "originalTitle", "year", "originallyAvailableAt", "contentRating", "studio", "tagline",
            "summary",
        ],
        "season": ["title", "summary"],
        "episode": ["title", "titleSort", "originallyAvailableAt", "summary"],
        "artist": ["title", "titleSort", "summary"],
        "album": ["title", "titleSort", "year", "originallyAvailableAt", "studio", "summary"],
        "track": ["title", "titleSort", "originalTitle"],
        "collection": ["title", "titleSort", "contentRating", "summary"],
    ]
    private static let tagsByKind: [String: [String]] = [
        "movie": ["genre", "label"], "show": ["genre", "label"], "artist": ["genre", "label"],
        "album": ["genre", "label"], "collection": ["label"],
    ]

    private var kind: String { Metadata.kind(item) }
    private var fields: [String] { Self.fieldsByKind[kind] ?? ["title", "titleSort", "summary"] }
    private var tags: [String] { Self.tagsByKind[kind] ?? [] }

    private func label(_ f: String) -> String {
        if kind == "album" && f == "studio" { return "Record Label" }
        if kind == "track" && f == "originalTitle" { return "Track Artist" }
        return Self.labels[f] ?? f
    }

    private static func values(_ m: Metadata) -> [String: String] {
        let tags = { (l: [Tag]) in l.map(Tag.name).filter { !$0.isEmpty }.joined(separator: ", ") }
        return [
            "title": m.title ?? "", "titleSort": m.titleSort ?? "", "originalTitle": m.originalTitle ?? "",
            "editionTitle": m.editionTitle ?? "", "year": m.year.map(String.init) ?? "",
            "originallyAvailableAt": m.originallyAvailableAt ?? "", "contentRating": m.contentRating ?? "",
            "studio": m.studio ?? "", "tagline": m.tagline ?? "", "summary": m.summary ?? "", "genre": tags(m.genre),
            "label": tags(m.label),
        ]
    }

    private static func split(_ s: String) -> [String] {
        var seen: [String] = []
        for t in s.split(separator: ",").map({ $0.trimmingCharacters(in: .whitespaces) }) where !t.isEmpty && !seen.contains(t) {
            seen.append(t)
        }
        return seen
    }

    private var invalid: String? {
        let year = form["year", default: ""].trimmingCharacters(in: .whitespaces)
        if fields.contains("year"), !year.isEmpty, year.count != 4 || !year.allSatisfy(\.isNumber) {
            return "Year should be four digits."
        }
        let date = form["originallyAvailableAt", default: ""].trimmingCharacters(in: .whitespaces)
        if fields.contains("originallyAvailableAt"), !date.isEmpty,
            date.range(of: #"^\d{4}-\d{2}-\d{2}$"#, options: .regularExpression) == nil
        {
            return "Release date should be YYYY-MM-DD."
        }
        return nil
    }

    private func changes() -> (fields: [(String, String)], tags: [Ops.Edit.TagEdit]) {
        let out = fields.compactMap { f -> (String, String)? in
            let v = form[f, default: ""].trimmingCharacters(in: .whitespacesAndNewlines)
            return v != original[f, default: ""] ? (f, v) : nil
        }
        var edits: [Ops.Edit.TagEdit] = []
        for tag in tags {
            let before = Self.split(original[tag, default: ""])
            let after = Self.split(form[tag, default: ""])
            let has = { (list: [String], t: String) in list.contains { $0.lowercased() == t.lowercased() } }
            let add = after.filter { !has(before, $0) }
            let remove = before.filter { !has(after, $0) }
            if !add.isEmpty || !remove.isEmpty { edits.append(Ops.Edit.TagEdit(tag: tag, add: add, remove: remove)) }
        }
        return (out, edits)
    }

    private var dirty: Bool {
        guard full != nil else { return false }
        let c = changes()
        return !c.fields.isEmpty || !c.tags.isEmpty
    }

    private func binding(_ key: String) -> Binding<String> {
        Binding(get: { form[key, default: ""] }, set: { form[key] = $0 })
    }

    var body: some View {
        NavigationStack {
            Form {
                if let error {
                    Text(error).foregroundStyle(Theme.danger).textSelection(.enabled)
                } else if full == nil {
                    ProgressView().frame(maxWidth: .infinity)
                } else {
                    Section {
                        ForEach(fields.filter { $0 != "summary" }, id: \.self) { f in
                            LabeledContent(label(f)) {
                                TextField(f == "titleSort" ? form["title", default: ""] : "", text: binding(f))
                                    .multilineTextAlignment(.trailing)
                                    .keyboardType(f == "year" ? .numberPad : .default)
                                    .textInputAutocapitalization(f == "originallyAvailableAt" ? .never : .sentences)
                            }
                        }
                    } footer: {
                        if fields.contains("originallyAvailableAt") { Text("Release date as YYYY-MM-DD.") }
                    }
                    if !tags.isEmpty {
                        Section {
                            ForEach(tags, id: \.self) { t in
                                LabeledContent(t == "genre" ? "Genres" : "Labels") {
                                    TextField("Comma-separated", text: binding(t)).multilineTextAlignment(.trailing)
                                }
                            }
                        }
                    }
                    if fields.contains("summary") {
                        Section("Summary") {
                            TextEditor(text: binding("summary")).frame(minHeight: 140)
                        }
                    }
                    Section {
                    } footer: {
                        Text(invalid ?? "Edited fields are locked, so refreshing metadata won't overwrite them.")
                            .foregroundStyle(invalid != nil ? Theme.danger : Theme.secondaryText)
                    }
                }
            }
            .disabled(busy)
            .navigationTitle(Metadata.fullTitle(full ?? item))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() }.disabled(busy) }
                ToolbarItem(placement: .confirmationAction) {
                    if busy {
                        ProgressView()
                    } else {
                        Button("Save", action: save).disabled(!dirty || invalid != nil)
                    }
                }
            }
            .interactiveDismissDisabled(dirty || busy)
            .task {
                scope = store.scope
                do {
                    // Menu items may be partial, so edit the full item.
                    guard let m = try await Commands.metadata(store.app, ratingKey: Metadata.ratingKey(item)) else {
                        throw CoreError("This item no longer exists on the server.")
                    }
                    original = Self.values(m)
                    form = original
                    full = m
                } catch {
                    self.error = errorText(error)
                }
            }
        }
    }

    private func save() {
        guard let full, dirty, invalid == nil else { return }
        // The editor holds a rating key from the server it was opened on; never send it to another one.
        guard scope == store.scope else {
            dismiss()
            return
        }
        let c = changes()
        busy = true
        Task {
            let ok = await store.act("Saved", refresh: true) {
                try await PendingCommands.editMetadata(
                    store.app, ratingKey: Metadata.ratingKey(full), fields: c.fields, tags: c.tags)
            }
            busy = false
            if ok {
                Haptics.success()
                dismiss()
            }
        }
    }
}

// MARK: Artwork

struct ArtworkSheet: View {
    let item: Metadata
    let kind: PatinaCore.Commands.ArtworkKind

    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var choices: [Metadata]?
    @State private var error: String?
    @State private var busy: String?
    @State private var scope = ""

    private var art: Bool { kind == .art }
    private var square: Bool { !art && ["artist", "album"].contains(Metadata.kind(item)) }

    var body: some View {
        NavigationStack {
            ScrollView {
                if let error {
                    StatusView(error: error)
                } else if let choices {
                    if choices.isEmpty {
                        StatusView(title: "Nothing to choose", text: "No \(art ? "backgrounds" : "posters") available.")
                    } else {
                        LazyVGrid(columns: [GridItem(.adaptive(minimum: art ? 170 : 104), spacing: 14)], spacing: 18) {
                            ForEach(Array(choices.enumerated()), id: \.offset) { _, a in choice(a) }
                        }
                        .padding()
                    }
                } else {
                    ProgressView().padding(.top, 60)
                }
            }
            .navigationTitle(art ? "Choose Background" : "Choose Poster")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() }.disabled(busy != nil) }
            }
            .task {
                scope = store.scope
                do {
                    choices = try await Commands.artwork(store.app, ratingKey: Metadata.ratingKey(item), kind: kind)
                } catch {
                    self.error = errorText(error)
                }
            }
        }
    }

    private func choice(_ a: Metadata) -> some View {
        let url = a.key ?? a.ratingKey
        let selected = PlexUI.isSelected(a)
        let (w, h) = art ? (480, 270) : square ? (300, 300) : (240, 360)
        return Button {
            pick(a)
        } label: {
            VStack(alignment: .leading, spacing: 6) {
                Color.clear
                    .aspectRatio(art ? 16 / 9 : square ? 1 : 2 / 3, contentMode: .fit)
                    .overlay { Artwork(a.thumb ?? a.key, width: w, height: h) }
                    .overlay {
                        if busy != nil && busy == url { ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity).background(.black.opacity(0.45)) }
                    }
                    .clipShape(.rect(cornerRadius: 8, style: .continuous))
                    .overlay {
                        RoundedRectangle(cornerRadius: 8, style: .continuous)
                            .strokeBorder(selected ? store.accent : Theme.hairline, lineWidth: selected ? 2.5 : 1)
                    }
                Text(selected ? "Current" : (PlexUI.extra(a, "provider") ?? ""))
                    .font(.caption)
                    .foregroundStyle(selected ? store.accent : Theme.secondaryText)
                    .lineLimit(1)
            }
        }
        .buttonStyle(CardButtonStyle())
        .disabled(busy != nil)
    }

    private func pick(_ a: Metadata) {
        guard let url = a.key ?? a.ratingKey, busy == nil else { return }
        guard scope == store.scope else {
            dismiss()
            return
        }
        busy = url
        Task {
            let ok = await store.act(art ? "Background updated" : "Poster updated", refresh: true) {
                try await Commands.setArtwork(store.app, ratingKey: Metadata.ratingKey(item), kind: kind, url: url)
            }
            busy = nil
            if ok {
                Haptics.success()
                dismiss()
            }
        }
    }
}

// MARK: Match

struct MatchSheet: View {
    let item: Metadata

    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var title = ""
    @State private var year = ""
    @State private var results: [MatchResult]?
    @State private var error: String?
    @State private var busy = false
    @State private var generation = 0
    @State private var confirmUnmatch = false
    @State private var scope = ""

    private var square: Bool { ["artist", "album"].contains(Metadata.kind(item)) }

    var body: some View {
        NavigationStack {
            List {
                Section {
                    TextField("Title", text: $title).submitLabel(.search).onSubmit { search(manual: true) }
                    HStack {
                        TextField("Year", text: $year).keyboardType(.numberPad)
                        Button("Search") { search(manual: true) }
                            .disabled(title.trimmingCharacters(in: .whitespaces).isEmpty)
                    }
                }
                Section("Matches") {
                    if let error {
                        Text(error).foregroundStyle(Theme.danger).textSelection(.enabled)
                    } else if let results {
                        if results.isEmpty {
                            Text("No matches. Try a different title or year.").foregroundStyle(Theme.secondaryText)
                        }
                        ForEach(Array(results.enumerated()), id: \.offset) { _, r in
                            Button { apply(r) } label: { row(r) }
                                .disabled(busy)
                        }
                    } else {
                        ProgressView().frame(maxWidth: .infinity)
                    }
                }
                Section {
                    Button("Unmatch", role: .destructive) { confirmUnmatch = true }.disabled(busy)
                }
            }
            .navigationTitle("Fix Match")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() }.disabled(busy) }
            }
            .alert("Unmatch this item?", isPresented: $confirmUnmatch) {
                Button("Cancel", role: .cancel) {}
                Button("Unmatch", role: .destructive, action: unmatch)
            } message: {
                Text("Its metadata and artwork are cleared until it's matched again.")
            }
            .task {
                scope = store.scope
                title = Metadata.title(item)
                year = item.year.map(String.init) ?? ""
                search(manual: false)
            }
        }
    }

    private func row(_ r: MatchResult) -> some View {
        HStack(alignment: .top, spacing: 12) {
            Artwork(r.thumb, width: 96, height: square ? 96 : 144)
                .frame(width: square ? 48 : 36, height: square ? 48 : 54)
                .clipShape(.rect(cornerRadius: 5, style: .continuous))
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 6) {
                    Text(r.name ?? "").foregroundStyle(.primary)
                    if let y = r.year { Text("(\(String(y)))").foregroundStyle(Theme.secondaryText) }
                    if r.matched {
                        Text("Current").font(.caption2.weight(.semibold)).foregroundStyle(store.accent)
                            .padding(.horizontal, 6).padding(.vertical, 2)
                            .background(store.accent.opacity(0.15), in: .capsule)
                    }
                }
                if let s = r.summary {
                    Text(s).font(.caption).foregroundStyle(Theme.secondaryText).lineLimit(2)
                }
            }
            Spacer()
            if let score = r.score {
                Text("\(score)%").font(.caption.monospacedDigit()).foregroundStyle(Theme.secondaryText)
            }
        }
    }

    private func search(manual: Bool) {
        generation += 1
        let run = generation
        results = nil
        error = nil
        let y = Int(year.trimmingCharacters(in: .whitespaces))
        let q = manual ? title.trimmingCharacters(in: .whitespaces) : nil
        Task {
            do {
                let r = try await Commands.matches(
                    store.app, ratingKey: Metadata.ratingKey(item), title: q?.isEmpty == true ? nil : q,
                    year: manual ? y : nil)
                if run == generation && scope == store.scope { results = r }
            } catch {
                if run == generation { self.error = errorText(error) }
            }
        }
    }

    private func apply(_ r: MatchResult) {
        guard let guid = r.guid, !busy else { return }
        guard scope == store.scope else {
            dismiss()
            return
        }
        busy = true
        Task {
            let ok = await store.act("Matched to “\(r.name ?? "")”", refresh: true) {
                try await Commands.applyMatch(store.app, ratingKey: Metadata.ratingKey(item), guid: guid, name: r.name)
            }
            busy = false
            if ok { dismiss() }
        }
    }

    private func unmatch() {
        guard scope == store.scope else { return }
        Task {
            if await store.act("Unmatched", refresh: true, { try await Commands.unmatch(store.app, ratingKey: Metadata.ratingKey(item)) }) {
                dismiss()
            }
        }
    }
}

// MARK: Subtitles

struct SubtitleSheet: View {
    let item: Metadata

    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var language = "en"
    @State private var hearingImpaired = false
    @State private var forced = false
    @State private var results: [MediaStream]?
    @State private var error: String?
    @State private var busy: String?
    @State private var generation = 0
    @State private var scope = ""

    /// ISO 639-1, ISO 639-2 (as Plex reports stream languages), name.
    static let languages: [(String, String, String)] = [
        ("ar", "ara", "Arabic"), ("bg", "bul", "Bulgarian"), ("zh", "chi", "Chinese"), ("hr", "hrv", "Croatian"),
        ("cs", "cze", "Czech"), ("da", "dan", "Danish"), ("nl", "dut", "Dutch"), ("en", "eng", "English"),
        ("et", "est", "Estonian"), ("fi", "fin", "Finnish"), ("fr", "fre", "French"), ("de", "ger", "German"),
        ("el", "gre", "Greek"), ("he", "heb", "Hebrew"), ("hi", "hin", "Hindi"), ("hu", "hun", "Hungarian"),
        ("is", "ice", "Icelandic"), ("id", "ind", "Indonesian"), ("it", "ita", "Italian"), ("ja", "jpn", "Japanese"),
        ("ko", "kor", "Korean"), ("lv", "lav", "Latvian"), ("lt", "lit", "Lithuanian"), ("ms", "may", "Malay"),
        ("no", "nor", "Norwegian"), ("fa", "per", "Persian"), ("pl", "pol", "Polish"), ("pt", "por", "Portuguese"),
        ("ro", "rum", "Romanian"), ("ru", "rus", "Russian"), ("sr", "srp", "Serbian"), ("sk", "slo", "Slovak"),
        ("sl", "slv", "Slovenian"), ("es", "spa", "Spanish"), ("sv", "swe", "Swedish"), ("th", "tha", "Thai"),
        ("tr", "tur", "Turkish"), ("uk", "ukr", "Ukrainian"), ("vi", "vie", "Vietnamese"),
    ]
    /// ISO 639-2/T variants Plex sometimes uses.
    static let aliases: [String: String] = [
        "zho": "zh", "ces": "cs", "nld": "nl", "fra": "fr", "deu": "de", "ell": "el", "isl": "is", "msa": "ms",
        "fas": "fa", "ron": "ro", "slk": "sk",
    ]

    static func iso1(_ code: String?) -> String? {
        guard let c = code?.lowercased() else { return nil }
        return languages.first { $0.0 == c || $0.1 == c }?.0 ?? aliases[c]
    }

    static func defaultLanguage(_ m: Metadata) -> String {
        guard let part = m.media.first?.part.first else { return "en" }
        return iso1(Part.selectedSubtitle(part)?.languageCode) ?? iso1(Part.selectedAudio(part)?.languageCode) ?? "en"
    }

    var body: some View {
        NavigationStack {
            List {
                Section {
                    Picker("Language", selection: $language) {
                        ForEach(Self.languages, id: \.0) { Text($0.2).tag($0.0) }
                    }
                    Toggle("Hearing Impaired", isOn: $hearingImpaired)
                    Toggle("Forced Only", isOn: $forced)
                }
                Section {
                    if let error {
                        Text(error).foregroundStyle(Theme.danger).textSelection(.enabled)
                    } else if let results {
                        if results.isEmpty {
                            Text("Nothing found for this language.").foregroundStyle(Theme.secondaryText)
                        }
                        ForEach(Array(results.enumerated()), id: \.offset) { _, s in row(s) }
                    } else {
                        HStack(spacing: 10) {
                            ProgressView()
                            Text("Searching subtitle providers…").foregroundStyle(Theme.secondaryText)
                        }
                    }
                }
            }
            .navigationTitle("Search Subtitles")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() }.disabled(busy != nil) }
            }
            .onChange(of: language) { search() }
            .onChange(of: hearingImpaired) { search() }
            .onChange(of: forced) { search() }
            .task {
                scope = store.scope
                let preferred = Self.defaultLanguage(item)
                // A changed language searches through `onChange`.
                if preferred != language { language = preferred } else { search() }
            }
        }
    }

    private func row(_ s: MediaStream) -> some View {
        HStack(spacing: 10) {
            VStack(alignment: .leading, spacing: 2) {
                Text(s.displayTitle ?? s.title ?? "Subtitle").lineLimit(1)
                let detail = [s.providerTitle, s.title != s.displayTitle ? s.title : nil].compactMap { $0 }
                if !detail.isEmpty {
                    Text(detail.joined(separator: " · ")).font(.caption).foregroundStyle(Theme.secondaryText).lineLimit(1)
                }
                HStack(spacing: 6) {
                    if s.perfectMatch { tag("Perfect match", accent: true) }
                    if s.hearingImpaired { tag("SDH") }
                    if s.forced { tag("Forced") }
                    if let score = s.score { Text("\(score)").font(.caption2.monospacedDigit()).foregroundStyle(Theme.tertiaryText) }
                }
            }
            Spacer()
            if s.downloaded {
                Text("Downloaded").font(.caption).foregroundStyle(Theme.secondaryText)
            } else if busy != nil && busy == s.key {
                ProgressView()
            } else {
                Button("Get") { fetch(s) }
                    .buttonStyle(.bordered)
                    .disabled(s.key == nil || busy != nil)
            }
        }
    }

    private func tag(_ text: String, accent: Bool = false) -> some View {
        Text(text)
            .font(.caption2.weight(.semibold))
            .foregroundStyle(accent ? store.accent : Theme.secondaryText)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .overlay(Capsule().strokeBorder(accent ? store.accent.opacity(0.5) : Theme.hairline))
    }

    private func search() {
        generation += 1
        let run = generation
        results = nil
        error = nil
        let (lang, hi, f) = (language, hearingImpaired, forced)
        Task {
            do {
                let r = try await Commands.searchSubtitles(
                    store.app, ratingKey: Metadata.ratingKey(item), language: lang, hearingImpaired: hi, forced: f)
                if run == generation && scope == store.scope { results = r }
            } catch {
                if run == generation { self.error = errorText(error) }
            }
        }
    }

    private func fetch(_ s: MediaStream) {
        guard let key = s.key, busy == nil else { return }
        guard scope == store.scope else {
            dismiss()
            return
        }
        busy = key
        Task {
            let ok = await store.act("Subtitle downloaded", refresh: true) {
                try await Commands.downloadSubtitle(store.app, ratingKey: Metadata.ratingKey(item), key: key)
            }
            busy = nil
            if ok { dismiss() }
        }
    }
}
