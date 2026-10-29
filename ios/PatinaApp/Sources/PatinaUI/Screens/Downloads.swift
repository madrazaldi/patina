// Port of web/src/pages/Downloads.svelte: what is saved on this device, grouped by show,
// artist or kind, with the transfers' progress, and the smart downloads that keep the next
// unwatched items of a show, season or playlist (desktop/commands/auto.ts). It talks only to
// the download commands, and shows the artwork the downloads saved, so it works offline.

import PatinaCore
import PlexKit
import SwiftUI

struct DownloadsScreen: View {
    @Environment(AppStore.self) private var store
    @State private var smart: Smart.Overview?
    @State private var editing: SmartPolicySheet.Mode?
    @State private var running = false
    @State private var removing: Smart.Policy?

    var body: some View {
        let groups = DownloadGroup.make(store.downloads)
        List {
            summary
            if store.downloads.isEmpty {
                Section {
                    StatusView(
                        title: "No downloads",
                        text: "Download movies, episodes and music from their menu to watch them without a connection.",
                        systemImage: "arrow.down.circle")
                }
                .listRowBackground(Color.clear)
            }
            smartSection
            ForEach(groups) { g in
                Section {
                    ForEach(g.items, id: \.id) { d in
                        DownloadRow(download: d, linkable: linkable(d))
                    }
                } header: {
                    HStack(alignment: .firstTextBaseline) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(g.title).font(.headline).foregroundStyle(.primary).textCase(nil)
                            Text(g.subtitle).font(.caption).foregroundStyle(Theme.secondaryText).textCase(nil)
                        }
                        Spacer()
                        let ready = g.items.filter { $0.state == .done }.map(\.id)
                        if !ready.isEmpty {
                            Button("Play", systemImage: "play.fill") { play(ready) }
                                .font(.subheadline.weight(.semibold))
                                .textCase(nil)
                        }
                    }
                }
            }
        }
        .navigationTitle("Downloads")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button("Show in Files", systemImage: "folder") {
                    store.perform { try await Commands.openDownloadsFolder(store.app, store.host) }
                }
            }
        }
        .sheet(item: $editing) { mode in
            SmartPolicySheet(mode: mode) { reload() }
                .environment(store)
                .tint(store.accent)
                .preferredColorScheme(.dark)
        }
        .confirmationDialog(
            "Remove “\(removing?.title ?? "")”?", isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }),
            titleVisibility: .visible, presenting: removing
        ) { p in
            Button("Remove and Delete Downloads", role: .destructive) { remove(p, deleteDownloads: true) }
            Button("Remove, Keep Downloads") { remove(p, deleteDownloads: false) }
            Button("Cancel", role: .cancel) {}
        }
        .onChange(of: store.downloads, initial: true) { reload() }
        .onChange(of: store.scope) { reload() }
    }

    private func reload() {
        smart = Commands.smartDownloads(store.app)
    }

    private func linkable(_ d: Download) -> Bool {
        d.item.ratingKey != nil && store.connected && store.session?.serverId == d.serverId
    }

    private func play(_ ids: [String]) {
        Haptics.tap()
        store.perform { try Commands.playDownloads(store.app, ids: ids) }
    }

    // MARK: Summary

    @ViewBuilder
    private var summary: some View {
        let list = store.downloads
        if !list.isEmpty {
            let onDisk = list.reduce(0) { $0 + ($1.state == .done ? $1.size : $1.received) }
            let active = list.filter { $0.state == .queued || $0.state == .downloading }.count
            Section {
                HStack(spacing: 14) {
                    Image(systemName: "internaldrive").font(.title2).foregroundStyle(store.accent)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("\(Format.plural(list.count, "item")) · \(Format.bytes(Int64(onDisk)))")
                            .font(.subheadline.weight(.semibold))
                        Text(active > 0 ? "Downloading \(Format.plural(active, "item"))…" : "Saved on this device")
                            .font(.caption)
                            .foregroundStyle(Theme.secondaryText)
                    }
                }
            }
        }
    }

    // MARK: Smart downloads

    @ViewBuilder
    private var smartSection: some View {
        let app = store.app!
        Section {
            if let smart {
                ForEach(smart.policies, id: \.policy.id) { p in
                    Button {
                        editing = .edit(p.policy)
                    } label: {
                        SmartPolicyRow(policy: p)
                    }
                    .buttonStyle(.plain)
                    .swipeActions {
                        Button("Remove", systemImage: "trash", role: .destructive) { removing = p.policy }
                    }
                    .contextMenu {
                        Button("Edit…", systemImage: "slider.horizontal.3") { editing = .edit(p.policy) }
                        if p.active {
                            Button("Go to \(Smart.targetLabel(p.target).capitalized)", systemImage: "arrow.right") {
                                store.navigate(to: p.target == .playlist ? .playlist(id: p.ratingKey) : .item(ratingKey: p.ratingKey))
                            }
                        }
                        Button("Remove…", systemImage: "trash", role: .destructive) { removing = p.policy }
                    }
                }
                budget(smart)
            }
            HStack {
                Button("Add Show or Playlist", systemImage: "plus") { editing = .pick }
                Spacer()
                if running {
                    ProgressView()
                } else if !(smart?.policies.isEmpty ?? true) {
                    Button("Run Now") {
                        running = true
                        Task {
                            do {
                                let r = try await Commands.runSmartDownloads(app)
                                store.toast(Self.reportText(r))
                            } catch {
                                store.error(error)
                            }
                            running = false
                            reload()
                        }
                    }
                }
            }
            .buttonStyle(.borderless)
        } header: {
            Text("Smart Downloads")
        } footer: {
            Text("Keep the next unwatched episodes of a show, season or playlist on this device. Patina checks in the background and when you open it.")
        }
    }

    private func budget(_ smart: Smart.Overview) -> some View {
        let app = store.app!
        let choices: [Int?] = [nil] + [5, 10, 20, 50, 100, 200].map { $0 * 1_000_000_000 }
        return VStack(alignment: .leading, spacing: 8) {
            Picker(
                "Storage Limit",
                selection: Binding(
                    get: { smart.budget },
                    set: { b in
                        do {
                            try Commands.setDownloadBudget(app, bytes: b)
                        } catch {
                            store.error(error)
                        }
                        reload()
                    })
            ) {
                ForEach(choices, id: \.self) { b in
                    Text(b.map { Format.bytes(Int64($0)) } ?? "No Limit").tag(b)
                }
            }
            if let b = smart.budget, b > 0 {
                ProgressView(value: min(1, Double(smart.used) / Double(b)))
                    .tint(smart.used > b ? Theme.danger : store.accent)
                Text("\(Format.bytes(Int64(smart.used))) of \(Format.bytes(Int64(b))) used")
                    .font(.caption)
                    .foregroundStyle(Theme.secondaryText)
            }
        }
    }

    private func remove(_ p: Smart.Policy, deleteDownloads: Bool) {
        do {
            try Commands.removeSmartDownload(store.app, id: p.id, deleteDownloads: deleteDownloads)
            store.toast(deleteDownloads ? "Removed with its downloads" : "Removed; its downloads stay")
        } catch {
            store.error(error)
        }
        reload()
    }

    static func reportText(_ r: Smart.Report) -> String {
        var bits: [String] = []
        if r.added > 0 { bits.append("\(Format.plural(r.added, "download")) queued") }
        if r.removed > 0 { bits.append("\(r.removed) removed") }
        if r.overBudget > 0 { bits.append("\(r.overBudget) over the storage limit") }
        if let e = r.errors.first { bits.append(e) }
        return bits.isEmpty ? "Smart downloads are up to date" : bits.joined(separator: " · ")
    }
}

