// Port of web/src/pages/LiveTV.svelte: Live TV from the server's DVR. The guide is a grid of
// channels by time (the web lists now and next; a grid suits touch better), where a channel
// tunes in and a program can be recorded; Recordings lists what is scheduled and the rules.

import PatinaCore
import PlexKit
import SwiftUI

struct LiveTvScreen: View {
    enum Tab: String, CaseIterable {
        case guide = "Guide"
        case recordings = "Recordings"
    }

    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var tab = Tab.guide
    @State private var guide = Loader<[LiveTv.Channel]>()
    @State private var recordings = Loader<LiveTv.Recordings>()
    /// Unix seconds, advanced every minute so finished programs drop off.
    @State private var now = Date().timeIntervalSince1970
    @State private var program: (LiveTv.Channel, LiveTv.Program)?

    var body: some View {
        VStack(spacing: 0) {
            PillTabs(selection: $tab, options: Tab.allCases.map { ($0, $0.rawValue) })
                .padding(.horizontal, Theme.margin(sizeClass))
                .padding(.vertical, 8)
            switch tab {
            case .guide: guideView
            case .recordings: recordingsView
            }
        }
        .navigationTitle("Live TV")
        .task(id: tab) {
            let app = store.app!
            if tab == .guide {
                // Refetch every 15 minutes; move the clock every minute.
                var ticks = 0
                while !Task.isCancelled {
                    if ticks % 15 == 0 { await guide.load { try await Commands.livetvGuide(app, hours: 6) } }
                    try? await Task.sleep(for: .seconds(60))
                    now = Date().timeIntervalSince1970
                    ticks += 1
                }
            }
        }
        .task(id: "\(tab):\(store.revision)") {
            if tab == .recordings { await recordings.load { try await Commands.recordings(store.app) } }
        }
        .sheet(isPresented: Binding(get: { program != nil }, set: { if !$0 { program = nil } })) {
            if let (ch, p) = program { ProgramSheet(channel: ch, program: p, now: now).presentationDetents([.medium]) }
        }
    }

    // MARK: Guide

    @ViewBuilder
    private var guideView: some View {
        if let channels = guide.value {
            if channels.isEmpty {
                StatusView(
                    title: "No channels", text: "Set up a tuner and scan for channels in Plex's Live TV & DVR settings.",
                    systemImage: "dot.radiowaves.left.and.right")
                Spacer()
            } else {
                if let error = guide.error {
                    StaleBanner(error: error) { guide.reload() }.padding(.horizontal, Theme.margin(sizeClass))
                }
                GuideGrid(channels: channels, now: now) { ch, p in program = (ch, p) }
                    .refreshable { await guide.refresh() }
            }
        } else if let error = guide.error {
            StatusView(error: error) { guide.reload() }
            Spacer()
        } else {
            ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    // MARK: Recordings

    @ViewBuilder
    private var recordingsView: some View {
        List {
            if let r = recordings.value {
                Section("Upcoming") {
                    if r.upcoming.isEmpty {
                        Text("Nothing scheduled. Record a program from the guide.").foregroundStyle(Theme.secondaryText)
                    }
                    ForEach(r.upcoming, id: \.id) { u in
                        HStack(spacing: 12) {
                            VStack(alignment: .leading, spacing: 2) {
                                if let at = u.beginsAt {
                                    Text(Self.when(at)).font(.caption.weight(.semibold)).foregroundStyle(store.accent)
                                }
                                Text(u.title)
                                let sub = [u.subtitle, u.channel].compactMap { $0 }.filter { !$0.isEmpty }
                                if !sub.isEmpty {
                                    Text(sub.joined(separator: " · ")).font(.caption).foregroundStyle(Theme.secondaryText)
                                }
                            }
                            Spacer()
                            if let s = u.status { Badge(s) }
                        }
                    }
                }
                Section {
                    if r.rules.isEmpty { Text("No recording rules.").foregroundStyle(Theme.secondaryText) }
                    ForEach(r.rules, id: \.id) { rule in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(rule.title)
                            if let k = rule.kind { Text(k).font(.caption).foregroundStyle(Theme.secondaryText) }
                        }
                        .swipeActions {
                            Button("Delete", systemImage: "trash", role: .destructive) { deleteRule(rule) }
                        }
                        .contextMenu {
                            Button("Delete Rule", systemImage: "trash", role: .destructive) { deleteRule(rule) }
                        }
                    }
                } header: {
                    Text("Rules")
                } footer: {
                    Text("Live TV needs a tuner connected to a Plex Pass server.")
                }
            } else {
                LoaderStatus(error: recordings.error) { recordings.reload() }
            }
        }
        .refreshable { await recordings.refresh() }
    }

    private func deleteRule(_ rule: LiveTv.Rule) {
        Task {
            let ok = await store.confirm(
                "Delete recording rule?", text: "\(rule.title) will no longer be recorded. Existing recordings are kept.",
                action: "Delete", danger: true)
            if ok, await store.act("Recording rule deleted", { try await Commands.deleteRecordingRule(store.app, id: rule.id) }) {
                recordings.reload()
            }
        }
    }

    static func time(_ s: Int) -> String {
        Date(timeIntervalSince1970: TimeInterval(s)).formatted(date: .omitted, time: .shortened)
    }

    static func when(_ s: Int) -> String {
        Date(timeIntervalSince1970: TimeInterval(s)).formatted(
            .dateTime.weekday(.abbreviated).month(.abbreviated).day().hour().minute())
    }
}

/// Channels down the side, time across: each program a block as long as it runs.
private struct GuideGrid: View {
    let channels: [LiveTv.Channel]
    let now: Double
    let select: (LiveTv.Channel, LiveTv.Program) -> Void

