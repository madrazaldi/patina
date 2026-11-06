// Port of web/src/pages/Server.svelte (with ServerLibraryDialog, ServerLibraryPrefs,
// ServerPrefRow, StatsBandwidth and StatsRanking): the owner's tools for the connected server.
// Overview with updates and streaming statistics, libraries, maintenance and diagnostics,
// the server's settings, and its users and devices.

import Charts
import PatinaCore
import PlexKit
import SwiftUI

struct ServerScreen: View {
    enum Tab: String, CaseIterable {
        case overview = "Overview"
        case libraries = "Libraries"
        case maintenance = "Maintenance"
        case settings = "Settings"
        case users = "Users & Devices"
    }

    @Environment(AppStore.self) private var store
    @State private var tab = Tab.overview

    var body: some View {
        Group {
            if !store.owned {
                StatusView(
                    title: "Server tools need owner access", text: "This server is shared with you.",
                    systemImage: "lock")
            } else {
                List {
                    Section {
                        PillTabs(selection: $tab, options: Tab.allCases.map { ($0, $0.rawValue) })
                            .listRowBackground(Color.clear)
                            .listRowInsets(EdgeInsets())
                    }
                    switch tab {
                    case .overview: ServerOverview()
                    case .libraries: ServerLibraries()
                    case .maintenance: ServerMaintenance()
                    case .settings: ServerSettings()
                    case .users: ServerUsers()
                    }
                }
            }
        }
        .navigationTitle(store.session?.serverName ?? "Server")
    }
}

// MARK: Overview

private struct ServerOverview: View {
    enum Period: String, CaseIterable {
        case day = "Last 24 Hours"
        case month = "Last 30 Days"
    }

    struct StatsData {
        var stats: Ops.Server.Stats
        var since: Int
        var step: Int
    }

    @Environment(AppStore.self) private var store
    @State private var overview = Loader<Ops.Server.Overview>()
    @State private var stats = Loader<StatsData>()
    @State private var period = Period.day