// MARK: Rows

struct DownloadRow: View {
    let download: Download
    let linkable: Bool
    @Environment(AppStore.self) private var store

    var body: some View {
        let d = download
        let app = store.app!
        let wide = Metadata.kind(d.item) == "episode"
        HStack(spacing: 12) {
            DownloadArtwork(id: d.id, source: wide ? d.item.thumb : Metadata.poster(d.item), server: d.serverId)
                .frame(width: wide ? 80 : 48, height: wide ? 45 : (Metadata.kind(d.item) == "track" ? 48 : 72))
                .clipShape(.rect(cornerRadius: 6, style: .continuous))
                .overlay(alignment: .bottom) {
                    if Metadata.isWatched(d.item) {
                        Image(systemName: "checkmark.circle.fill")
                            .font(.caption2)
                            .foregroundStyle(store.accent)
                            .padding(3)
                            .frame(maxWidth: .infinity, alignment: .trailing)
                    }
                }
            VStack(alignment: .leading, spacing: 3) {
                Text(DownloadGroup.rowTitle(d.item)).font(.subheadline.weight(.medium)).lineLimit(2)
                status
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            action
        }
        .contentShape(.rect)
        .onTapGesture {
            if d.state == .done { play() }
        }
        .swipeActions {
            Button("Remove", systemImage: "trash", role: .destructive) {
                withAnimation { Commands.removeDownload(app, id: d.id) }
            }
        }
        .contextMenu {
            if d.state == .done { Button("Play", systemImage: "play.fill") { play() } }
            switch d.state {
            case .downloading, .queued:
                Button("Pause", systemImage: "pause") { Commands.pauseDownload(app, id: d.id) }
            case .paused:
                Button("Resume", systemImage: "arrow.down") { Commands.retryDownload(app, id: d.id) }
            case .failed:
                Button("Retry", systemImage: "arrow.clockwise") { Commands.retryDownload(app, id: d.id) }
            case .done:
                EmptyView()
            }
            if linkable, let key = d.item.ratingKey {
                Button("Go to Item", systemImage: "arrow.right") { store.navigate(to: .item(ratingKey: key)) }
            }
            Button("Remove Download", systemImage: "trash", role: .destructive) { Commands.removeDownload(app, id: d.id) }
        }
    }

    @ViewBuilder
    private var status: some View {
        let d = download
        switch d.state {
        case .downloading:
            VStack(alignment: .leading, spacing: 4) {
                ProgressView(value: d.size > 0 ? min(1, Double(d.received) / Double(d.size)) : 0)
                    .tint(store.accent)
                Text(DownloadGroup.rowSubtitle(d)).font(.caption).foregroundStyle(Theme.secondaryText).monospacedDigit()
            }
        case .queued:
            Text("Waiting…").font(.caption).foregroundStyle(Theme.secondaryText)
        case .paused:
            Text("Paused · \(DownloadGroup.rowSubtitle(d))").font(.caption).foregroundStyle(Theme.secondaryText)
        case .failed:
            Text(d.error ?? "Failed").font(.caption).foregroundStyle(Theme.danger).lineLimit(2)
        case .done:
            Text(DownloadGroup.rowSubtitle(d)).font(.caption).foregroundStyle(Theme.secondaryText)
        }
    }

    @ViewBuilder
    private var action: some View {
        let d = download
        let app = store.app!
        Group {
            switch d.state {
            case .downloading, .queued:
                Button { Commands.pauseDownload(app, id: d.id) } label: { Image(systemName: "pause.circle") }
                    .accessibilityLabel("Pause")
            case .paused:
                Button { Commands.retryDownload(app, id: d.id) } label: { Image(systemName: "arrow.down.circle") }
                    .accessibilityLabel("Resume")
            case .failed:
                Button { Commands.retryDownload(app, id: d.id) } label: { Image(systemName: "arrow.clockwise.circle") }
                    .accessibilityLabel("Retry")
            case .done:
                Button { play() } label: { Image(systemName: "play.circle.fill") }
                    .accessibilityLabel("Play")
            }
        }
        .font(.title2)
        .foregroundStyle(d.state == .done ? store.accent : Theme.secondaryText)
        .buttonStyle(.borderless)
    }

    private func play() {
        Haptics.tap()
        let (app, id) = (store.app!, download.id)
        store.perform { try Commands.playDownloads(app, ids: [id]) }
    }
}

struct SmartPolicyRow: View {
    let policy: Smart.PolicyView

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: policy.target == .playlist ? "music.note.list" : "sparkles.tv")
                .font(.title3)
                .foregroundStyle(policy.enabled ? Color.accentColor : Theme.tertiaryText)
                .frame(width: 32)
            VStack(alignment: .leading, spacing: 2) {
                Text(policy.title).font(.subheadline.weight(.medium)).lineLimit(1)
                Text(detail).font(.caption).foregroundStyle(Theme.secondaryText).lineLimit(2)
            }
            Spacer()
            Image(systemName: "chevron.right").font(.caption).foregroundStyle(Theme.tertiaryText)
        }
        .contentShape(.rect)
        .opacity(policy.active ? 1 : 0.55)
    }

    private var detail: String {
        var bits = ["Next \(policy.keep) unwatched"]
        if policy.deleteWatched { bits.append("deletes watched") }
        if policy.downloads > 0 { bits.append("\(policy.downloads) on device · \(Format.bytes(Int64(policy.bytes)))") }
        if !policy.enabled { bits.append("paused") }
        if !policy.active { bits.append("on \(policy.serverName)") }
        return bits.joined(separator: " · ")
    }
}

