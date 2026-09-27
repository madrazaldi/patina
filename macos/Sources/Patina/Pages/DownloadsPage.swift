import AppKit
import SwiftUI

/// Downloaded items for offline playback, grouped by show / artist
/// (web/src/pages/Downloads.svelte). Live from `app.downloads`; only ever
/// talks to the download commands, so it works without a server.
struct DownloadsPage: View {
    @Environment(AppModel.self) private var app
    @State private var selection = Set<String>()

    var body: some View {
        let groups = DownloadGroup.make(app.downloads)
        VStack(spacing: 0) {
            PageHeader(title: "Downloads", subtitle: summary) {
                Button("Open Folder", systemImage: "folder") { app.perform { try await API.openDownloadsFolder() } }
            }
            if app.downloads.isEmpty {
                EmptyState(
                    title: "Nothing downloaded",
                    message: "Choose “Download” from any movie, episode, season or album menu to keep a copy on this computer and play it without a connection."
                )
                .frame(maxHeight: .infinity, alignment: .top)
            } else {
                list(groups)
            }
        }
        .navigationTitle("Downloads")
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button("Play Selected", systemImage: "play") { play(Array(playable(selection))) }
                    .disabled(playable(selection).isEmpty)
                    .help("Play the selected downloads")
            }
        }
    }

    private var summary: String {
        let all = app.downloads
        guard !all.isEmpty else { return "Saved on this computer for offline playback." }
        let onDisk = all.reduce(Int64(0)) { $0 + ($1.state == .done ? $1.size : $1.received) }
        let active = all.filter { $0.state == .queued || $0.state == .downloading }.count
        var s = "\(Fmt.plural(all.count, "item")) · \(Fmt.bytes(onDisk)) on disk"
        if active > 0 { s += " · \(active) in progress" }
        return s
    }

    private func list(_ groups: [DownloadGroup]) -> some View {
        List(selection: $selection) {
            ForEach(groups) { g in
                Section {
                    ForEach(g.items) { d in
                        DownloadRow(download: d, linkable: linkable(d))
                            .tag(d.id)
                    }
                } header: {
                    DownloadGroupHeader(group: g) { play($0) }
                }
            }
        }
        .listStyle(.inset)
        .scrollContentBackground(.hidden)
        .contextMenu(forSelectionType: String.self) { ids in
            menu(for: ids)
        } primaryAction: { ids in
            play(Array(playable(ids)))
        }
        .onDeleteCommand { remove(ids: selection) }
    }

    @ViewBuilder private func menu(for ids: Set<String>) -> some View {
        let items = app.downloads.filter { ids.contains($0.id) }
        if items.count == 1, let d = items.first {
            if d.state == .done {
                Button("Play", systemImage: "play") { play([d.id]) }
            }
            switch d.state {
            case .queued, .downloading:
                Button("Pause", systemImage: "pause") { app.perform { try await API.pauseDownload(d.id) } }
            case .paused:
                Button("Resume", systemImage: "arrow.down.circle") { app.perform { try await API.retryDownload(d.id) } }
            case .failed:
                Button("Retry", systemImage: "arrow.clockwise") { app.perform { try await API.retryDownload(d.id) } }
            case .done: EmptyView()
            }
            if linkable(d), let rk = d.item.ratingKey {
                Button("Go to \(Self.kindName(d.item))", systemImage: "arrow.right.circle") { app.open(.item(rk)) }
            }
            if let file = d.file, d.state == .done {
                Button("Show in Finder", systemImage: "folder") {
                    NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: file)])
                }
            }
            Divider()
            Button(d.state == .done ? "Delete Download…" : "Remove from Downloads…", systemImage: "trash", role: .destructive) {
                remove(ids: [d.id])
            }
        } else if !items.isEmpty {
            let ready = playable(ids)
            if !ready.isEmpty {
                Button("Play \(Fmt.plural(ready.count, "Item"))", systemImage: "play") { play(Array(ready)) }
            }
            let running = items.filter { $0.state == .queued || $0.state == .downloading }
            if !running.isEmpty {
                Button("Pause", systemImage: "pause") {
                    app.perform { for d in running { try await API.pauseDownload(d.id) } }
                }
            }
            let stopped = items.filter { $0.state == .paused || $0.state == .failed }
            if !stopped.isEmpty {
                Button("Resume", systemImage: "arrow.down.circle") {
                    app.perform { for d in stopped { try await API.retryDownload(d.id) } }
                }
            }
            Divider()
            Button("Remove \(Fmt.plural(items.count, "Download"))…", systemImage: "trash", role: .destructive) { remove(ids: ids) }
        }
    }

    // MARK: Actions

    private func linkable(_ d: Download) -> Bool {
        d.item.ratingKey != nil && app.session?.serverId == d.serverId
    }

    /// Finished downloads among `ids`, in list order.
    private func playable(_ ids: Set<String>) -> [String] {
        DownloadGroup.make(app.downloads).flatMap(\.items).filter { ids.contains($0.id) && $0.state == .done }.map(\.id)
    }

    private func play(_ ids: [String]) {
        guard !ids.isEmpty else { return }
        app.perform { try await API.playDownloads(ids) }
    }

    private func remove(ids: Set<String>) {
        let items = app.downloads.filter { ids.contains($0.id) }
        guard let first = items.first else { return }
        let done = items.allSatisfy { $0.state == .done }
        let title: String, message: String
        if items.count == 1 {
            title = done ? "Delete download?" : "Remove from downloads?"
            message = done
                ? "\(first.item.fullTitle) will be removed from this computer."
                : "\(first.item.fullTitle) will stop downloading."
        } else {
            title = "Remove \(items.count) downloads?"
            message = "The downloaded files are removed from this computer."
        }
        app.confirm(title, message: message, action: done ? "Delete" : "Remove", destructive: true) {
            await app.act {
                for d in items { try await API.removeDownload(d.id) }
            }
            selection.subtract(ids)
        }
    }

    static func kindName(_ m: Metadata) -> String {
        switch m.kind {
        case "episode": "Episode"
        case "track": "Track"
        case "movie": "Movie"
        default: "Item"
        }
    }
}
