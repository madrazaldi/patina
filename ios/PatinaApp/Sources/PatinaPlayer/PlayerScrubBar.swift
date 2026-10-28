// The scrub bar: played, buffered and remaining, chapter ticks, intro and credits ranges, and the
// bubble with the time and a still while scrubbing (desktop: the seek bar in NowPlaying.svelte).
//
// Dragging is relative, as on Apple's scrubbers: the knob moves with the finger rather than
// jumping under it, and moving the finger up off the bar makes the drag finer. A tap jumps.

import SwiftUI

public struct PlayerScrubBar: View {
    private let state: PlayerTransportState
    private let actions: PlayerTransportActions
    private let interaction: PlayerInteraction
    @State private var dragging = false
    @State private var moved = false
    @State private var lastX: CGFloat = 0

    public init(state: PlayerTransportState, actions: PlayerTransportActions, interaction: PlayerInteraction) {
        self.state = state
        self.actions = actions
        self.interaction = interaction
    }

    private var duration: Double { max(state.duration ?? 0, 0.001) }
    private var shown: Double { interaction.scrub?.target ?? state.position }
    private var active: Bool { interaction.scrub != nil }

    public var body: some View {
        VStack(spacing: 4) {
            if state.isLive {
                HStack {
                    LiveBadge()
                    Spacer()
                }
            } else {
                track
                    .frame(height: 30)
                    .accessibilityElement()
                    .accessibilityLabel("Playback position")
                    .accessibilityValue("\(PlayerTime.spoken(shown)) of \(PlayerTime.spoken(duration))")
                    .accessibilityAdjustableAction { direction in
                        switch direction {
                        case .increment: actions.skip(5, in: state)
                        case .decrement: actions.skip(-5, in: state)
                        @unknown default: break
                        }
                    }
                HStack {
                    Text(PlayerTime.format(shown))
                    Spacer()
                    Text("−" + PlayerTime.format(duration - shown))
                }
                .font(.caption.monospacedDigit().weight(.medium))
                .foregroundStyle(.white.opacity(0.8))
                .contentTransition(.numericText())
                .accessibilityHidden(true)
            }
        }
    }

    private var track: some View {
        GeometryReader { geo in
            let width = geo.size.width
            let height: CGFloat = active ? 9 : 5
            let x = { (seconds: Double) in CGFloat(min(max(seconds / duration, 0), 1)) * width }
            ZStack(alignment: .leading) {
                Capsule().fill(.white.opacity(0.22))
                if let buffered = state.buffered, buffered > 0 {
                    Capsule().fill(.white.opacity(0.32)).frame(width: x(buffered))
                }
                ForEach(state.markers) { marker in
                    Rectangle()
                        .fill(Color.yellow.opacity(0.7))
                        .frame(width: max(x(marker.end) - x(marker.start), 2))
                        .offset(x: x(marker.start))
                }
                Rectangle().fill(.white).frame(width: x(shown))
                ForEach(state.chapters.filter { $0 > 0 && $0 < duration }, id: \.self) { start in
                    Rectangle()
                        .fill(.black.opacity(0.55))
                        .frame(width: 2)
                        .offset(x: x(start) - 1)
                }
            }
            .frame(height: height)
            .clipShape(.capsule)
            .overlay(alignment: .leading) {
                Circle()
                    .fill(.white)
                    .frame(width: active ? 22 : 13, height: active ? 22 : 13)
                    .shadow(color: .black.opacity(0.3), radius: 4, y: 1)
                    .offset(x: x(shown) - (active ? 11 : 6.5))
            }
            .frame(maxHeight: .infinity)
            .contentShape(.rect)
            .overlay(alignment: .topLeading) {
                if let scrub = interaction.scrub {
                    let knob = x(scrub.target)
                    ScrubBubble(scrub: scrub, thumbnail: interaction.scrubThumbnail, showsThumbnail: state.supportsThumbnails)
                        .fixedSize()
                        .alignmentGuide(.top) { d in d[.bottom] + 14 }
                        .alignmentGuide(.leading) { d in -min(max(knob - d.width / 2, 0), max(width - d.width, 0)) }
                        .transition(.scale(scale: 0.85, anchor: .bottom).combined(with: .opacity))
                        .allowsHitTesting(false)
                }
            }
            .animation(.spring(response: 0.25, dampingFraction: 0.85), value: active)
            .gesture(drag(width: width))
        }
    }