// MARK: Grouping

/// Downloads of one show or artist, or of one kind (port of `groupOf` and friends).
struct DownloadGroup: Identifiable {
    var id: String
    var title: String
    var kind: String
    var items: [Download]

    var subtitle: String {
        let what = kind == "episode" ? "episode" : kind == "track" ? "track" : kind == "movie" ? "movie" : "item"
        return "\(Format.plural(items.count, what)) · \(Format.bytes(Int64(items.reduce(0) { $0 + $1.size })))"
    }

    static func make(_ all: [Download]) -> [DownloadGroup] {
        var groups: [DownloadGroup] = []
        var index: [String: Int] = [:]
        for d in all {
            let k = Metadata.kind(d.item)
            let scope = "\(d.account)|\(d.serverId)|\(k)"
            let (key, title): (String, String) =
                k == "episode" || k == "track"
                ? ("\(scope)|\(d.item.grandparentTitle ?? "Unknown")", d.item.grandparentTitle ?? "Unknown")
                : (scope, k == "movie" ? "Movies" : "Other")
            if let i = index[key] {
                groups[i].items.append(d)
            } else {
                index[key] = groups.count
                groups.append(DownloadGroup(id: key, title: title, kind: k, items: [d]))
            }
        }
        for i in groups.indices { groups[i].items.sort { before($0.item, $1.item) } }
        func rank(_ g: DownloadGroup) -> Int { g.kind == "movie" ? 0 : ["episode", "track"].contains(g.kind) ? 1 : 2 }
        return groups.sorted {
            rank($0) != rank($1) ? rank($0) < rank($1) : $0.title.localizedStandardCompare($1.title) == .orderedAscending
        }
    }

