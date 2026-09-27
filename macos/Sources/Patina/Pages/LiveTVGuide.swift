import SwiftUI

// MARK: Guide

/// Channels down the side, programs laid out on a time axis across.
struct LiveTVGuide: View {
    let channels: [LiveChannel]
    @Environment(AppModel.self) private var app

    static let rowHeight: CGFloat = 60
    static let channelWidth: CGFloat = 220
    /// Points per minute of air time.
    static let scale: CGFloat = 5
    static let hours = 6

    var body: some View {
        TimelineView(.periodic(from: .now, by: 60)) { context in
            let now = Int64(context.date.timeIntervalSince1970)
            // The axis starts on the half hour before now.
            let start = now / 1800 * 1800
            let end = start + Int64(Self.hours) * 3600
            ScrollView(.vertical) {
                HStack(alignment: .top, spacing: 0) {
                    VStack(spacing: 0) {
                        Color.clear.frame(height: 28)
                        Hairline()
                        ForEach(channels, id: \.guideKey) { ch in
                            LiveTVChannelCell(channel: ch)
                                .frame(height: Self.rowHeight)
                            Hairline()
                        }
                    }
                    .frame(width: Self.channelWidth)
                    Rectangle().fill(Theme.hairline).frame(width: 1)
                    ScrollView(.horizontal, showsIndicators: true) {
                        VStack(alignment: .leading, spacing: 0) {
                            LiveTVRuler(start: start, end: end)
                                .frame(height: 28)
                            Hairline()
                            ForEach(channels, id: \.guideKey) { ch in
                                LiveTVProgramRow(channel: ch, start: start, end: end, now: now)
                                    .frame(height: Self.rowHeight)
                                Hairline()
                            }
                        }
                        .frame(width: width(start, end))
                        .overlay(alignment: .topLeading) {
                            // Now, as a line through every channel.
                            Rectangle()
                                .fill(app.accent)
                                .frame(width: 1)
                                .offset(x: x(now, start))
                                .allowsHitTesting(false)
                        }
                    }
                }
                .padding(.horizontal, Theme.gutter)
                Text("Live TV needs a tuner connected to a Plex Pass server.")
                    .font(Theme.caption)
                    .foregroundStyle(.tertiary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, Theme.gutter)
                    .padding(.vertical, 24)
            }
        }
    }

    private func x(_ t: Int64, _ start: Int64) -> CGFloat { CGFloat(t - start) / 60 * Self.scale }
    private func width(_ start: Int64, _ end: Int64) -> CGFloat { x(end, start) }
}

/// Half-hour marks along the top of the guide.
struct LiveTVRuler: View {
    let start: Int64
    let end: Int64

