import SwiftUI

/// Metadata editor. Only changed fields are sent; the server locks them.
struct EditSheet: View {
    let item: Metadata
    @Environment(AppModel.self) private var app
    @Environment(\.dismiss) private var dismiss
    @State private var full: Metadata?
    @State private var error: String?
    @State private var busy = false
    @State private var form: [String: String] = [:]
    @State private var original: [String: String] = [:]

    private static let labels: [String: String] = [
        "title": "Title", "titleSort": "Sort title", "originalTitle": "Original title", "editionTitle": "Edition",
        "year": "Year", "originallyAvailableAt": "Release date", "contentRating": "Content rating",
        "studio": "Studio", "tagline": "Tagline", "summary": "Summary",
    ]

    private static let fieldsByKind: [String: [String]] = [
        "movie": ["title", "titleSort", "originalTitle", "editionTitle", "year", "originallyAvailableAt", "contentRating", "studio", "tagline", "summary"],
        "show": ["title", "titleSort", "originalTitle", "year", "originallyAvailableAt", "contentRating", "studio", "tagline", "summary"],
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

    private var fields: [String] { Self.fieldsByKind[item.kind] ?? ["title", "titleSort", "summary"] }
    private var tags: [String] { Self.tagsByKind[item.kind] ?? [] }

    private func label(_ f: String) -> String {
        if item.kind == "album" && f == "studio" { return "Record label" }
        if item.kind == "track" && f == "originalTitle" { return "Track artist" }
        return Self.labels[f] ?? f
    }

    var body: some View {
        SheetFrame(title: "Edit", subtitle: (full ?? item).fullTitle, width: 540, height: 600) {
            if let error {
                SheetMessage(text: error, error: true)
            } else if full == nil {
                SheetMessage(text: "", spinner: true)
            } else {
                Form {
                    Section {
                        ForEach(fields.filter { $0 != "summary" }, id: \.self) { f in
                            TextField(label(f), text: binding(f), prompt: prompt(f))
                        }
                    }
                    if !tags.isEmpty {
                        Section {
                            ForEach(tags, id: \.self) { t in
                                TextField(t == "genre" ? "Genres" : "Labels", text: binding(t), prompt: Text("Comma-separated"))
                            }
                        } footer: {
                            Text("Separate tags with commas.").font(.caption).foregroundStyle(.secondary)
                        }
                    }
                    if fields.contains("summary") {
                        Section("Summary") {
                            TextEditor(text: binding("summary"))
                                .font(.body)
                                .frame(minHeight: 110)
                                .labelsHidden()
                        }
                    }
                }
                .formStyle(.grouped)
                .disabled(busy)
                .onSubmit(save)
            }
        } footer: {
            if let invalid {
                Label(invalid, systemImage: "exclamationmark.triangle").font(.caption).foregroundStyle(.red)
            } else {
                Text("Edited fields are locked, so refreshing metadata won't overwrite them.")
                    .font(.caption).foregroundStyle(.secondary).lineLimit(2)
            }
            Spacer(minLength: 12)
            Button("Cancel", role: .cancel) { dismiss() }
                .keyboardShortcut(.cancelAction)
                .disabled(busy)
            Button(busy ? "Saving…" : "Save", action: save)
                .keyboardShortcut(.defaultAction)
                .disabled(!dirty || invalid != nil || busy)
        }
        .interactiveDismissDisabled(busy)
        .task { await load() }
    }

    private func prompt(_ f: String) -> Text? {
        switch f {
        case "titleSort": Text(form["title"] ?? "")
        case "originallyAvailableAt": Text("YYYY-MM-DD")
        case "year": Text("YYYY")
        default: nil
        }
    }

    private func binding(_ key: String) -> Binding<String> {
        Binding(get: { form[key] ?? "" }, set: { form[key] = $0 })
    }

    private func load() async {
        do {
            guard let m = try await API.metadata(item.ratingKey ?? "") else {
                error = "This item no longer exists on the server."
                return
            }
            original = Self.values(m)
            form = original
            full = m
        } catch {
            self.error = error.localizedDescription
        }
    }

    private static func values(_ m: Metadata) -> [String: String] {
        let tagList = { (list: [Tag]) in list.compactMap(\.tag).filter { !$0.isEmpty }.joined(separator: ", ") }
        return [
            "title": m.title ?? "", "titleSort": m.titleSort ?? "", "originalTitle": m.originalTitle ?? "",
            "editionTitle": m.editionTitle ?? "", "year": m.year.map(String.init) ?? "",
            "originallyAvailableAt": m.originallyAvailableAt ?? "", "contentRating": m.contentRating ?? "",
            "studio": m.studio ?? "", "tagline": m.tagline ?? "", "summary": m.summary ?? "",
            "genre": tagList(m.genres), "label": tagList(m.labels),
        ]
    }

    private var invalid: String? {
        let trim = { (k: String) in (form[k] ?? "").trimmingCharacters(in: .whitespacesAndNewlines) }
        let year = trim("year")
        if fields.contains("year"), !year.isEmpty, year.range(of: #"^\d{4}$"#, options: .regularExpression) == nil {
            return "Year should be four digits."
        }
        let date = trim("originallyAvailableAt")
        if fields.contains("originallyAvailableAt"), !date.isEmpty,
           date.range(of: #"^\d{4}-\d{2}-\d{2}$"#, options: .regularExpression) == nil {
            return "Release date should be YYYY-MM-DD."
        }
        return nil
    }

    private static func split(_ s: String) -> [String] {
        var seen = Set<String>(), out: [String] = []
        for t in s.split(separator: ",").map({ $0.trimmingCharacters(in: .whitespaces) }) where !t.isEmpty && seen.insert(t).inserted {
            out.append(t)
        }
        return out
    }

    private var changes: (fields: [(String, String)], tags: [TagEdit]) {
        let out = fields.compactMap { f -> (String, String)? in
            let v = (form[f] ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            return v != original[f] ? (f, v) : nil
        }
        var edits: [TagEdit] = []
        for tag in tags {
            let before = Self.split(original[tag] ?? ""), after = Self.split(form[tag] ?? "")
            let has = { (list: [String], t: String) in list.contains { $0.lowercased() == t.lowercased() } }
            let add = after.filter { !has(before, $0) }, remove = before.filter { !has(after, $0) }
            if !add.isEmpty || !remove.isEmpty { edits.append(TagEdit(tag: tag, add: add, remove: remove)) }
        }
        return (out, edits)
    }

    private var dirty: Bool {
        guard full != nil else { return false }
        let c = changes
        return !c.fields.isEmpty || !c.tags.isEmpty
    }

    private func save() {
        guard let full, dirty, invalid == nil, !busy else { return }
        let c = changes, rk = full.ratingKey ?? ""
        busy = true
        Task {
            let ok = await app.act("Saved", refresh: true) { try await API.editMetadata(rk, fields: c.fields, tags: c.tags) }
            busy = false
            if ok { dismiss() }
        }
    }
}