    /// Episodes by season and episode, tracks by album, disc and track, the rest by title.
    private static func before(_ a: Metadata, _ b: Metadata) -> Bool {
        switch Metadata.kind(a) {
        case "episode":
            return (a.parentIndex ?? 0, a.index ?? 0) < (b.parentIndex ?? 0, b.index ?? 0)
        case "track":
            let (x, y) = (a.parentTitle ?? "", b.parentTitle ?? "")
            if x != y { return x.localizedStandardCompare(y) == .orderedAscending }
            return (a.parentIndex ?? 0, a.index ?? 0) < (b.parentIndex ?? 0, b.index ?? 0)
        default:
            return (a.titleSort ?? a.title ?? "").localizedStandardCompare(b.titleSort ?? b.title ?? "") == .orderedAscending
        }
    }

    /// Inside a group the show or artist is already in the heading.
    static func rowTitle(_ m: Metadata) -> String {
        switch Metadata.kind(m) {
        case "episode": "\(Metadata.episodeCode(m)) · \(Metadata.title(m))"
        case "track": Metadata.title(m)
        default: Metadata.fullTitle(m)
        }
    }

    /// "Album · 1.2 GB of 2.9 GB · Server"
    static func rowSubtitle(_ d: Download) -> String {
        let size =
            d.state == .downloading || d.state == .paused
            ? "\(Format.bytes(Int64(d.received))) of \(Format.bytes(Int64(d.size)))"
            : d.size > 0 ? Format.bytes(Int64(d.size)) : ""
        let album = Metadata.kind(d.item) == "track" ? d.item.parentTitle ?? "" : ""
        return [album, size, d.serverName].filter { !$0.isEmpty }.joined(separator: " · ")
    }
}