    var body: some View {
        Group {
            if let o = overview.value {
                Section {
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 130), spacing: 12)], spacing: 12) {
                        meter("Host CPU", o.hostCpu)
                        meter("Plex CPU", o.plexCpu)
                        meter("Host Memory", o.hostMemory)
                        meter("Plex Memory", o.plexMemory)
                    }
                    .padding(.vertical, 4)
                }
                Section("Server") {
                    LabeledContent("Version", value: o.version ?? "—")
                    LabeledContent("Platform", value: o.platform ?? "—")
                    LabeledContent("Owner", value: o.owner ?? "—")
                    LabeledContent("Plex Pass", value: o.plexPass ? "Active" : "No")
                    LabeledContent("Connected Via") { Text(o.connectedVia).textSelection(.enabled).lineLimit(1) }
                    LabeledContent("Machine ID") {
                        Text(o.machineIdentifier ?? "—").textSelection(.enabled).lineLimit(1).truncationMode(.middle)
                    }
                    LabeledContent("Transcoder", value: o.transcoder ? "Available" : "—")
                }
                updates(o)
            } else {
                LoaderStatus(error: overview.error) { overview.reload() }
            }
            streaming
        }
        .task { await overview.load { try await Commands.serverOverview(store.app) } }
        .task(id: period) {
            let app = store.app!
            let now = Int(Date().timeIntervalSince1970)
            // Hourly buckets for a day, daily ones for a month.
            let (timespan, step) = period == .day ? (6, 3600) : (4, 86400)
            let since = period == .day ? now - 86400 : now - 30 * 86400
            await stats.load {
                StatsData(stats: try await Commands.serverStats(app, timespan: timespan, since: since), since: since, step: step)
            }
        }
    }

    private func meter(_ label: String, _ value: Double) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(label).font(.caption).foregroundStyle(Theme.secondaryText)
            Text("\(Int(value.rounded()))%").font(.title2.weight(.semibold)).monospacedDigit()
            ProgressView(value: min(100, max(0, value)), total: 100).tint(store.accent)
        }
    }

    private func updates(_ o: Ops.Server.Overview) -> some View {
        Section("Updates") {
            if let release = o.release {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Version \(release.version ?? "") is available.")
                    if let added = release.added, !added.isEmpty {
                        Text(added).font(.caption).foregroundStyle(Theme.secondaryText).lineLimit(6)
                    }
                }
                if o.canInstall {
                    Button("Install Now") {
                        store.perform("Installing update…") { try await Commands.applyUpdate(store.app) }
                    }
                    Button("Install Tonight") {
                        store.perform("Update scheduled for tonight") { try await Commands.applyUpdate(store.app, tonight: true) }
                    }
                } else if let url = release.downloadURL {
                    Button("Download", systemImage: "arrow.up.right.square") {
                        store.perform { try await store.host.openUrl(url) }
                    }
                }
            } else {
                Text("The server is up to date.").foregroundStyle(Theme.secondaryText)
            }
            Button("Check Now") {
                Task {
                    if await store.act("Checked for updates", { try await Commands.checkForUpdates(store.app) }) {
                        overview.reload()
                    }
                }
            }
        }
    }

    @ViewBuilder
    private var streaming: some View {
        Section {
            Picker("Period", selection: $period) {
                ForEach(Period.allCases, id: \.self) { Text($0.rawValue) }
            }
            .pickerStyle(.segmented)
            if let d = stats.value {
                let s = d.stats
                if s.bandwidth.isEmpty && s.plays == 0 {
                    Text(
                        "Nothing was streamed from this server \(period == .day ? "in the last 24 hours" : "in the last 30 days")."
                    )
                    .foregroundStyle(Theme.secondaryText)
                } else {
                    HStack {
                        figure(Format.bytes(Int64(s.bandwidth.reduce(0) { $0 + $1.lan + $1.wan })), "Data Sent")
                        figure(Format.count(s.plays), "Plays")
                        figure(hours(s.playSeconds), "Hours Played")
                    }
                    if !s.bandwidth.isEmpty { BandwidthChart(data: d, accent: store.accent) }
                }
            } else {
                LoaderStatus(error: stats.error) { stats.reload() }
            }
        } header: {
            Text("Streaming")
        }
        .opacity(stats.loading && stats.value != nil ? 0.5 : 1)
        if let s = stats.value?.stats, !s.bandwidth.isEmpty || s.plays > 0 {
            Section("Top Users") { RankingChart(rows: s.accounts, empty: "No data sent to users.", accent: store.accent) }
            Section("Top Devices") {
                RankingChart(rows: s.devices, empty: "No data sent to devices.", accent: store.accent)
            }
        }
    }

    private func figure(_ value: String, _ label: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label).font(.caption).foregroundStyle(Theme.secondaryText)
            Text(value).font(.title3.weight(.semibold)).monospacedDigit().lineLimit(1).minimumScaleFactor(0.7)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    /// "12.5" hours below ten, whole hours above.
    private func hours(_ secs: Int) -> String {
        let h = Double(secs) / 3600
        return h < 10 ? String(format: "%.1f", h) : Format.count(Int(h.rounded()))
    }
}

/// Bytes sent per time bucket, LAN and WAN stacked (StatsBandwidth.svelte).
private struct BandwidthChart: View {
    let data: ServerOverview.StatsData
    let accent: Color

    private struct Point: Identifiable {
        var id: String { "\(at)-\(network)" }
        var at: Date
        var network: String
        var bytes: Int
    }

    private var points: [Point] {
        data.stats.bandwidth.flatMap { b in
            let at = Date(timeIntervalSince1970: TimeInterval(b.at))
            return [Point(at: at, network: "Local", bytes: b.lan), Point(at: at, network: "Remote", bytes: b.wan)]
        }
    }

    var body: some View {
        Chart(points) { p in
            BarMark(
                x: .value("Time", p.at, unit: data.step == 3600 ? .hour : .day),
                y: .value("Data", p.bytes))
                .foregroundStyle(by: .value("Network", p.network))
        }
        .chartForegroundStyleScale(["Local": accent, "Remote": accent.opacity(0.45)])
        .chartXScale(
            domain: Date(timeIntervalSince1970: TimeInterval(data.since))...Date(
                timeIntervalSince1970: TimeInterval(data.since + (data.step == 3600 ? 86400 : 30 * 86400))))
        .chartYAxis {
            AxisMarks { v in
                AxisGridLine()
                AxisValueLabel { if let n = v.as(Int.self) { Text(Format.bytes(Int64(n))) } }
            }
        }
        .frame(height: 180)
        .padding(.vertical, 6)
        .accessibilityLabel("Data sent over time")
    }
}