    @Environment(AppStore.self) private var store

    static let perMinute: CGFloat = 4
    static let rowHeight: CGFloat = 64
    static let channelWidth: CGFloat = 104
    static let headerHeight: CGFloat = 28

    /// The grid starts at the half hour before now.
    private var start: Int { Int(now) / 1800 * 1800 }
    private var end: Int {
        let last = channels.flatMap(\.programs).map(\.endsAt).max() ?? 0
        return max(start + 3 * 3600, min(last, start + 24 * 3600))
    }
    private func x(_ t: Int) -> CGFloat { CGFloat(t - start) / 60 * Self.perMinute }

    var body: some View {
        ScrollView(.vertical) {
            HStack(alignment: .top, spacing: 0) {
                VStack(spacing: 0) {
                    Color.clear.frame(height: Self.headerHeight)
                    ForEach(channels, id: \.key) { ch in channel(ch) }
                }
                .frame(width: Self.channelWidth)
                .background(Theme.background)
                .zIndex(1)
                ScrollView(.horizontal) {
                    VStack(alignment: .leading, spacing: 0) {
                        header
                        ForEach(channels, id: \.key) { ch in row(ch) }
                    }
                    .overlay(alignment: .topLeading) {
                        // Now.
                        Rectangle().fill(store.accent).frame(width: 2)
                            .offset(x: x(Int(now)))
                            .allowsHitTesting(false)
                    }
                }
                .scrollIndicators(.hidden)
            }
        }
    }

    private var header: some View {
        ZStack(alignment: .topLeading) {
            ForEach(Array(stride(from: start, to: end, by: 1800)), id: \.self) { t in
                Text(LiveTvScreen.time(t))
                    .font(.caption.weight(.medium)).foregroundStyle(Theme.secondaryText)
                    .padding(.leading, 6)
                    .frame(width: 30 * Self.perMinute, height: Self.headerHeight, alignment: .leading)
                    .overlay(alignment: .leading) { Rectangle().fill(Theme.hairline).frame(width: 1) }
                    .offset(x: x(t))
            }
        }
        .frame(width: x(end), height: Self.headerHeight, alignment: .topLeading)
    }

