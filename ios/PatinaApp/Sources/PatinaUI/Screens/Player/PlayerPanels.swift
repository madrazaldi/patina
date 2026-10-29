// What the player opens over itself: the queue (web/src/components/Queue.svelte), chapters
// and diagnostics (PlayerPanel.svelte), and what to do when playback fails
// (PlaybackFailure.svelte).

import PatinaCore
import PatinaPlayer
import PlexKit
import SwiftUI

enum PlayerPanel: String, Identifiable {
    case queue, chapters
    var id: String { rawValue }
}

struct PlayerPanelSheet: View {
    let panel: PlayerPanel

    var body: some View {
        NavigationStack {
            switch panel {
            case .queue: QueuePanel()
            case .chapters: ChaptersPanel()
            }
        }
        .presentationDetents([.medium, .large])
        .presentationDragIndicator(.visible)
    }
}

// MARK: Queue

/// The running queue: tap to jump, drag to reorder, swipe to remove.
struct QueuePanel: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        let app = store.app!
        let items = store.queue?.items ?? []
        let current = store.queue?.current
        List {
            ForEach(Array(items.enumerated()), id: \.element.uid) { i, entry in
                Button {
                    Haptics.tap()
                    Commands.queue(app, command: .jump(uid: entry.uid))
                } label: {
                    QueueRow(item: entry.item, playing: i == current, played: current.map { i < $0 } ?? false)
                }
                .buttonStyle(.plain)
                .listRowBackground(i == current ? Theme.raised : Color.clear)
                .deleteDisabled(i == current)
            }
            .onMove { from, to in
                guard let i = from.first else { return }
                let before = to < items.count ? items[to].uid : nil
                Commands.queue(app, command: .move(uid: items[i].uid, before: before))
            }
            .onDelete { offsets in
                for i in offsets { Commands.queue(app, command: .remove(uid: items[i].uid)) }
            }
        }
        .listStyle(.plain)
        .overlay {
            if items.isEmpty {
                StatusView(title: "Nothing queued", systemImage: "list.number")
            }
        }
        .navigationTitle("Queue")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarLeading) {
                let shuffled = store.queue?.shuffled ?? false
                Button {
                    Commands.queue(app, command: .shuffle(on: !shuffled))
                } label: {
                    Image(systemName: "shuffle")
                        .foregroundStyle(shuffled ? store.accent : Theme.secondaryText)
                }
                .accessibilityLabel(shuffled ? "Unshuffle" : "Shuffle")
            }
            ToolbarItem(placement: .topBarTrailing) {
                Button("Done") { dismiss() }
            }
        }
    }
}

struct QueueRow: View {
    let item: Metadata
    var playing = false
    var played = false
    @Environment(AppStore.self) private var store

    var body: some View {
        let (title, subtitle) = PlexUI.playingLines(item)
        let wide = Metadata.kind(item) == "episode"
        HStack(spacing: 12) {
            Artwork(wide ? item.thumb : Metadata.poster(item), width: 160, height: wide ? 90 : 160, server: Metadata.serverId(item))
                .frame(width: wide ? 72 : 44, height: wide ? 40.5 : 44)
                .clipShape(.rect(cornerRadius: 6, style: .continuous))
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.subheadline.weight(playing ? .semibold : .regular)).lineLimit(1)
                if !subtitle.isEmpty {
                    Text(subtitle).font(.caption).foregroundStyle(Theme.secondaryText).lineLimit(1)
                }
            }
            Spacer(minLength: 8)
            if playing {
                Image(systemName: "speaker.wave.2.fill")
                    .foregroundStyle(store.accent)
                    .symbolEffect(.variableColor.iterative, isActive: !(store.nowPlaying?.paused ?? true))
                    .accessibilityLabel("Playing")
            } else if let ms = item.duration, ms > 0 {
                Text(Format.clock(ms: Int64(ms))).font(.caption.monospacedDigit()).foregroundStyle(Theme.tertiaryText)
            }
        }
        .opacity(played ? 0.5 : 1)
        .contentShape(.rect)
    }
}

// MARK: Chapters

struct ChaptersPanel: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        let chapters = store.playback?.chapters ?? []
        let current = store.playback?.chapter
        List {
            ForEach(Array(chapters.enumerated()), id: \.offset) { i, c in
                Button {
                    Commands.tune(store.app, command: .chapter(i))
                    dismiss()
                } label: {
                    HStack {
                        Text(c.title.isEmpty ? "Chapter \(i + 1)" : c.title)
                            .fontWeight(i == current ? .semibold : .regular)
                        Spacer()
                        Text(Format.clock(ms: Int64(c.timeMs)))
                            .font(.subheadline.monospacedDigit())
                            .foregroundStyle(Theme.secondaryText)
                    }
                    .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .listRowBackground(i == current ? Theme.raised : Color.clear)
            }
        }
        .listStyle(.plain)
        .navigationTitle("Chapters")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) { Button("Done") { dismiss() } }
        }
    }
}

