import SwiftUI

// MARK: Overview

struct ServerOverviewTab: View {
    let reload: Int
    @Environment(AppModel.self) private var app
    @State private var bump = 0

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                Loading(id: [reload, bump]) { try await API.serverOverview() } content: { o in
                    overview(o)
                }
                ServerStreaming(reload: reload)
                    .padding(.top, 44)
            }
            .padding(.horizontal, Theme.gutter)
            .padding(.bottom, 32)
        }
    }

    private func overview(_ o: Overview) -> some View {
        VStack(alignment: .leading, spacing: 32) {
            HStack(spacing: 48) {
                ServerFigure(value: pct(o.hostCpu), label: "Host CPU")
                ServerFigure(value: pct(o.plexCpu), label: "Plex CPU")
                ServerFigure(value: pct(o.hostMemory), label: "Host memory")
                ServerFigure(value: pct(o.plexMemory), label: "Plex memory")
            }
            HStack(alignment: .top, spacing: 48) {
                VStack(alignment: .leading, spacing: 12) {
                    ServerSectionTitle("Server")
                    Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: 20, verticalSpacing: 7) {
                        row("Version", o.version ?? "—")
                        row("Platform", o.platform ?? "—")
                        row("Owner", o.owner ?? "—")
                        row("Plex Pass", o.plexPass ? "Active" : "No")
                        row("Connected via", o.connectedVia, selectable: true)
                        row("Machine ID", o.machineIdentifier ?? "—", selectable: true)
                        row("Transcoder", o.transcoder ? "Available" : "—")
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                VStack(alignment: .leading, spacing: 12) {
                    ServerSectionTitle("Updates")
                    if let r = o.release {
                        Text("Version \(r.version ?? "") is available.")
                        if let notes = r.added, !notes.isEmpty {
                            ScrollView {
                                Text(notes).font(Theme.caption).foregroundStyle(.secondary)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .textSelection(.enabled)
                            }
                            .frame(maxHeight: 160)
                        }
                        HStack {
                            if o.canInstall {
                                Button("Install Now") { app.perform("Installing update…") { try await API.applyUpdate(tonight: false) } }
                                    .buttonStyle(.borderedProminent)
                                Button("Install Tonight") { app.perform("Update scheduled for tonight") { try await API.applyUpdate(tonight: true) } }
                            } else if let url = r.downloadUrl {
                                Button("Download", systemImage: "arrow.up.right.square") { app.perform { try await API.openUrl(url) } }
                            }
                        }
                        .controlSize(.small)
                    } else {
                        Text("The server is up to date.").foregroundStyle(.secondary)
                    }
                    Button("Check Now") {
                        Task { if await app.act("Checked for updates", { try await API.checkForUpdates() }) { bump += 1 } }
                    }
                    .controlSize(.small)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    private func pct(_ v: Double) -> String { "\(Int(v.rounded()))%" }

    private func row(_ k: String, _ v: String, selectable: Bool = false) -> some View {
        GridRow {
            Text(k).foregroundStyle(.secondary).gridColumnAlignment(.leading)
            if selectable { Text(v).textSelection(.enabled).lineLimit(1).truncationMode(.middle) } else { Text(v) }
        }
    }
}

struct ServerStreaming: View {
    let reload: Int
    enum Period: String, CaseIterable, Identifiable {
        case day = "Last 24 Hours", month = "Last 30 Days"
        var id: Self { self }
    }

    @State private var period: Period = .day

    struct Loaded { var stats: ServerStats; var since: Int; var step: Int }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                ServerSectionTitle("Streaming")
                Spacer()
                Picker("Period", selection: $period) {
                    ForEach(Period.allCases) { Text($0.rawValue).tag($0) }
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .fixedSize()
            }
            Loading(id: [AnyHashable(period), AnyHashable(reload)]) {
                let now = Int(Date().timeIntervalSince1970)
                // Hourly buckets for a day, daily ones for a month.
                let (timespan, step, since) = period == .day ? (6, 3600, now - 86400) : (4, 86400, now - 30 * 86400)
                return Loaded(stats: try await API.serverStats(timespan: timespan, since: since), since: since, step: step)
            } content: { l in
                content(l)
            }
        }
    }

    @ViewBuilder private func content(_ l: Loaded) -> some View {
        let s = l.stats
        if s.bandwidth.isEmpty && s.plays == 0 {
            Text("Nothing was streamed from this server \(period == .day ? "in the last 24 hours" : "in the last 30 days").")
                .foregroundStyle(.secondary)
        } else {
            VStack(alignment: .leading, spacing: 28) {
                HStack(spacing: 48) {
                    ServerFigure(value: Fmt.bytes(Int64(s.bandwidth.reduce(0) { $0 + $1.lan + $1.wan })), label: "Data sent")
                    ServerFigure(value: Fmt.count(s.plays), label: "Plays")
                    ServerFigure(value: hours(s.playSeconds), label: "Hours played")
                }
                if !s.bandwidth.isEmpty {
                    ServerBandwidthChart(buckets: s.bandwidth, since: l.since, step: l.step)
                }
                HStack(alignment: .top, spacing: 48) {
                    VStack(alignment: .leading, spacing: 12) {
                        ServerSectionTitle("Top users")
                        ServerRanking(rows: s.accounts, empty: "No data sent to users.")
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    VStack(alignment: .leading, spacing: 12) {
                        ServerSectionTitle("Top devices")
                        ServerRanking(rows: s.devices, empty: "No data sent to devices.")
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
    }

    /// "12.5" hours below ten, whole hours above.
    private func hours(_ secs: Double) -> String {
        let h = secs / 3600
        return h < 10 ? String(format: "%.1f", h) : Fmt.count(Int(h.rounded()))
    }
}

struct ServerSectionTitle: View {
    let text: String
    init(_ text: String) { self.text = text }
    var body: some View { Text(text).font(Theme.display(18)) }
}