// MARK: Policy sheet

/// Create a smart download (picking a show, season or playlist) or edit one: how many
/// unwatched items to keep, and whether watched ones go (KeepDialog.svelte).
struct SmartPolicySheet: View {
    enum Mode: Identifiable {
        /// Choose what to keep first.
        case pick
        case create(ratingKey: String, title: String)
        case edit(Smart.Policy)

        var id: String {
            switch self {
            case .pick: "pick"
            case .create(let key, _): "create:\(key)"
            case .edit(let p): "edit:\(p.id)"
            }
        }
    }

    @State var mode: Mode
    var saved: () -> Void = {}

    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var keep = 3
    @State private var deleteWatched = true
    @State private var enabled = true
    @State private var saving = false
    @State private var query = ""
    @State private var results: [Metadata] = []
    @State private var playlists: [Metadata] = []

    var body: some View {
        NavigationStack {
            Group {
                if case .pick = mode { picker } else { form }
            }
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                if !isPicking {
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Save", action: save).disabled(saving)
                    }
                }
            }
        }
        .onAppear {
            if case .edit(let p) = mode {
                keep = p.keep
                deleteWatched = p.deleteWatched
                enabled = p.enabled
            }
        }
    }

    private var isPicking: Bool {
        if case .pick = mode { return true }
        return false
    }

    private var title: String {
        switch mode {
        case .pick: "Keep Downloaded"
        case .create(_, let t): t
        case .edit(let p): p.title
        }
    }

    private var form: some View {
        Form {
            Section {
                Stepper(value: $keep, in: 1...Smart.MAX_KEEP) {
                    LabeledContent("Keep", value: Format.plural(keep, "unwatched item"))
                }
                Toggle("Delete Watched Items", isOn: $deleteWatched)
            } footer: {
                Text("Patina downloads the next unwatched items in order, and replaces them as you watch.")
            }
            if case .edit = mode {
                Section {
                    Toggle("Active", isOn: $enabled)
                }
            }
        }
        .navigationTitle(title)
        .navigationBarTitleDisplayMode(.inline)
    }

    /// Search shows and seasons; the playlists show while nothing is typed.
    private var picker: some View {
        let shown = query.isEmpty ? playlists : results
        return List(shown, id: \.ratingKey) { m in
            Button {
                mode = .create(ratingKey: Metadata.ratingKey(m), title: Metadata.title(m))
            } label: {
                QueueRow(item: m)
            }
            .buttonStyle(.plain)
        }
        .listStyle(.plain)
        .overlay {
            if shown.isEmpty {
                StatusView(
                    title: query.isEmpty ? "Search for a show" : "No shows found",
                    text: "Smart downloads work on shows, seasons and playlists.", systemImage: "magnifyingglass")
            }
        }
        .searchable(text: $query, placement: .navigationBarDrawer(displayMode: .always), prompt: "Shows and seasons")
        .navigationTitle(title)
        .navigationBarTitleDisplayMode(.inline)
        .task {
            playlists = ((try? await PendingCommands.playlists(store.app, kind: "video")) ?? [])
        }
        .task(id: query) {
            guard !query.isEmpty else { return }
            try? await Task.sleep(for: .milliseconds(250))
            let hubs = (try? await Commands.search(store.app, query: query)) ?? []
            guard !Task.isCancelled else { return }
            results = hubs.flatMap(\.metadata).filter { ["show", "season", "playlist"].contains(Metadata.kind($0)) }
        }
    }

    private func save() {
        saving = true
        let app = store.app!
        Task {
            do {
                switch mode {
                case .pick:
                    break
                case .create(let key, let t):
                    _ = try await Commands.setSmartDownload(app, ratingKey: key, keep: keep, deleteWatched: deleteWatched)
                    store.toast("Keeping \(t) downloaded")
                case .edit(let p):
                    try Commands.updateSmartDownload(
                        app, id: p.id, patch: Smart.Patch(keep: keep, deleteWatched: deleteWatched, enabled: enabled))
                }
                saved()
                dismiss()
            } catch {
                store.error(error)
            }
            saving = false
        }
    }
}
