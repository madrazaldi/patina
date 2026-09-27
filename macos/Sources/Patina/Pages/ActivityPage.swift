import SwiftUI

/// What's playing on the server, watch history, and background work
/// (web/src/pages/Activity.svelte).
struct ActivityPage: View {
    enum Tab: String, CaseIterable, Identifiable {
        case playing = "Now Playing", history = "History", tasks = "Server Tasks"
        var id: Self { self }
    }

    @Environment(AppModel.self) private var app
    @State private var tab: Tab = .playing
    /// Ticks while a tab shows: sessions refetch on server events and the slow
    /// tick keeps their progress moving; transcodes have no events, so they poll.
    @State private var tick = 0
    @State private var page = 0

    var body: some View {
        VStack(spacing: 0) {
            PageHeader("Activity", subtitle: "What's playing on the server, watch history, and background work.")
            Group {
                switch tab {
                case .playing:
                    Loading(id: [app.sessionsTick, tick]) { try await API.sessions() } content: { list in
                        ActivitySessions(sessions: list)
                    }
                case .history:
                    Loading(id: [app.revision, page]) { try await API.history(page: page) } content: { h in
                        ActivityHistory(history: h, page: $page)
                    }
                case .tasks:
                    ActivityTasks(tick: tick)
                }
            }
            .frame(maxHeight: .infinity, alignment: .top)
        }
        .navigationTitle("Activity")
        .task(id: tab) {
            let secs: Double = tab == .playing ? 15 : tab == .tasks ? 5 : 0
            guard secs > 0 else { return }
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(secs))
                if Task.isCancelled { break }
                tick += 1
            }
        }
        .toolbar {
            ToolbarItem(placement: .principal) {
                Picker("View", selection: $tab) {
                    ForEach(Tab.allCases) { Text($0.rawValue).tag($0) }
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .fixedSize()
            }
            ToolbarItem(placement: .primaryAction) {
                Button("Reload", systemImage: "arrow.clockwise") {
                    if tab == .history { app.refresh() } else { tick += 1 }
                }
                .help("Reload")
                .keyboardShortcut("r", modifiers: .command)
            }
        }
    }
}

// MARK: Now playing

struct ActivitySessions: View {
    let sessions: [Metadata]
    @Environment(AppModel.self) private var app

    var body: some View {
        if sessions.isEmpty {
            EmptyState(title: "Nothing is playing", message: "Streams from every Plex app on this server show up here live.")
        } else {
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    HStack(spacing: 48) {
                        ServerFigure(value: "\(sessions.count)", label: "Active streams")
                        ServerFigure(
                            value: "\(sessions.filter { $0.transcodeSession?.videoDecision == "transcode" }.count)",
                            label: "Transcoding"
                        )
                        ServerFigure(
                            value: Fmt.bitrate(sessions.reduce(0) { $0 + ($1.session?.bandwidth ?? 0) }),
                            label: "Total bandwidth"
                        )
                    }
                    .padding(.bottom, 22)
                    Hairline()
                    ForEach(Array(sessions.enumerated()), id: \.offset) { _, m in
                        ActivitySessionRow(item: m)
                        Hairline()
                    }
                }
                .padding(.horizontal, Theme.gutter)
                .padding(.bottom, 32)
            }
        }
    }
}

struct ActivitySessionRow: View {
    let item: Metadata
    @Environment(AppModel.self) private var app

    /// "Direct play", "Transcode HEVC → H264 (HW)", ...; and whether it's a transcode.
    private var decision: (String, Bool) {
        guard let t = item.transcodeSession else { return ("Direct play", false) }
        if t.videoDecision == "transcode" {
            let hw = t.transcodeHwRequested ? " (HW)" : ""
            return ("Transcode \((t.sourceVideoCodec ?? "").uppercased()) → \((t.videoCodec ?? "").uppercased())\(hw)", true)
        }
        if t.audioDecision == "transcode" { return ("Audio transcode", true) }
        return ("Direct stream", false)
    }

    var body: some View {
        let (title, sub) = item.playingLines
        let user = item.user?.title ?? "Unknown"
        let (label, transcoding) = decision
        HStack(alignment: .top, spacing: 20) {
            Artwork(item.posterPath, width: 200, height: 300)
                .frame(width: 80, height: 120)
                .clipShape(RoundedRectangle(cornerRadius: Theme.radius))
                .contentShape(Rectangle())
                .onTapGesture { if item.ratingKey != nil { app.open(item) } }
                .help("Open \(item.name)")
            VStack(alignment: .leading, spacing: 0) {
                HStack(spacing: 6) {
                    Text((item.player?.state ?? "").uppercased())
                        .font(.system(size: 10, weight: .semibold))
                        .tracking(0.8)
                        .foregroundStyle(app.accent)
                    Text("·").foregroundStyle(.tertiary)
                    Avatar(path: item.user?.thumb, name: user, size: 18)
                    Text(user)
                    Text("on \(item.player?.title ?? "") (\(item.player?.product ?? ""))").foregroundStyle(.secondary)
                }
                .font(.callout)
                .lineLimit(1)
                Text(title).font(Theme.display(20)).lineLimit(1).padding(.top, 8)
                if !sub.isEmpty { Text(sub).foregroundStyle(.secondary).lineLimit(1).padding(.top, 2) }
                HStack(spacing: 6) {
                    ServerTag(label, accent: transcoding)
                    if let loc = item.session?.location { ServerTag(loc.uppercased()) }
                    if let bw = item.session?.bandwidth, bw > 0 { ServerTag(Fmt.bitrate(bw)) }
                    if let addr = item.player?.address { ServerTag(addr) }
                }
                .padding(.vertical, 12)
                ProgressView(value: item.progress ?? 0)
                    .progressViewStyle(.linear)
                    .controlSize(.small)
                    .tint(app.accent)
                HStack {
                    Text("\(Fmt.clock(item.viewOffset ?? 0)) / \(Fmt.clock(item.duration ?? 0))")
                        .font(Theme.caption).foregroundStyle(.secondary).monospacedDigit()
                    Spacer()
                    if app.owned, let sid = item.session?.id {
                        Button("Stop Stream") { stop(sid, title: title, user: user) }
                            .buttonStyle(.link)
                            .font(Theme.caption)
                    }
                }
                .padding(.top, 8)
            }
        }
        .padding(.vertical, 18)
        .contextMenu {
            if item.ratingKey != nil {
                Button("Go to \(item.name)", systemImage: "arrow.right.circle") { app.open(item) }
            }
            if app.owned, let sid = item.session?.id {
                Divider()
                Button("Stop Stream…", systemImage: "stop.circle", role: .destructive) { stop(sid, title: title, user: user) }
            }
        }
    }

    private func stop(_ id: String, title: String, user: String) {
        app.confirm("Stop this stream?", message: "\(user) will stop playing \(title).", action: "Stop Stream", destructive: true) {
            await app.act("Stream stopped") { try await API.terminateSession(id) }
        }
    }
}
