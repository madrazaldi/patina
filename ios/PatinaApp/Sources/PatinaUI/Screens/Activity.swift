// Port of web/src/pages/Activity.svelte: what is playing on the server (owners can stop a
// stream), the watch history page by page, and the server's background tasks and transcodes.

import PatinaCore
import PlexKit
import SwiftUI

struct ActivityScreen: View {
    enum Tab: String, CaseIterable {
        case playing = "Now Playing"
        case history = "History"
        case tasks = "Server Tasks"
    }

    @Environment(AppStore.self) private var store
    @State private var tab = Tab.playing
    @State private var sessions = Loader<[Metadata]>()
    @State private var history = Loader<Ops.Lists.HistoryPage>()
    @State private var transcodes = Loader<[TranscodeSession]>()
    @State private var page = 0

    var body: some View {
        List {
            Section {
                PillTabs(selection: $tab, options: Tab.allCases.map { ($0, $0.rawValue) })
                    .listRowBackground(Color.clear)
                    .listRowInsets(EdgeInsets())
            }
            switch tab {
            case .playing: playing
            case .history: historyList
            case .tasks: tasks
            }
        }
        .navigationTitle("Activity")
        .refreshable { await reload() }
        // Sessions refetch on server events; the slow tick only keeps their progress moving.
        // Transcodes have no events, so they poll.
        .task(id: "\(tab):\(store.sessionsTick):\(store.revision):\(page)") {
            while !Task.isCancelled {
                await reload()
                let secs = tab == .playing ? 15 : tab == .tasks ? 5 : 0
                if secs == 0 { return }
                try? await Task.sleep(for: .seconds(secs))
            }
        }
    }

    private func reload() async {
        let app = store.app!
        switch tab {
        case .playing: await sessions.load { try await Commands.sessions(app) }
        case .history:
            let p = page
            await history.load { try await Commands.history(app, page: p) }
        case .tasks: await transcodes.load { try await Commands.tasks(app).transcodes }
        }
    }

    // MARK: Now playing

    @ViewBuilder
    private var playing: some View {
        if let list = sessions.value {
            if list.isEmpty {
                StatusView(
                    title: "Nothing is playing", text: "Streams from every Plex app on this server show up here live.",
                    systemImage: "play.tv")
                    .listRowBackground(Color.clear)
            } else {
                Section {
                    HStack {
                        stat("\(list.count)", "Streams")
                        stat("\(list.count { $0.transcodeSession?.videoDecision == "transcode" })", "Transcoding")
                        stat(
                            Format.bitrate(kbps: Int64(list.reduce(0) { $0 + ($1.session?.bandwidth ?? 0) })),
                            "Bandwidth")
                    }
                }
                Section {
                    ForEach(Array(list.enumerated()), id: \.offset) { _, m in SessionRow(item: m) }
                }
            }
        } else {
            LoaderStatus(error: sessions.error) { sessions.reload() }
        }
    }

    private func stat(_ value: String, _ label: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label).font(.caption).foregroundStyle(Theme.secondaryText)
            Text(value).font(.title3.weight(.semibold)).monospacedDigit().lineLimit(1).minimumScaleFactor(0.7)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    // MARK: History

    @ViewBuilder
    private var historyList: some View {
        if let h = history.value {
            if h.items.isEmpty && page == 0 {
                StatusView(title: "No history yet", systemImage: "clock.arrow.circlepath")
                    .listRowBackground(Color.clear)
            } else {
                Section {
                    ForEach(Array(h.items.enumerated()), id: \.offset) { _, m in
                        historyRow(m, who: m.accountID.flatMap { h.accounts[$0] })
                    }
                } footer: {
                    HStack {
                        Button("Newer", systemImage: "chevron.left") { page -= 1 }.disabled(page == 0)
                        Spacer()
                        Text("Page \(page + 1)").monospacedDigit()
                        Spacer()
                        Button("Older", systemImage: "chevron.right") { page += 1 }.disabled(!h.hasMore)
                            .labelStyle(TrailingIconLabelStyle())
                    }
                    .font(.subheadline)
                    .padding(.top, 8)
                }
            }
        } else {
            LoaderStatus(error: history.error) { history.reload() }
        }
    }

