// The overlays of the shell: toasts (web/src/components/Toasts.svelte) and the slot the
// mini player will fill (the player screen is a later task).

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
        .padding(.bottom, (store.compact && store.stage == .ready ? 58 : 16) + (store.nowPlaying != nil ? 72 : 0))
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

/// Room for the mini player above the tab bar (iPhone) or at the bottom of the detail
/// column (iPad), shown while something plays. TODO: the player agent fills it in (the
/// controls, the full-screen player); for now it shows what plays and pauses it.
struct MiniPlayerSlot: View {
    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        if let np = store.nowPlaying {
            let (title, subtitle) = PlexUI.playingLines(np.item)
            HStack(spacing: 12) {
                Artwork(Metadata.poster(np.item), width: 96, height: 96, server: Metadata.serverId(np.item))
                    .frame(width: 44, height: 44)
                    .clipShape(.rect(cornerRadius: 8, style: .continuous))
                VStack(alignment: .leading, spacing: 1) {
                    Text(title).font(.subheadline.weight(.semibold)).lineLimit(1)
                    if !subtitle.isEmpty {
                        Text(subtitle).font(.caption).foregroundStyle(Theme.secondaryText).lineLimit(1)
                    }
                }
                Spacer()
                Button {
                    Haptics.tap()
                    Commands.control(store.app, command: .togglePause)
                } label: {
                    Image(systemName: np.paused ? "play.fill" : "pause.fill")
                        .font(.title3)
                        .frame(width: 44, height: 44)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(np.paused ? "Play" : "Pause")
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
            .padding(.horizontal, sizeClass == .regular ? 20 : 10)
            .padding(.bottom, 6)
            .transition(.move(edge: .bottom).combined(with: .opacity))
        }
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