    var body: some View {
        ZStack(alignment: .topLeading) {
            ForEach(Array(stride(from: start, to: end, by: 1800)), id: \.self) { t in
                HStack(spacing: 0) {
                    Rectangle().fill(Theme.hairline).frame(width: 1, height: 28)
                    Text(LiveTVFormat.time(t))
                        .font(Theme.caption)
                        .foregroundStyle(.secondary)
                        .monospacedDigit()
                        .padding(.leading, 6)
                }
                .offset(x: CGFloat(t - start) / 60 * LiveTVGuide.scale)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

struct LiveTVChannelCell: View {
    let channel: LiveChannel
    @Environment(AppModel.self) private var app

    var body: some View {
        HStack(spacing: 10) {
            Text(channel.number ?? "")
                .font(Theme.caption)
                .foregroundStyle(.secondary)
                .monospacedDigit()
                .frame(width: 34, alignment: .trailing)
            Artwork(channel.thumb, width: 80, height: 80, contentMode: .fit)
                .frame(width: 30, height: 30)
                .clipShape(RoundedRectangle(cornerRadius: 3))
            Text(channel.title)
                .font(.system(size: 13, weight: .medium))
                .lineLimit(1)
            Spacer(minLength: 4)
            Button { LiveTVActions.watch(channel) } label: { Image(systemName: "play.fill") }
                .buttonStyle(.borderless)
                .help("Watch \(channel.title)")
                .padding(.trailing, 12)
        }
        .contentShape(Rectangle())
        .contextMenu {
            Button("Watch \(channel.title)", systemImage: "play") { LiveTVActions.watch(channel) }
        }
    }
}

@MainActor
enum LiveTVActions {
    static func watch(_ ch: LiveChannel) {
        let app = AppModel.shared
        app.toast("Tuning…")
        Task {
            do { try await API.livetvWatch(dvr: ch.dvr, channel: ch.id, title: ch.title) } catch { app.error(error) }
        }
    }

    static func record(_ p: LiveProgram) {
        guard let guid = p.guid else { return }
        AppModel.shared.perform("Recording scheduled", refresh: true) { try await API.record(guid) }
    }
}

struct LiveTVProgramRow: View {
    let channel: LiveChannel
    let start: Int64
    let end: Int64
    let now: Int64

    var body: some View {
        let shown = channel.programs.filter { $0.endsAt > start && $0.beginsAt < end && $0.endsAt > now }
        ZStack(alignment: .topLeading) {
            if shown.isEmpty {
                Text("No guide data")
                    .font(Theme.caption)
                    .foregroundStyle(.tertiary)
                    .padding(.leading, 10)
                    .frame(maxHeight: .infinity)
            }
            ForEach(shown, id: \.self) { p in
                let from = max(p.beginsAt, start), to = min(p.endsAt, end)
                LiveTVProgramCell(channel: channel, program: p, now: now)
                    .frame(width: max(2, CGFloat(to - from) / 60 * LiveTVGuide.scale - 1))
                    .offset(x: CGFloat(from - start) / 60 * LiveTVGuide.scale + 1)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}

struct LiveTVProgramCell: View {
    let channel: LiveChannel
    let program: LiveProgram
    let now: Int64

    @State private var hovering = false
    @State private var showing = false

    private var onAir: Bool { program.beginsAt <= now && now < program.endsAt }

    private var elapsed: Double {
        let span = Double(program.endsAt - program.beginsAt)
        return span > 0 ? min(1, max(0, Double(now - program.beginsAt) / span)) : 0
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(program.title)
                .font(.system(size: 12, weight: onAir ? .semibold : .regular))
                .foregroundStyle(onAir ? .primary : .secondary)
            Text(detail)
                .font(.system(size: 10.5))
                .foregroundStyle(.tertiary)
                .monospacedDigit()
            Spacer(minLength: 0)
            if onAir { ProgressLine(value: elapsed).frame(maxWidth: 200) }
        }
        .lineLimit(1)
        .padding(.horizontal, 8)
        .padding(.top, 8)
        .padding(.bottom, onAir ? 6 : 8)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(hovering || showing ? Theme.placeholder : .clear)
        .overlay(alignment: .leading) { Rectangle().fill(Theme.hairline).frame(width: 1) }
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
        .onTapGesture { showing = true }
        .help("\(program.title) · \(LiveTVFormat.range(program))")
        .contextMenu {
            if onAir { Button("Watch \(channel.title)", systemImage: "play") { LiveTVActions.watch(channel) } }
            if program.guid != nil { Button("Record", systemImage: "record.circle") { LiveTVActions.record(program) } }
        }
        .popover(isPresented: $showing, arrowEdge: .bottom) {
            LiveTVProgramDetail(channel: channel, program: program, onAir: onAir) { showing = false }
        }
    }

    private var detail: String {
        [LiveTVFormat.range(program), program.subtitle ?? ""].filter { !$0.isEmpty }.joined(separator: " · ")
    }
}

struct LiveTVProgramDetail: View {
    let channel: LiveChannel
    let program: LiveProgram
    let onAir: Bool
    let dismiss: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Eyebrow(onAir ? "On now · \(channel.title)" : channel.title)
            Text(program.title).font(Theme.display(19)).fixedSize(horizontal: false, vertical: true)
            if let s = program.subtitle { Text(s).font(.callout).foregroundStyle(.secondary) }
            Text(LiveTVFormat.range(program)).font(Theme.caption).foregroundStyle(.secondary).monospacedDigit()
            if let summary = program.summary, !summary.isEmpty {
                Text(summary).font(.callout).fixedSize(horizontal: false, vertical: true)
            }
            HStack {
                if onAir {
                    Button("Watch", systemImage: "play.fill") { dismiss(); LiveTVActions.watch(channel) }
                        .keyboardShortcut(.defaultAction)
                }
                if program.guid != nil {
                    Button("Record", systemImage: "record.circle") { dismiss(); LiveTVActions.record(program) }
                }
            }
            .padding(.top, 4)
        }
        .padding(18)
        .frame(width: 320, alignment: .leading)
    }
}

private extension LiveChannel {
    /// Channel ids repeat across tuners (DVRs).
    var guideKey: String { "\(dvr)\u{1}\(id)" }
}