// MARK: Diagnostics

/// What the engine and the connection are doing (the desktop's diagnostics rows).
struct DiagnosticsPanel: View {
    let engine: AetherPlaybackEngine
    let np: NowPlaying
    @Environment(AppStore.self) private var store

    var body: some View {
        let quality = Config.playerQuality(store.app.settings().player)
        VStack(alignment: .leading, spacing: 3) {
            row("Stream", np.local ? "Downloaded file" : np.transcoding ? "Transcoding (\(quality.label))" : "Direct play")
            if let d = store.playback?.diagnostics {
                if let c = d.connection {
                    row("Connection", [c.kind, c.address].compactMap { $0 }.joined(separator: " · "))
                }
                row("Route", d.route ?? "–")
                row("Decoder", d.hwdec ?? "–")
                row("Video", [d.videoSize, d.videoFormat].compactMap { $0 }.joined(separator: " ").nonEmpty ?? "–")
                row("Dropped", "\(d.droppedFrames) frames")
                row("Buffer", (d.buffering ? "Buffering · " : "") + (d.bufferedSecs.map { String(format: "%.1f s ahead", $0) } ?? "–"))
                if let ms = d.recoveredFromMs {
                    row("Recovered", "from \(Format.clock(ms: Int64(ms)))")
                }
            }
            row("Speed", (store.playback?.speed ?? engine.rate).formatted(.number.precision(.fractionLength(0...2))) + "×")
        }
        .font(.caption2.monospaced())
        .foregroundStyle(.white)
        .padding(10)
        .frame(maxWidth: 360, alignment: .leading)
        .background(.black.opacity(0.6), in: .rect(cornerRadius: 10))
    }

    private func row(_ label: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(label).foregroundStyle(.white.opacity(0.6)).frame(width: 80, alignment: .leading)
            Text(value).lineLimit(2)
        }
    }
}

extension String {
    var nonEmpty: String? { isEmpty ? nil : self }
}

// MARK: Failure

/// Why playback failed and what to try: again, another version, or a transcode.
struct PlaybackFailureView: View {
    let failure: PlaybackFailure
    let close: () -> Void
    @Environment(AppStore.self) private var store

    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: failure.kind == .network ? "wifi.exclamationmark" : "exclamationmark.triangle.fill")
                .font(.system(size: 44))
                .foregroundStyle(Theme.danger)
            VStack(spacing: 6) {
                Text("Couldn't play \(Metadata.fullTitle(failure.item))")
                    .font(.title3.bold())
                    .multilineTextAlignment(.center)
                Text(failure.reason)
                    .foregroundStyle(Theme.secondaryText)
                    .multilineTextAlignment(.center)
                if let detail = failure.detail {
                    Text(detail)
                        .font(.footnote.monospaced())
                        .foregroundStyle(Theme.tertiaryText)
                        .multilineTextAlignment(.center)
                        .textSelection(.enabled)
                        .lineLimit(4)
                }
            }
            VStack(spacing: 10) {
                if failure.canRetry {
                    Button {
                        recover(.retry)
                    } label: {
                        Label("Try Again", systemImage: "arrow.clockwise").frame(maxWidth: 280)
                    }
                    .buttonStyle(.borderedProminent)
                }
                let versions = failure.item.media
                if versions.count > 1 {
                    Menu {
                        ForEach(Array(versions.enumerated()), id: \.offset) { i, m in
                            if i != failure.mediaIndex {
                                Button(PlaybackFailureView.versionLabel(m, i)) { recover(.version(i)) }
                            }
                        }
                    } label: {
                        Label("Play Another Version", systemImage: "square.stack").frame(maxWidth: 280)
                    }
                    .buttonStyle(.bordered)
                }
                if let lower = failure.lowerQuality {
                    Button {
                        recover(.lowerQuality)
                    } label: {
                        Label("Transcode at \(lower.label)", systemImage: "dial.low").frame(maxWidth: 280)
                    }
                    .buttonStyle(.bordered)
                }
                Button("Close") {
                    Commands.dismissFailure(store.app)
                    close()
                }
                .buttonStyle(.bordered)
                .tint(.secondary)
            }
            .controlSize(.large)
            if failure.remaining > 0 {
                Text(Format.plural(failure.remaining, "more item") + " in the queue")
                    .font(.footnote)
                    .foregroundStyle(Theme.tertiaryText)
            }
        }
        .padding(28)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Theme.background)
    }

    private func recover(_ action: Failure.Recover) {
        Haptics.tap()
        do {
            try Commands.recoverPlayback(store.app, action: action)
        } catch {
            store.error(error)
        }
    }

    static func versionLabel(_ m: Media, _ i: Int) -> String {
        let parts = [m.editionTitle ?? m.title, PlexUI.resolution(m.videoResolution), m.videoCodec?.uppercased()]
        let label = parts.compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " · ")
        return label.isEmpty ? "Version \(i + 1)" : label
    }
}