    private func drag(width: CGFloat) -> some Gesture {
        DragGesture(minimumDistance: 0)
            .onChanged { value in
                if !dragging {
                    guard interaction.beginScrub(state: state) else { return }
                    dragging = true
                    moved = false
                    lastX = value.startLocation.x
                }
                if !moved, abs(value.translation.width) < 4, abs(value.translation.height) < 4 { return }
                moved = true
                interaction.moveScrub(
                    dx: value.location.x - lastX, velocity: nil, verticalOffset: value.translation.height,
                    width: width, state: state, actions: actions
                )
                lastX = value.location.x
            }
            .onEnded { value in
                guard dragging else { return }
                if !moved {
                    interaction.setScrub(to: Double(value.location.x / max(width, 1)) * duration, state: state, actions: actions)
                }
                interaction.endScrub(actions: actions)
                dragging = false
            }
    }
}

/// The time, the distance from where the drag began, and a still, over the knob.
struct ScrubBubble: View {
    let scrub: PlayerInteraction.Scrub
    let thumbnail: CGImage?
    let showsThumbnail: Bool

    var body: some View {
        VStack(spacing: 6) {
            if showsThumbnail {
                ZStack {
                    Rectangle().fill(.black.opacity(0.5))
                    if let thumbnail {
                        Image(decorative: thumbnail, scale: 1)
                            .resizable()
                            .aspectRatio(contentMode: .fill)
                            .transition(.opacity)
                    } else {
                        ProgressView().tint(.white)
                    }
                }
                .frame(width: 176, height: 99)
                .clipShape(.rect(cornerRadius: 10))
                .animation(.easeOut(duration: 0.12), value: thumbnail.map(ObjectIdentifier.init))
            }
            HStack(spacing: 8) {
                Text(PlayerTime.format(scrub.target))
                    .font(.subheadline.monospacedDigit().weight(.semibold))
                Text(PlayerTime.delta(scrub.target - scrub.origin))
                    .font(.caption.monospacedDigit().weight(.medium))
                    .foregroundStyle(.white.opacity(0.7))
                if scrub.snapped {
                    Image(systemName: "bookmark.fill")
                        .font(.caption2)
                        .foregroundStyle(.yellow)
                        .transition(.scale.combined(with: .opacity))
                }
            }
            .contentTransition(.numericText())
            if let label = scrub.precision.label {
                Text(label)
                    .font(.caption2.weight(.medium))
                    .foregroundStyle(.white.opacity(0.75))
                    .transition(.opacity)
            }
        }
        .foregroundStyle(.white)
        .padding(showsThumbnail ? 6 : 10)
        .padding(.bottom, showsThumbnail ? 2 : 0)
        .background(.ultraThinMaterial, in: .rect(cornerRadius: showsThumbnail ? 14 : 12))
        .environment(\.colorScheme, .dark)
        .shadow(color: .black.opacity(0.35), radius: 12, y: 6)
        .animation(.spring(response: 0.25, dampingFraction: 0.85), value: scrub.precision)
        .animation(.spring(response: 0.2, dampingFraction: 0.8), value: scrub.snapped)
    }
}

private struct LiveBadge: View {
    var body: some View {
        HStack(spacing: 6) {
            Circle().fill(.red).frame(width: 7, height: 7)
                .phaseAnimator([1, 0.35]) { dot, opacity in dot.opacity(opacity) } animation: { _ in .easeInOut(duration: 0.9) }
            Text("LIVE")
                .font(.caption.weight(.bold))
                .tracking(0.6)
        }
        .foregroundStyle(.white)
        .padding(.horizontal, 10)
        .padding(.vertical, 5)
        .background(.ultraThinMaterial, in: .capsule)
        .environment(\.colorScheme, .dark)
    }
}