/// The accounts or devices that streamed the most (StatsRanking.svelte).
private struct RankingChart: View {
    let rows: [Ops.Server.Usage]
    let empty: String
    let accent: Color

    var body: some View {
        if rows.isEmpty {
            Text(empty).foregroundStyle(Theme.secondaryText)
        } else {
            let top = Array(rows.prefix(8))
            Chart(Array(top.enumerated()), id: \.offset) { _, r in
                BarMark(x: .value("Data", r.bytes), y: .value("Name", r.name))
                    .foregroundStyle(accent)
                    .annotation(position: .trailing) {
                        Text(Format.bytes(Int64(r.bytes))).font(.caption2).foregroundStyle(Theme.secondaryText)
                    }
            }
            .chartXAxis(.hidden)
            .frame(height: CGFloat(top.count) * 34 + 8)
            .padding(.vertical, 4)
        }
    }
}

// MARK: Libraries

private struct ServerLibraries: View {
    @Environment(AppStore.self) private var store
    @State private var editing: LibraryEdit?
    @State private var prefsFor: Directory?

    struct LibraryEdit: Identifiable {
        let id = UUID()
        var section: Directory?
    }

    var body: some View {
        Section {
            if store.sections.isEmpty {
                Text("This server has no libraries yet.").foregroundStyle(Theme.secondaryText)
            }
            ForEach(store.sections, id: \.key) { s in row(s) }
        } header: {
            Text("Libraries")
        } footer: {
            Text("Scan for new files, change folders and settings, or add a library.")
        }
        Section {
            Button("Add Library…", systemImage: "plus") { editing = LibraryEdit() }
            Button("Scan All Libraries", systemImage: "arrow.clockwise") {
                store.perform("Scanning all libraries…") { try await Commands.serverAction(store.app, action: .scanAll) }
            }
            Button("Reload List", systemImage: "list.bullet") {
                store.perform { try await Commands.reloadSections(store.app) }
            }
        }
        .sheet(item: $editing) { e in LibraryEditSheet(section: e.section) }
        .sheet(item: Binding(get: { prefsFor.map { LibraryEdit(section: $0) } }, set: { if $0 == nil { prefsFor = nil } })) { e in
            if let s = e.section { LibraryPrefsSheet(section: s) }
        }
    }

    private func row(_ s: Directory) -> some View {
        let id = s.key ?? ""
        let task = store.activities.first { $0.sectionId == id }
        return VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(s.title ?? "").font(.headline)
                Spacer()
                if task != nil || s.refreshing {
                    Button("Cancel") {
                        store.perform("Scan cancelled") { try await Commands.cancelScan(store.app, id: id) }
                    }
                    .buttonStyle(.bordered).controlSize(.small)
                } else {
                    Button("Scan") {
                        store.perform("Scanning…") { try await Commands.sectionAction(store.app, id: id, action: .scan) }
                    }
                    .buttonStyle(.bordered).controlSize(.small)
                }
                Menu {
                    Button("Edit…", systemImage: "pencil") { editing = LibraryEdit(section: s) }
                    Button("Settings…", systemImage: "slider.horizontal.3") { prefsFor = s }
                    Divider()
                    Button("Refresh All Metadata", systemImage: "arrow.triangle.2.circlepath") {
                        run(id, .refresh, "Refreshing all metadata…")
                    }
                    Button("Analyze Media", systemImage: "waveform.badge.magnifyingglass") { run(id, .analyze, "Analyzing…") }
                    Button("Empty Trash", systemImage: "trash.slash") { run(id, .emptyTrash, "Trash emptied") }
                    Divider()
                    Button("Delete Library…", systemImage: "trash", role: .destructive) { remove(s) }
                } label: {
                    Image(systemName: "ellipsis.circle").font(.title3).frame(width: 36, height: 32)
                }
                .accessibilityLabel("More actions for \(s.title ?? "")")
            }
            Text(s.location.compactMap(\.path).joined(separator: ", "))
                .font(.caption).foregroundStyle(Theme.secondaryText).lineLimit(2)
            if let task {
                Text("\(task.subtitle ?? task.title) · \(task.progress)%").font(.caption).foregroundStyle(store.accent)
                ProgressView(value: Double(task.progress), total: 100).tint(store.accent)
            } else if s.refreshing {
                Text("Scanning…").font(.caption).foregroundStyle(Theme.secondaryText)
            } else if let at = s.scannedAt {
                Text("Last scanned \(Format.ago(Int64(at)))").font(.caption).foregroundStyle(Theme.secondaryText)
            }
        }
        .padding(.vertical, 4)
    }

    private func run(_ id: String, _ action: Commands.SectionAction, _ ok: String) {
        store.perform(ok) { try await Commands.sectionAction(store.app, id: id, action: action) }
    }

    private func remove(_ s: Directory) {
        Task {
            let ok = await store.confirm(
                "Delete “\(s.title ?? "")”?",
                text: "The library and its metadata are removed from the server. Media files on disk are not touched.",
                action: "Delete Library", danger: true)
            if ok { await store.act("Deleted “\(s.title ?? "")”") { try await Commands.deleteLibrary(store.app, id: s.key ?? "") } }
        }
    }
}