    private func historyRow(_ m: Metadata, who: String?) -> some View {
        Button {
            store.navigate(to: PlexUI.route(m))
        } label: {
            HStack(spacing: 12) {
                Artwork(Metadata.poster(m), width: 96, height: 144)
                    .frame(width: 36, height: 54)
                    .clipShape(.rect(cornerRadius: 6))
                VStack(alignment: .leading, spacing: 2) {
                    Text(Metadata.fullTitle(m)).lineLimit(2)
                    Text([who, m.viewedAt.map { Format.ago(Int64($0)) }].compactMap { $0 }.joined(separator: " · "))
                        .font(.caption).foregroundStyle(Theme.secondaryText)
                }
            }
        }
        .buttonStyle(.plain)
        .swipeActions {
            if store.owned, let key = m.historyKey {
                Button("Delete", systemImage: "trash", role: .destructive) {
                    store.perform(refresh: true) { try await Commands.deleteHistory(store.app, key: key) }
                }
            }
        }
    }

    // MARK: Server tasks

    @ViewBuilder
    private var tasks: some View {
        Section("Background Activity") {
            if store.activities.isEmpty {
                Text("Nothing running.").foregroundStyle(Theme.secondaryText)
            }
            ForEach(store.activities, id: \.uuid) { a in
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(a.title).font(.subheadline.weight(.semibold))
                            if let s = a.subtitle { Text(s).font(.caption).foregroundStyle(Theme.secondaryText) }
                        }
                        Spacer()
                        Text("\(a.progress)%").font(.caption).monospacedDigit().foregroundStyle(Theme.secondaryText)
                        if a.cancellable {
                            Button("Cancel") {
                                store.perform { try await Commands.cancelActivity(store.app, uuid: a.uuid) }
                            }
                            .buttonStyle(.bordered).controlSize(.small)
                        }
                    }
                    ProgressView(value: Double(a.progress), total: 100).tint(store.accent)
                }
            }
        }
        Section("Transcoder Sessions") {
            if let list = transcodes.value {
                if list.isEmpty { Text("No transcodes running.").foregroundStyle(Theme.secondaryText) }
                ForEach(Array(list.enumerated()), id: \.offset) { _, t in transcodeRow(t) }
            } else {
                LoaderStatus(error: transcodes.error) { transcodes.reload() }
            }
        }
    }

    private func transcodeRow(_ t: TranscodeSession) -> some View {
        let up = { (s: String?) in (s ?? "").uppercased() }
        return VStack(alignment: .leading, spacing: 6) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text("\(up(t.sourceVideoCodec)) → \(up(t.videoCodec)) · \(up(t.audioCodec))")
                        .font(.subheadline.weight(.semibold))
                    Text(
                        "\(t.context ?? "") · \(t.protocol ?? "") · speed \(String(format: "%.1f", t.speed ?? 0))×\(t.throttled ? " (throttled)" : "")"
                    )
                    .font(.caption).foregroundStyle(Theme.secondaryText)
                }
                Spacer()
                if let key = t.key {
                    Button("Stop", role: .destructive) {
                        store.perform { try await Commands.killTranscode(store.app, key: key) }
                    }
                    .buttonStyle(.bordered).controlSize(.small)
                }
            }
            ProgressView(value: min(100, max(0, t.progress ?? 0)), total: 100).tint(store.accent)
        }
    }
}

/// One playback session: who, on what, how it is delivered, and how far along.
private struct SessionRow: View {
    let item: Metadata
    @Environment(AppStore.self) private var store

