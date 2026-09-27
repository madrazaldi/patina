import AppKit
import SwiftUI

/// Downloads of one show or artist (or all movies) on one server and account.
struct DownloadGroup: Identifiable {
    var id: String
    var title: String
    var kind: String
    var items: [Download]

    var subtitle: String {
        let what = switch kind {
        case "episode": "episode"
        case "track": "track"
        case "movie": "movie"
        default: "item"
        }
        return "\(Fmt.plural(items.count, what)) · \(Fmt.bytes(items.reduce(Int64(0)) { $0 + $1.size }))"
    }

    /// Movies first, then shows and artists by name, anything else last.
    private var rank: Int { kind == "movie" ? 0 : ["episode", "track"].contains(kind) ? 1 : 2 }

    static func make(_ downloads: [Download]) -> [DownloadGroup] {
        var map: [String: DownloadGroup] = [:]
        var order: [String] = []
        for d in downloads {
            let k = d.item.kind
            let scope = "\(d.account)\u{1}\(d.serverId)\u{1}\(k)"
            let (key, title): (String, String)
            if k == "episode" || k == "track" {
                let t = d.item.grandparentTitle ?? "Unknown"
                (key, title) = ("\(scope)\u{1}\(t)", t)
            } else {
                (key, title) = (scope, k == "movie" ? "Movies" : "Other")
            }
            if map[key] == nil {
                map[key] = DownloadGroup(id: key, title: title, kind: k, items: [])
                order.append(key)
            }
            map[key]!.items.append(d)
        }
        return order.compactMap { map[$0] }
            .map { g in
                var g = g
                g.items.sort { less($0.item, $1.item) }
                return g
            }
            .sorted { a, b in
                a.rank != b.rank ? a.rank < b.rank : a.title.localizedStandardCompare(b.title) == .orderedAscending
            }
    }

    /// Episodes by season/episode, tracks by album/disc/track, the rest by title.
    private static func less(_ a: Metadata, _ b: Metadata) -> Bool {
        switch a.kind {
        case "episode":
            return (a.parentIndex ?? 0, a.index ?? 0) < (b.parentIndex ?? 0, b.index ?? 0)
        case "track":
            let at = a.parentTitle ?? "", bt = b.parentTitle ?? ""
            if at != bt { return at.localizedStandardCompare(bt) == .orderedAscending }
            return (a.parentIndex ?? 0, a.index ?? 0) < (b.parentIndex ?? 0, b.index ?? 0)
        default:
            let at = a.titleSort ?? a.name, bt = b.titleSort ?? b.name
            return at.localizedStandardCompare(bt) == .orderedAscending
        }
    }
}

struct DownloadGroupHeader: View {
    let group: DownloadGroup
    let play: ([String]) -> Void

    var body: some View {
        let done = group.items.filter { $0.state == .done }.map(\.id)
        HStack(alignment: .firstTextBaseline, spacing: 12) {
            Text(group.title).font(Theme.display(17)).foregroundStyle(.primary)
            Spacer()
            Text(group.subtitle).font(Theme.caption).foregroundStyle(.secondary).monospacedDigit()
            if ["episode", "track"].contains(group.kind), !done.isEmpty {
                Button("Play All") { play(done) }
                    .buttonStyle(.link)
                    .font(Theme.caption)
            }
        }
        .textCase(nil)
        .padding(.top, 14)
        .padding(.bottom, 4)
    }
}

struct DownloadRow: View {
    let download: Download
    let linkable: Bool

    @Environment(AppModel.self) private var app

    private var d: Download { download }
    private var square: Bool { d.item.kind == "track" }

    private var pct: Double {
        d.size > 0 ? min(1, Double(d.received) / Double(d.size)) : 0
    }

    private var title: String {
        switch d.item.kind {
        case "episode": "\(d.item.episodeCode) · \(d.item.name)"
        case "track": d.item.name
        default: d.item.fullTitle
        }
    }

    /// "Album · 1.2 GB of 2.9 GB · Server"
    private var detail: String {
        let size = d.state == .downloading
            ? "\(Fmt.bytes(d.received)) of \(Fmt.bytes(d.size))"
            : d.size > 0 ? Fmt.bytes(d.size) : ""
        return [d.item.kind == "track" ? (d.item.parentTitle ?? "") : "", size, d.serverName]
            .filter { !$0.isEmpty }.joined(separator: " · ")
    }

    var body: some View {
        HStack(spacing: 12) {
            Artwork(d.poster != nil ? .download(d.id) : nil)
                .frame(width: square ? 44 : 36, height: square ? 44 : 54)
                .clipShape(RoundedRectangle(cornerRadius: 3))
            VStack(alignment: .leading, spacing: 3) {
                if linkable, let rk = d.item.ratingKey {
                    Button(title) { app.open(.item(rk)) }
                        .buttonStyle(.plain)
                        .foregroundStyle(dim ? .secondary : .primary)
                } else {
                    Text(title).foregroundStyle(dim ? .secondary : .primary)
                }
                if d.state == .failed {
                    Text(d.error ?? "Download failed")
                        .font(Theme.caption)
                        .foregroundStyle(.red.opacity(0.8))
                        .help(d.error ?? "")
                } else if !detail.isEmpty {
                    Text(detail).font(Theme.caption).foregroundStyle(.secondary).monospacedDigit()
                }
                if d.state == .downloading {
                    ProgressView(value: pct)
                        .progressViewStyle(.linear)
                        .controlSize(.small)
                        .tint(app.accent)
                        .frame(maxWidth: 360)
                        .padding(.top, 2)
                }
            }
            .lineLimit(1)
            Spacer(minLength: 12)
            controls
        }
        .padding(.vertical, 4)
    }

    private var dim: Bool { d.state == .queued || d.state == .failed }

    @ViewBuilder private var controls: some View {
        HStack(spacing: 10) {
            switch d.state {
            case .queued:
                Text("Waiting").font(Theme.caption).foregroundStyle(.secondary)
                icon("pause.fill", "Pause download") { try await API.pauseDownload(d.id) }
            case .downloading:
                Text("\(Int(pct * 100))%").font(Theme.caption).foregroundStyle(.secondary).monospacedDigit()
                    .frame(minWidth: 34, alignment: .trailing)
                icon("pause.fill", "Pause download") { try await API.pauseDownload(d.id) }
            case .paused:
                Text("Paused").font(Theme.caption).foregroundStyle(.secondary)
                icon("arrow.down.circle", "Resume download") { try await API.retryDownload(d.id) }
            case .failed:
                Button("Retry", systemImage: "arrow.clockwise") { app.perform { try await API.retryDownload(d.id) } }
                    .controlSize(.small)
            case .done:
                icon("play.fill", "Play") { try await API.playDownloads([d.id]) }
            }
        }
    }

    private func icon(_ symbol: String, _ help: String, _ work: @escaping () async throws -> Void) -> some View {
        Button { app.perform(nil, refresh: false, work) } label: { Image(systemName: symbol) }
            .buttonStyle(.borderless)
            .help(help)
            .accessibilityLabel(help)
    }
}