/// Add a library, or rename one and change its folders (ServerLibraryDialog.svelte).
private struct LibraryEditSheet: View {
    let section: Directory?

    static let kinds = [("movie", "Movies"), ("show", "TV Shows"), ("artist", "Music"), ("photo", "Photos")]

    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var locations: [String] = [""]
    @State private var kind = "movie"
    @State private var agent = ""
    @State private var scanner = ""
    @State private var language = "en-US"
    @State private var choices: Ops.Server.Choices?
    @State private var choicesError: String?
    @State private var busy = false
    /// Library ids and folders belong to the server this opened on.
    @State private var scope = ""

    private var label: String { Self.kinds.first { $0.0 == kind }?.1 ?? "Library" }
    private var paths: [String] {
        locations.map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField(section == nil ? label : "Name", text: $name)
                } header: {
                    Text("Name")
                } footer: {
                    Text(section == nil ? "Point Plex at folders of media on the server." : "Rename the library or change the folders it scans.")
                }
                if section == nil {
                    Section {
                        Picker("Type", selection: $kind) {
                            ForEach(Self.kinds, id: \.0) { Text($0.1).tag($0.0) }
                        }
                        Picker("Agent", selection: $agent) {
                            ForEach(choices?.agents ?? [], id: \.identifier) { Text($0.title.isEmpty ? $0.identifier : $0.title).tag($0.identifier) }
                        }
                        .disabled(choices?.agents.isEmpty ?? true)
                        Picker("Scanner", selection: $scanner) {
                            ForEach(choices?.scanners ?? [], id: \.self) { Text($0).tag($0) }
                        }
                        .disabled(choices?.scanners.isEmpty ?? true)
                        LabeledContent("Language") {
                            TextField("en-US", text: $language).multilineTextAlignment(.trailing)
                                .textInputAutocapitalization(.never).autocorrectionDisabled()
                        }
                        if let choicesError { Text(choicesError).foregroundStyle(Theme.danger) }
                    }
                }
                Section {
                    ForEach(locations.indices, id: \.self) { i in
                        TextField("/path/to/media", text: $locations[i])
                            .textInputAutocapitalization(.never).autocorrectionDisabled()
                    }
                    .onDelete { at in
                        locations.remove(atOffsets: at)
                        if locations.isEmpty { locations = [""] }
                    }
                    Button("Add Folder", systemImage: "plus") { locations.append("") }
                } header: {
                    Text("Folders")
                } footer: {
                    Text("Paths as the server sees them, e.g. /data/movies")
                }
            }
            .navigationTitle(section.map { "Edit \($0.title ?? "")" } ?? "Add Library")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button(section == nil ? "Add" : "Save") { submit() }
                        .disabled(busy || paths.isEmpty || (section == nil && choices == nil))
                }
            }
        }
        .onAppear {
            scope = store.scope
            if let section {
                name = section.title ?? ""
                let l = section.location.compactMap(\.path).filter { !$0.isEmpty }
                locations = l.isEmpty ? [""] : l
            }
        }
        // Agents and scanners depend on the library type; preselect the first of each.
        .task(id: kind) {
            guard section == nil else { return }
            choices = nil
            choicesError = nil
            let k = kind
            do {
                let c = try await Commands.libraryChoices(store.app, kind: k)
                guard k == kind else { return }
                choices = c
                agent = c.agents.first?.identifier ?? ""
                scanner = c.scanners.first ?? ""
            } catch {
                if k == kind { choicesError = errorText(error) }
            }
        }
    }

    private func submit() {
        guard !busy, !paths.isEmpty else { return }
        guard scope == store.scope else { return dismiss() }
        let title = name.trimmingCharacters(in: .whitespaces).isEmpty ? label : name.trimmingCharacters(in: .whitespaces)
        busy = true
        let app = store.app!
        Task {
            let ok: Bool
            if let section {
                ok = await store.act("Library updated") {
                    try await Commands.editLibrary(app, id: section.key ?? "", name: title, locations: paths)
                }
            } else {
                let lang = language.trimmingCharacters(in: .whitespaces)
                let form = Ops.Server.LibraryForm(
                    name: title, kind: kind, agent: agent, scanner: scanner, language: lang.isEmpty ? "en-US" : lang,
                    locations: paths)
                ok = await store.act("Added “\(title)”") { try await Commands.createLibrary(app, form: form) }
            }
            busy = false
            if ok { dismiss() }
        }
    }
}