    var body: some View {
        let m = item
        let (title, sub) = PlexUI.playingLines(m)
        let (label, transcoding) = decision(m.transcodeSession)
        HStack(alignment: .top, spacing: 12) {
            Button {
                store.navigate(to: PlexUI.route(m))
            } label: {
                Artwork(Metadata.poster(m), width: 200, height: 300)
                    .frame(width: 60, height: 90)
                    .clipShape(.rect(cornerRadius: 8))
            }
            .buttonStyle(.plain)
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 6) {
                    Text((m.player?.state ?? "").uppercased())
                        .font(.caption2.weight(.semibold)).foregroundStyle(store.accent)
                    Text(m.user?.title ?? "Unknown").font(.caption)
                    Text("on \(m.player?.title ?? "") (\(m.player?.product ?? ""))")
                        .font(.caption).foregroundStyle(Theme.secondaryText)
                }
                .lineLimit(1)
                Text(title).font(.headline).lineLimit(2)
                if !sub.isEmpty { Text(sub).font(.subheadline).foregroundStyle(Theme.secondaryText).lineLimit(1) }
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        Badge(label, accent: transcoding)
                        if let l = m.session?.location { Badge(l.uppercased()) }
                        if let b = m.session?.bandwidth, b > 0 { Badge(Format.bitrate(kbps: Int64(b))) }
                        if let a = m.player?.address { Badge(a) }
                    }
                }
                ProgressView(value: Metadata.progress(m) ?? 0).tint(store.accent)
                HStack {
                    Text(
                        "\(Format.clock(ms: Int64(m.viewOffset ?? 0))) / \(Format.clock(ms: Int64(m.duration ?? 0)))"
                    )
                    .font(.caption).monospacedDigit().foregroundStyle(Theme.secondaryText)
                    Spacer()
                    if store.owned, let sid = m.session?.id {
                        Button("Stop Stream", role: .destructive) {
                            store.perform("Stream stopped") { try await Commands.terminateSession(store.app, id: sid) }
                        }
                        .buttonStyle(.bordered).controlSize(.mini)
                    }
                }
            }
        }
        .padding(.vertical, 4)
    }

    private func decision(_ t: TranscodeSession?) -> (String, Bool) {
        guard let t else { return ("Direct play", false) }
        if t.videoDecision == "transcode" {
            let hw = t.transcodeHwRequested ? " (HW)" : ""
            return (
                "Transcode \((t.sourceVideoCodec ?? "").uppercased()) → \((t.videoCodec ?? "").uppercased())\(hw)", true
            )
        }
        if t.audioDecision == "transcode" { return ("Audio transcode", true) }
        return ("Direct stream", false)
    }
}

/// A small outlined tag ("Direct play", "LAN", "8 Mbps").
struct Badge: View {
    let text: String
    var accent = false
    @Environment(AppStore.self) private var store

    init(_ text: String, accent: Bool = false) {
        self.text = text
        self.accent = accent
    }

    var body: some View {
        Text(text)
            .font(.caption2.weight(.medium))
            .lineLimit(1)
            .padding(.horizontal, 7)
            .padding(.vertical, 3)
            .foregroundStyle(accent ? store.accent : Theme.secondaryText)
            .background(accent ? store.accent.opacity(0.12) : .clear, in: .capsule)
            .overlay(Capsule().strokeBorder(accent ? store.accent.opacity(0.4) : Theme.hairline))
    }
}

/// A list row while a `Loader` has no value: a spinner, or its error with a retry.
struct LoaderStatus: View {
    var error: String?
    var retry: () -> Void

    var body: some View {
        if let error {
            StatusView(error: error, retry: retry).listRowBackground(Color.clear)
        } else {
            ProgressView().frame(maxWidth: .infinity).padding(.vertical, 24).listRowBackground(Color.clear)
        }
    }
}

/// "Older ›": the icon after the title.
struct TrailingIconLabelStyle: LabelStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(spacing: 4) {
            configuration.title
            configuration.icon
        }
    }
}
