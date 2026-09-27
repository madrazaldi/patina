import SwiftUI

/// Add a library, or rename one and change its folders when `section` is set
/// (web/src/components/ServerLibraryDialog.svelte).
struct ServerLibraryEditor: View {
    var section: Directory?

    @Environment(AppModel.self) private var app
    @Environment(\.dismiss) private var dismiss

    static let kinds: [(String, String)] = [("movie", "Movies"), ("show", "TV Shows"), ("artist", "Music"), ("photo", "Photos")]

    @State private var name = ""
    @State private var locations: [String] = [""]
    @State private var kind = "movie"
    @State private var agent = ""
    @State private var scanner = ""
    @State private var language = "en-US"
    @State private var choices: LibraryChoices?
    @State private var choicesError: String?
    @State private var busy = false

    private var editing: Bool { section != nil }
    private var label: String { Self.kinds.first { $0.0 == kind }?.1 ?? "Library" }
    private var paths: [String] { locations.map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty } }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 4) {
                Text(editing ? "Edit \(section?.title ?? "")" : "Add Library").font(Theme.display(20))
                Text(editing ? "Rename the library or change the folders it scans." : "Point Plex at folders of media on the server.")
                    .font(.callout).foregroundStyle(.secondary)
            }
            .padding([.horizontal, .top], 20)

            Form {
                Section {
                    TextField("Name", text: $name, prompt: Text(editing ? "" : label))
                    if !editing {
                        Picker("Type", selection: $kind) {
                            ForEach(Self.kinds, id: \.0) { Text($0.1).tag($0.0) }
                        }
                        Picker("Agent", selection: $agent) {
                            if choices?.agents.isEmpty ?? true { Text("—").tag("") }
                            ForEach(choices?.agents ?? [], id: \.self) { a in
                                let id = a.first ?? ""
                                Text(a.count > 1 && !a[1].isEmpty ? a[1] : id).tag(id)
                            }
                        }
                        .disabled(choices?.agents.isEmpty ?? true)
                        Picker("Scanner", selection: $scanner) {
                            if choices?.scanners.isEmpty ?? true { Text("—").tag("") }
                            ForEach(choices?.scanners ?? [], id: \.self) { Text($0).tag($0) }
                        }
                        .disabled(choices?.scanners.isEmpty ?? true)
                        TextField("Language", text: $language, prompt: Text("en-US"))
                        if let e = choicesError {
                            Text(e).foregroundStyle(.red).font(.callout)
                        }
                    }
                }
                Section {
                    ForEach(locations.indices, id: \.self) { i in
                        HStack {
                            // Bounds-checked: removing a row must not read a stale index.
                            TextField("Folder \(i + 1)", text: Binding(
                                get: { i < locations.count ? locations[i] : "" },
                                set: { if i < locations.count { locations[i] = $0 } }
                            ), prompt: Text("/path/to/media"))
                                .labelsHidden()
                                .onSubmit(submit)
                            Button { if i < locations.count { locations.remove(at: i) } } label: { Image(systemName: "minus.circle") }
                                .buttonStyle(.borderless)
                                .disabled(locations.count == 1)
                                .help("Remove folder")
                        }
                    }
                    Button("Add Folder", systemImage: "plus") { locations.append("") }
                        .buttonStyle(.borderless)
                } header: {
                    Text("Folders")
                } footer: {
                    Text("Paths as the server sees them, e.g. /data/movies").font(Theme.caption).foregroundStyle(.secondary)
                }
            }
            .formStyle(.grouped)
            .scrollContentBackground(.hidden)

            HStack {
                Spacer()
                Button("Cancel", role: .cancel) { dismiss() }
                    .keyboardShortcut(.cancelAction)
                Button(editing ? "Save" : "Add Library", action: submit)
                    .keyboardShortcut(.defaultAction)
                    .disabled(busy || paths.isEmpty || (!editing && choices == nil))
            }
            .padding(20)
        }
        .frame(width: 520, height: editing ? 400 : 560)
        .onAppear {
            if let s = section {
                name = s.title ?? ""
                let ls = s.locations.compactMap(\.path).filter { !$0.isEmpty }
                locations = ls.isEmpty ? [""] : ls
            }
        }
        .task(id: kind) {
            // Agents and scanners depend on the library type; preselect the first of each.
            guard !editing else { return }
            choices = nil
            choicesError = nil
            let k = kind
            do {
                let c = try await API.libraryChoices(kind: k)
                guard k == kind else { return }
                choices = c
                agent = c.agents.first?.first ?? ""
                scanner = c.scanners.first ?? ""
            } catch is CancellationError {
            } catch {
                if k == kind { choicesError = error.localizedDescription }
            }
        }
    }

    private func submit() {
        guard !busy, !paths.isEmpty else { return }
        let title = name.trimmingCharacters(in: .whitespaces).isEmpty ? label : name.trimmingCharacters(in: .whitespaces)
        busy = true
        Task {
            let ok: Bool
            if let s = section {
                ok = await app.act("Library updated") { try await API.editLibrary(s.key ?? "", name: title, locations: paths) }
            } else {
                let lang = language.trimmingCharacters(in: .whitespaces)
                let form = LibraryForm(name: title, kind: kind, agent: agent, scanner: scanner, language: lang.isEmpty ? "en-US" : lang, locations: paths)
                ok = await app.act("Added “\(title)”") { try await API.createLibrary(form) }
            }
            busy = false
            if ok { dismiss() }
        }
    }
}

/// Advanced settings of one library, rendered like the server settings
/// (web/src/components/ServerLibraryPrefs.svelte).
struct ServerLibraryPrefs: View {
    let section: Directory

    @Environment(AppModel.self) private var app
    @Environment(\.dismiss) private var dismiss
    @State private var list: ServerPrefList?
    @State private var failed: String?
    @State private var advanced = false

    private var id: String { section.key ?? "" }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("\(section.title ?? "Library") Settings").font(Theme.display(20))
                    Text("Changes are saved as you make them.").font(.callout).foregroundStyle(.secondary)
                }
                Spacer()
                if list?.prefs.contains(where: \.advanced) ?? false {
                    Toggle("Show Advanced", isOn: $advanced).toggleStyle(.checkbox)
                }
            }
            .padding([.horizontal, .top], 20)

            Group {
                if let list {
                    let shown = list.prefs.filter { advanced || !$0.advanced }
                    if shown.isEmpty {
                        EmptyState(title: "This library has no settings.")
                    } else {
                        Form {
                            ForEach(shown) { p in
                                ServerPrefRow(pref: p, showId: false) { v in
                                    let ok = await app.act("Setting saved") { try await API.setLibraryPref(id, pref: p.id, value: v) }
                                    if ok { list.stored(p.id, v) }
                                    return ok
                                }
                            }
                        }
                        .formStyle(.grouped)
                        .scrollContentBackground(.hidden)
                    }
                } else if let failed {
                    ErrorState(message: failed) { self.failed = nil; Task { await load() } }
                } else {
                    ProgressView().controlSize(.small).frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            .frame(maxHeight: .infinity)

            HStack {
                Spacer()
                Button("Done") { dismiss() }.keyboardShortcut(.defaultAction)
            }
            .padding(20)
        }
        .frame(width: 680, height: 560)
        .task { await load() }
    }

    private func load() async {
        do { list = ServerPrefList(try await API.libraryPrefs(id)) } catch { failed = error.localizedDescription }
    }
}