    private func channel(_ ch: LiveTv.Channel) -> some View {
        Button {
            watch(ch)
        } label: {
            VStack(spacing: 3) {
                Artwork(ch.thumb, width: 80, height: 80)
                    .frame(width: 30, height: 30)
                    .clipShape(.rect(cornerRadius: 6))
                Text([ch.number, ch.title].compactMap { $0 }.joined(separator: " "))
                    .font(.caption2).lineLimit(1)
            }
            .padding(.horizontal, 6)
            .frame(width: Self.channelWidth, height: Self.rowHeight)
            .overlay(alignment: .bottom) { Rectangle().fill(Theme.hairline).frame(height: 1) }
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .contextMenu {
            Button("Watch \(ch.title)", systemImage: "play.fill") { watch(ch) }
        }
        .accessibilityLabel("Watch \(ch.title)")
    }

    private func row(_ ch: LiveTv.Channel) -> some View {
        ZStack(alignment: .topLeading) {
            let shown = ch.programs.filter { $0.endsAt > start && $0.beginsAt < end }
            if shown.isEmpty {
                Text("No guide data").font(.caption).foregroundStyle(Theme.tertiaryText)
                    .padding(.leading, x(Int(now)) + 10)
                    .frame(height: Self.rowHeight)
            }
            ForEach(shown, id: \.beginsAt) { p in
                let from = max(p.beginsAt, start)
                let width = max(0, x(min(p.endsAt, end)) - x(from) - 2)
                let onAir = Double(p.beginsAt) <= now && Double(p.endsAt) > now
                Button {
                    select(ch, p)
                } label: {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(p.title).font(.subheadline.weight(.semibold)).lineLimit(1)
                        Text(p.subtitle ?? "\(LiveTvScreen.time(p.beginsAt)) – \(LiveTvScreen.time(p.endsAt))")
                            .font(.caption).foregroundStyle(Theme.secondaryText).lineLimit(1)
                    }
                    .padding(.horizontal, 8)
                    .frame(width: width, height: Self.rowHeight - 6, alignment: .leading)
                    .background(
                        onAir ? store.accent.opacity(0.16) : Theme.surface,
                        in: .rect(cornerRadius: 8, style: .continuous)
                    )
                    .overlay(
                        RoundedRectangle(cornerRadius: 8, style: .continuous)
                            .strokeBorder(onAir ? store.accent.opacity(0.4) : Theme.hairline))
                    .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .offset(x: x(from) + 1, y: 3)
                .contextMenu {
                    if onAir { Button("Watch \(ch.title)", systemImage: "play.fill") { watch(ch) } }
                    if let guid = p.guid {
                        Button("Record", systemImage: "record.circle") { record(guid) }
                    }
                }
            }
        }
        .frame(width: x(end), height: Self.rowHeight, alignment: .topLeading)
        .overlay(alignment: .bottom) { Rectangle().fill(Theme.hairline).frame(height: 1) }
    }

    private func watch(_ ch: LiveTv.Channel) {
        store.toast("Tuning…")
        store.perform { try await Commands.livetvWatch(store.app, dvr: ch.dvr, channel: ch.id, title: ch.title) }
    }

    private func record(_ guid: String) {
        store.perform("Recording scheduled") { try await Commands.record(store.app, guid: guid) }
    }
}

/// A program of the guide: when, what, and Watch or Record.
private struct ProgramSheet: View {
    let channel: LiveTv.Channel
    let program: LiveTv.Program
    let now: Double

    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        let p = program
        let onAir = Double(p.beginsAt) <= now && Double(p.endsAt) > now
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                HStack(alignment: .top, spacing: 14) {
                    if p.thumb != nil {
                        Artwork(p.thumb, width: 240, height: 360)
                            .frame(width: 80, height: 120)
                            .clipShape(.rect(cornerRadius: 8))
                    }
                    VStack(alignment: .leading, spacing: 4) {
                        Text(channel.title).font(.caption.weight(.semibold)).foregroundStyle(store.accent)
                        Text(p.title).font(.title3.weight(.semibold))
                        if let s = p.subtitle { Text(s).foregroundStyle(Theme.secondaryText) }
                        Text("\(LiveTvScreen.when(p.beginsAt)) – \(LiveTvScreen.time(p.endsAt))")
                            .font(.caption).foregroundStyle(Theme.secondaryText)
                    }
                }
                if onAir {
                    ProgressView(value: (now - Double(p.beginsAt)) / Double(max(1, p.endsAt - p.beginsAt)))
                        .tint(store.accent)
                }
                if let s = p.summary, !s.isEmpty { Text(s).font(.subheadline) }
                HStack {
                    if onAir {
                        Button("Watch", systemImage: "play.fill") {
                            dismiss()
                            store.toast("Tuning…")
                            store.perform {
                                try await Commands.livetvWatch(
                                    store.app, dvr: channel.dvr, channel: channel.id, title: channel.title)
                            }
                        }
                        .buttonStyle(.borderedProminent)
                        .foregroundStyle(.black)
                    }
                    if let guid = p.guid {
                        Button("Record", systemImage: "record.circle") {
                            dismiss()
                            store.perform("Recording scheduled") { try await Commands.record(store.app, guid: guid) }
                        }
                        .buttonStyle(.bordered)
                    }
                }
                .padding(.top, 4)
            }
            .padding(20)
        }
    }
}

extension LiveTv.Channel {
    /// Unique across DVRs.
    fileprivate var key: String { "\(dvr):\(id)" }
}
