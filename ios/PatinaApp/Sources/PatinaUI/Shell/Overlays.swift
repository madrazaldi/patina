// The overlays of the shell: toasts (web/src/components/Toasts.svelte) and the mini player
// (the bar of web/src/components/NowPlaying.svelte).

import PatinaCore
import PlexKit
import SwiftUI

/// Toasts, newest at the bottom, with Undo where the action can be reversed.
struct ToastStack: View {
    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        VStack(spacing: 8) {
            ForEach(store.toasts) { t in
                ToastView(toast: t)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .frame(maxWidth: 460)
        .padding(.horizontal, 16)
        // Clear of the tab bar and the mini player.
        .padding(.bottom, (store.compact && store.stage == .ready ? 58 : 16) + (store.nowPlaying != nil || store.failure != nil ? 72 : 0))
        .frame(maxWidth: .infinity, alignment: sizeClass == .regular ? .bottomTrailing : .bottom)
    }
}

struct ToastView: View {
    let toast: ToastItem
    @Environment(AppStore.self) private var store

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: toast.error ? "exclamationmark.circle.fill" : "checkmark.circle.fill")
                .foregroundStyle(toast.error ? Theme.danger : store.accent)
            Text(toast.text)
                .font(.subheadline)
                .foregroundStyle(toast.error ? Theme.danger : .primary)
                .lineLimit(3)
                .frame(maxWidth: .infinity, alignment: .leading)
            if let entry = toast.undo {
                Button("Undo") { store.undo(entry) }
                    .font(.subheadline.weight(.semibold))
                    .buttonStyle(.bordered)
                    .buttonBorderShape(.capsule)
                    .controlSize(.small)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .background(.regularMaterial, in: .rect(cornerRadius: 18, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).strokeBorder(Theme.hairline))
        .shadow(color: .black.opacity(0.35), radius: 18, y: 8)
        .gesture(DragGesture(minimumDistance: 12).onEnded { g in
            if g.translation.height > 20 { store.dismissToast(toast.id) }
        })
        .accessibilityElement(children: .contain)
        .accessibilityAddTraits(.isStaticText)
    }
}

/// The mini player, above the tab bar (iPhone) or at the bottom of the detail column (iPad),
/// while something plays: tap or swipe it up for the full player. After a failure it says so
/// and offers to try again.
struct MiniPlayerSlot: View {
    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        Group {
            if let np = store.nowPlaying {
                bar(np)
            } else if let failure = store.failure {
                failed(failure)
            }
        }
        .padding(.horizontal, sizeClass == .regular ? 20 : 10)
        .padding(.bottom, 6)
        .transition(.move(edge: .bottom).combined(with: .opacity))
    }

    private func bar(_ np: NowPlaying) -> some View {
        let (title, subtitle) = PlexUI.playingLines(np.item)
        let app = store.app!
        return HStack(spacing: 12) {
            Group {
                if let d = store.localDownload(np) {
                    DownloadArtwork(id: d.id, source: Metadata.poster(np.item))
                } else {
                    Artwork(Metadata.poster(np.item), width: 96, height: 96, server: Metadata.serverId(np.item))
                }
            }
            .frame(width: 44, height: 44)
            .clipShape(.rect(cornerRadius: 8, style: .continuous))
            VStack(alignment: .leading, spacing: 1) {
                Text(title).font(.subheadline.weight(.semibold)).lineLimit(1)
                if !subtitle.isEmpty {
                    Text(subtitle).font(.caption).foregroundStyle(Theme.secondaryText).lineLimit(1)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            Button {
                Haptics.tap()
                Commands.control(app, command: .togglePause)
            } label: {
                Image(systemName: np.paused ? "play.fill" : "pause.fill")
                    .font(.title3)
                    .contentTransition(.symbolEffect(.replace))
                    .frame(width: 44, height: 44)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(np.paused ? "Play" : "Pause")
            if np.queuePos + 1 < np.queueLen {
                Button {
                    Haptics.tap()
                    Commands.control(app, command: .next)
                } label: {
                    Image(systemName: "forward.fill")
                        .font(.body)
                        .frame(width: 36, height: 44)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Next")
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(.ultraThinMaterial, in: .rect(cornerRadius: 16, style: .continuous))
        .overlay(alignment: .bottom) {
            if np.durationMs > 0 {
                ProgressStrip(value: Double(np.positionMs) / Double(np.durationMs), height: 2)
                    .clipShape(.capsule)
                    .padding(.horizontal, 14)
            }
        }
        .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(Theme.hairline))
        .contentShape(.rect(cornerRadius: 16))
        .onTapGesture { store.playerOpen = true }
        .gesture(DragGesture(minimumDistance: 16).onEnded { g in
            if g.translation.height < -24 { store.playerOpen = true }
        })
        .contextMenu {
            Button("Open Player", systemImage: "arrow.up.left.and.arrow.down.right") { store.playerOpen = true }
            Button("Stop Playback", systemImage: "stop.fill", role: .destructive) {
                Commands.control(app, command: .stop)
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityAction(named: "Open Player") { store.playerOpen = true }
    }

    private func failed(_ failure: PlaybackFailure) -> some View {
        HStack(spacing: 12) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(Theme.danger)
                .frame(width: 44, height: 44)
            VStack(alignment: .leading, spacing: 1) {
                Text("Couldn't play \(Metadata.title(failure.item))").font(.subheadline.weight(.semibold)).lineLimit(1)
                Text(failure.reason).font(.caption).foregroundStyle(Theme.secondaryText).lineLimit(1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if failure.canRetry {
                Button("Retry") {
                    store.perform { try Commands.recoverPlayback(store.app, action: .retry) }
                }
                .font(.subheadline.weight(.semibold))
            }
            Button {
                Commands.dismissFailure(store.app)
            } label: {
                Image(systemName: "xmark").frame(width: 32, height: 44)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Dismiss")
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(.ultraThinMaterial, in: .rect(cornerRadius: 16, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(Theme.hairline))
        .contentShape(.rect(cornerRadius: 16))
        // The full player lists the other ways to play it.
        .onTapGesture { store.playerOpen = true }
    }
}

#Preview("Toasts") {
    let store = AppStore.preview { s in
        s.toasts = [
            ToastItem(id: 1, text: "Marked 3 episodes as watched", error: false, undo: 1),
            ToastItem(id: 2, text: "The server did not answer.", error: true, undo: nil),
        ]
        s.nowPlaying = NowPlaying(
            item: Fixtures.episode(), positionMs: 600_000, durationMs: 2_580_000, paused: false, transcoding: false,
            queueLen: 1, queuePos: 0, uid: 1, ending: false, stopAfter: false, live: false, local: false)
    }
    ZStack(alignment: .bottom) {
        Theme.background.ignoresSafeArea()
        VStack {
            Spacer()
            ToastStack()
            MiniPlayerSlot()
        }
    }
    .environment(store)
    .preferredColorScheme(.dark)
}