/// A library's advanced settings (ServerLibraryPrefs.svelte).
private struct LibraryPrefsSheet: View {
    let section: Directory

    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var prefs = Loader<[Ops.Server.Pref]>()

    var body: some View {
        NavigationStack {
            Form {
                if let list = prefs.value {
                    if list.isEmpty { Text("This library has no settings.").foregroundStyle(Theme.secondaryText) }
                    ForEach(list, id: \.id) { p in
                        PrefRow(pref: p, showId: false) { value in
                            await store.act("Setting saved") {
                                try await Commands.setLibraryPref(store.app, id: section.key ?? "", pref: p.id, value: value)
                            }
                        }
                    }
                } else {
                    LoaderStatus(error: prefs.error) { prefs.reload() }
                }
            }
            .navigationTitle("\(section.title ?? "") Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
        .task { await prefs.load { try await Commands.libraryPrefs(store.app, id: section.key ?? "") } }
    }
}

/// One server or library setting: a switch, a choice or a text field (ServerPrefRow.svelte).
/// `save` returns whether the server took the value.
private struct PrefRow: View {
    let pref: Ops.Server.Pref
    var showId = false
    let save: (String) async -> Bool

    @State private var value: String?
    @State private var edit: String?

    private var current: String { value ?? pref.value }
    private static func on(_ v: String) -> Bool { v == "true" || v == "1" }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            control
            if showId { Text(pref.id).font(.caption2.monospaced()).foregroundStyle(Theme.tertiaryText) }
            if let s = pref.summary, !s.isEmpty {
                Text(s).font(.caption).foregroundStyle(Theme.secondaryText)
            }
        }
    }

    @ViewBuilder
    private var control: some View {
        if pref.kind == "bool" {
            Toggle(pref.label, isOn: Binding(get: { Self.on(current) }, set: { set($0 ? "true" : "false") }))
        } else if !pref.options.isEmpty {
            Picker(pref.label, selection: Binding(get: { current }, set: { set($0) })) {
                ForEach(pref.options, id: \.value) { Text($0.label).tag($0.value) }
            }
        } else {
            LabeledContent(pref.label) {
                HStack {
                    TextField("", text: Binding(get: { edit ?? current }, set: { edit = $0 }))
                        .multilineTextAlignment(.trailing)
                        .textInputAutocapitalization(.never).autocorrectionDisabled()
                        .onSubmit { if let edit { set(edit) } }
                    if let edit, edit != current {
                        Button("Save") { set(edit) }.buttonStyle(.borderless)
                    }
                }
            }
        }
    }

    private func set(_ v: String) {
        let before = current
        value = v
        Task {
            if await save(v) {
                edit = nil
            } else {
                value = before
            }
        }
    }
}

// MARK: Maintenance

private struct ServerMaintenance: View {
    @Environment(AppStore.self) private var store
    @State private var data = Loader<Ops.Server.Maintenance>()
    @State private var saving: Commands.Diagnostics?
    @State private var saved: [Commands.Diagnostics: URL] = [:]

    var body: some View {
        Group {
            Section("Database") {
                action("Clean Bundles", "Delete metadata bundles for media that no longer exists.") {
                    store.perform("Cleaning bundles…") { try await Commands.serverAction(store.app, action: .cleanBundles) }
                }
                action("Optimize Database", "Compact and re-index the library database.") {
                    store.perform("Optimizing database…") {
                        try await Commands.serverAction(store.app, action: .optimizeDatabase)
                    }
                }
            }
            Section {
                if let m = data.value {
                    ForEach(m.schedule, id: \.id) { p in PrefRow(pref: p) { await save(p, $0) } }
                    ForEach(Array(m.tasks.enumerated()), id: \.offset) { _, t in task(t) }
                } else {
                    LoaderStatus(error: data.error) { data.reload() }
                }
            } header: {
                HStack {
                    Text("Scheduled Tasks")
                    Spacer()
                    Menu("All") {
                        Button("Start All", systemImage: "play") {
                            store.perform("Started all tasks") { try await Commands.butlerAll(store.app, start: true) }
                        }
                        Button("Stop All", systemImage: "stop") {
                            store.perform("Stopped all tasks") { try await Commands.butlerAll(store.app, start: false) }
                        }
                    }
                    .font(.footnote)
                }
            } footer: {
                Text("Plex's butler runs these during the maintenance window. Switch off any you don't want, or run one now.")
            }
            Section("Diagnostics") {
                diagnostics(.logs, "Download Logs", "Server logs as a zip, for troubleshooting or a support request.")
                diagnostics(
                    .database, "Download Database Backup", "A copy of the library database. Large libraries take a while.")
            }
        }
        .task { await data.load { try await Commands.maintenance(store.app) } }
    }

    private func action(_ title: String, _ text: String, run: @escaping () -> Void) -> some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                Text(text).font(.caption).foregroundStyle(Theme.secondaryText)
            }
            Spacer()
            Button("Run", action: run).buttonStyle(.bordered).controlSize(.small)
        }
    }

    private func task(_ t: Ops.Server.ScheduledTask) -> some View {
        let name = t.name ?? ""
        let title = t.title ?? name
        return VStack(alignment: .leading, spacing: 4) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(title)
                    if let interval = t.interval, interval > 0 {
                        Text(interval == 1 ? "Daily" : "Every \(interval) days")
                            .font(.caption).foregroundStyle(Theme.secondaryText)
                    }
                }
                Spacer()
                Button("Run Now") {
                    store.perform("Started \(title)") { try await Commands.butlerTask(store.app, name: name, start: true) }
                }
                .buttonStyle(.bordered).controlSize(.small)
                if let pref = t.pref {
                    Toggle(
                        "Run \(title) on schedule",
                        isOn: Binding(
                            get: { pref.value == "true" || pref.value == "1" },
                            set: { on in Task { await save(pref, on ? "true" : "false") } })
                    )
                    .labelsHidden()
                } else if !t.enabled {
                    Badge("Off")
                }
            }
            if let d = t.description, !d.isEmpty {
                Text(d).font(.caption).foregroundStyle(Theme.secondaryText)
            }
        }
    }

    @discardableResult
    private func save(_ p: Ops.Server.Pref, _ value: String) async -> Bool {
        let ok = await store.act("Setting saved") { try await Commands.setPref(store.app, id: p.id, value: value) }
        if ok { await data.refresh() }
        return ok
    }

    private func diagnostics(_ what: Commands.Diagnostics, _ title: String, _ text: String) -> some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                Text(text).font(.caption).foregroundStyle(Theme.secondaryText)
            }
            Spacer()
            if let url = saved[what] {
                ShareLink(item: url) { Image(systemName: "square.and.arrow.up") }
            } else if saving == what {
                ProgressView()
            } else {
                // Diagnostics can take a while on big servers.
                Button("Download") {
                    saving = what
                    Task {
                        do {
                            saved[what] = try await Commands.saveDiagnostics(store.app, what: what)
                        } catch {
                            store.error(error)
                        }
                        saving = nil
                    }
                }
                .buttonStyle(.bordered).controlSize(.small)
                .disabled(saving != nil)
            }
        }
    }
}

// MARK: Settings

private struct ServerSettings: View {
    @Environment(AppStore.self) private var store
    @State private var prefs = Loader<[Ops.Server.PrefGroup]>()
    @State private var filter = ""
    @State private var advanced = false

    /// Groups with the settings that pass the advanced switch and the filter.
    private var groups: [Ops.Server.PrefGroup] {
        let f = filter.trimmingCharacters(in: .whitespaces).lowercased()
        return (prefs.value ?? []).compactMap { g in
            var g = g
            g.prefs = g.prefs.filter { p in
                (advanced || !p.advanced)
                    && (f.isEmpty || [p.label, p.id, p.summary ?? ""].contains { $0.lowercased().contains(f) })
            }
            return g.prefs.isEmpty ? nil : g
        }
    }

    var body: some View {
        Group {
            Section {
                TextField("Search all settings", text: $filter)
                    .textInputAutocapitalization(.never).autocorrectionDisabled()
                Toggle("Show Advanced", isOn: $advanced)
            }
            if prefs.value != nil {
                if groups.isEmpty {
                    Text("No settings match “\(filter.trimmingCharacters(in: .whitespaces))”.")
                        .foregroundStyle(Theme.secondaryText)
                }
                ForEach(groups, id: \.id) { g in
                    Section(g.label) {
                        ForEach(g.prefs, id: \.id) { p in
                            PrefRow(pref: p, showId: advanced) { value in
                                let ok = await store.act("Setting saved") {
                                    try await Commands.setPref(store.app, id: p.id, value: value)
                                }
                                if ok { await prefs.refresh() }
                                return ok
                            }
                        }
                    }
                }
            } else {
                LoaderStatus(error: prefs.error) { prefs.reload() }
            }
        }
        .task { await prefs.load { try await Commands.prefs(store.app) } }
    }
}

// MARK: Users and devices

private struct ServerUsers: View {
    @Environment(AppStore.self) private var store
    @State private var data = Loader<Ops.Server.UsersDevices>()

    var body: some View {
        Group {
            if let d = data.value {
                Section("Accounts") {
                    ForEach(d.accounts.filter { ($0.id ?? 0) > 0 }, id: \.id) { a in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(a.name ?? "")
                            Text("Audio: \(a.defaultAudioLanguage ?? "auto") · Subtitles: \(a.defaultSubtitleLanguage ?? "auto")")
                                .font(.caption).foregroundStyle(Theme.secondaryText)
                        }
                    }
                }
                Section("Friends & Shared Users") {
                    if d.friends.isEmpty { Text("This server isn't shared with anyone.").foregroundStyle(Theme.secondaryText) }
                    ForEach(Array(d.friends.enumerated()), id: \.offset) { _, f in friend(f) }
                }
                Section("Players on the Network") {
                    if d.clients.isEmpty {
                        Text("No players are advertising on the local network right now.").foregroundStyle(Theme.secondaryText)
                    }
                    ForEach(Array(d.clients.enumerated()), id: \.offset) { _, c in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(c.name ?? "")
                            Text("\(c.product ?? "") · \(c.address ?? "")").font(.caption).foregroundStyle(Theme.secondaryText)
                        }
                    }
                }
                Section("Devices That Have Connected") {
                    ForEach(Array(d.devices.reversed().enumerated()), id: \.offset) { _, dev in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(dev.name ?? "")
                                Text(dev.platform ?? "").font(.caption).foregroundStyle(Theme.secondaryText)
                            }
                            Spacer()
                            if let at = dev.createdAt {
                                Text("First seen \(Format.date(Int64(at)))").font(.caption).foregroundStyle(Theme.secondaryText)
                            }
                        }
                    }
                }
            } else {
                LoaderStatus(error: data.error) { data.reload() }
            }
        }
        .task { await data.load { try await Commands.usersDevices(store.app) } }
    }

    private func friend(_ f: Friend) -> some View {
        let name = [f.title, f.username, f.email].compactMap { $0 }.first { !$0.isEmpty } ?? "User"
        let detail = [f.username != name ? f.username : nil, f.email].compactMap { $0 }.filter { !$0.isEmpty }
            .joined(separator: " · ")
        return HStack(spacing: 12) {
            Artwork(f.thumb, width: 56, height: 56, fallback: Format.initials(name))
                .frame(width: 32, height: 32)
                .clipShape(.circle)
            VStack(alignment: .leading, spacing: 2) {
                Text(name)
                if !detail.isEmpty { Text(detail).font(.caption).foregroundStyle(Theme.secondaryText) }
            }
            Spacer()
            if f.home { Badge("Home") }
            if f.restricted { Badge("Restricted") }
            if let s = f.status, s != "accepted" { Badge(s.capitalized) }
        }
    }
}
