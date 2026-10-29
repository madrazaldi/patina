// The player's chrome: close, title and AirPlay along the top, the transport in the middle, and
// the scrub bar along the bottom (desktop: NowPlaying.svelte and PlayerPanel.svelte). The track,
// speed and "more" menus go in the `menus` slot; the skip pill and the up-next card in `accessory`,
// which stays up when the rest of the chrome hides.

import AVKit
import SwiftUI

public struct PlayerControlsOverlay<Menus: View, Accessory: View>: View {
    private let title: String
    private let subtitle: String?
    private let state: PlayerTransportState
    private let actions: PlayerTransportActions
    private let interaction: PlayerInteraction
    private let menus: Menus
    private let accessory: Accessory

    public init(
        title: String, subtitle: String? = nil, state: PlayerTransportState, actions: PlayerTransportActions,
        interaction: PlayerInteraction, @ViewBuilder menus: () -> Menus, @ViewBuilder accessory: () -> Accessory
    ) {
        self.title = title
        self.subtitle = subtitle
        self.state = state
        self.actions = actions
        self.interaction = interaction
        self.menus = menus()
        self.accessory = accessory()
    }

    public var body: some View {
        let visible = interaction.chromeVisible
        let scrubbing = interaction.scrub != nil
        ZStack {
            if visible {
                Scrims()
                    .transition(.opacity)
            }
            if visible, !scrubbing {
                CentreControls(state: state, actions: actions, interaction: interaction)
                    .transition(.scale(scale: 0.92).combined(with: .opacity))
            }
            VStack(spacing: 0) {
                if visible {
                    topBar
                        .opacity(scrubbing ? 0 : 1)
                        .transition(.move(edge: .top).combined(with: .opacity))
                }
                Spacer(minLength: 0)
                HStack {
                    Spacer(minLength: 0)
                    accessory
                }
                .padding(.bottom, 14)
                if visible {
                    PlayerScrubBar(state: state, actions: actions, interaction: interaction)
                        .transition(.move(edge: .bottom).combined(with: .opacity))
                }
            }
            .padding(.horizontal, 20)
            .padding(.top, 8)
            .padding(.bottom, 10)
        }
        .animation(.spring(response: 0.35, dampingFraction: 0.86), value: visible)
        .animation(.spring(response: 0.3, dampingFraction: 0.9), value: scrubbing)
        .onChange(of: state.isPaused, initial: true) { interaction.setPlaying(!state.isPaused) }
    }

    private var topBar: some View {
        HStack(spacing: 12) {
            Button {
                actions.close()
            } label: {
                PlayerIcon(systemName: "xmark")
            }
            .buttonStyle(PlayerPressStyle())
            .keyboardShortcut(.cancelAction)
            .accessibilityLabel("Close")

            VStack(alignment: .leading, spacing: 1) {
                Text(title)
                    .font(.headline)
                    .lineLimit(1)
                if let subtitle {
                    Text(subtitle)
                        .font(.subheadline)
                        .foregroundStyle(.white.opacity(0.7))
                        .lineLimit(1)
                }
            }
            .foregroundStyle(.white)
            .shadow(color: .black.opacity(0.3), radius: 6)
            .frame(maxWidth: .infinity, alignment: .leading)
            .accessibilityElement(children: .combine)
            .accessibilityAddTraits(.isHeader)

            HStack(spacing: 10) {
                menus
                AirPlayButton()
                    .frame(width: 44, height: 44)
                    .background(.ultraThinMaterial, in: .circle)
                    .environment(\.colorScheme, .dark)
            }
            .buttonStyle(PlayerPressStyle())
        }
    }
}

extension PlayerControlsOverlay where Menus == EmptyView {
    public init(
        title: String, subtitle: String? = nil, state: PlayerTransportState, actions: PlayerTransportActions,
        interaction: PlayerInteraction, @ViewBuilder accessory: () -> Accessory
    ) {
        self.init(title: title, subtitle: subtitle, state: state, actions: actions, interaction: interaction, menus: { EmptyView() }, accessory: accessory)
    }
}

extension PlayerControlsOverlay where Accessory == EmptyView {
    public init(
        title: String, subtitle: String? = nil, state: PlayerTransportState, actions: PlayerTransportActions,
        interaction: PlayerInteraction, @ViewBuilder menus: () -> Menus
    ) {
        self.init(title: title, subtitle: subtitle, state: state, actions: actions, interaction: interaction, menus: menus, accessory: { EmptyView() })
    }
}

extension PlayerControlsOverlay where Menus == EmptyView, Accessory == EmptyView {
    public init(title: String, subtitle: String? = nil, state: PlayerTransportState, actions: PlayerTransportActions, interaction: PlayerInteraction) {
        self.init(title: title, subtitle: subtitle, state: state, actions: actions, interaction: interaction, menus: { EmptyView() }, accessory: { EmptyView() })
    }
}

/// Previous, back 10, play/pause, forward 10, next.
struct CentreControls: View {
    let state: PlayerTransportState
    let actions: PlayerTransportActions
    let interaction: PlayerInteraction
    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        let compact = sizeClass == .compact
        HStack(spacing: compact ? 26 : 44) {
            if let previous = actions.previous {
                transportButton("backward.end.fill", label: "Previous", size: 22, enabled: state.hasPrevious, action: previous)
            }
            transportButton("gobackward.10", label: "Back 10 seconds", size: 28, enabled: state.canSeek) {
                actions.skip(-10, in: state)
            }
            Button {
                interaction.poke()
                actions.togglePause()
            } label: {
                ZStack {
                    if state.isWaiting {
                        ProgressView()
                            .controlSize(.large)
                            .tint(.white)
                            .transition(.opacity)
                    } else {
                        Image(systemName: state.isPaused ? "play.fill" : "pause.fill")
                            .font(.system(size: 34, weight: .semibold))
                            .contentTransition(.symbolEffect(.replace.downUp.byLayer))
                            .transition(.opacity)
                    }
                }
                .frame(width: 80, height: 80)
                .background(.ultraThinMaterial, in: .circle)
                .environment(\.colorScheme, .dark)
                .animation(.spring(response: 0.3, dampingFraction: 0.8), value: state.isWaiting)
            }
            .buttonStyle(PlayerPressStyle())
            .accessibilityLabel(state.isPaused ? "Play" : "Pause")
            transportButton("goforward.10", label: "Forward 10 seconds", size: 28, enabled: state.canSeek) {
                actions.skip(10, in: state)
            }
            if let next = actions.next {
                transportButton("forward.end.fill", label: "Next", size: 22, enabled: state.hasNext, action: next)
            }
        }
        .foregroundStyle(.white)
        .shadow(color: .black.opacity(0.35), radius: 10)
    }

    private func transportButton(_ symbol: String, label: String, size: CGFloat, enabled: Bool, action: @escaping () -> Void) -> some View {
        Button {
            interaction.poke()
            action()
        } label: {
            Image(systemName: symbol)
                .font(.system(size: size, weight: .semibold))
                .frame(width: 56, height: 56)
                .contentShape(.circle)
        }
        .buttonStyle(PlayerPressStyle())
        .disabled(!enabled)
        .opacity(enabled ? 1 : 0.35)
        .accessibilityLabel(label)
    }
}

/// The gradients that keep the chrome legible on a bright picture.
private struct Scrims: View {
    var body: some View {
        VStack(spacing: 0) {
            LinearGradient(colors: [.black.opacity(0.6), .clear], startPoint: .top, endPoint: .bottom)
                .frame(height: 160)
            Spacer(minLength: 0)
            LinearGradient(colors: [.clear, .black.opacity(0.7)], startPoint: .top, endPoint: .bottom)
                .frame(height: 200)
        }
        .background(.black.opacity(0.18))
        .ignoresSafeArea()
        .allowsHitTesting(false)
    }
}

/// A symbol in a frosted circle, for the top bar and the menus slot.
public struct PlayerIcon: View {
    private let systemName: String

    public init(systemName: String) {
        self.systemName = systemName
    }

    public var body: some View {
        Image(systemName: systemName)
            .font(.system(size: 17, weight: .semibold))
            .foregroundStyle(.white)
            .frame(width: 44, height: 44)
            .background(.ultraThinMaterial, in: .circle)
            .environment(\.colorScheme, .dark)
            .contentShape(.circle)
    }
}

/// Springs down a little while pressed.
public struct PlayerPressStyle: ButtonStyle {
    public init() {}

    public func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(configuration.isPressed ? 0.86 : 1)
            .opacity(configuration.isPressed ? 0.8 : 1)
            .animation(.spring(response: 0.25, dampingFraction: 0.6), value: configuration.isPressed)
    }
}

/// The system's AirPlay picker. `video` false lists the audio routes first (music).
public struct AirPlayButton: UIViewRepresentable {
    private let video: Bool

    public init(video: Bool = true) {
        self.video = video
    }

    public func makeUIView(context: Context) -> AVRoutePickerView {
        let view = AVRoutePickerView()
        view.tintColor = .white
        view.prioritizesVideoDevices = video
        return view
    }

    public func updateUIView(_ view: AVRoutePickerView, context: Context) {}
}

// MARK: - Prompts

/// "Skip intro", "Skip credits": a pill that stays up while the marker plays. With a `progress`
/// (0 to 1) it fills, for a skip that happens by itself.
public struct PlayerSkipPill: View {
    private let title: String
    private let progress: Double?
    private let action: () -> Void

    public init(_ title: String, progress: Double? = nil, action: @escaping () -> Void) {
        self.title = title
        self.progress = progress
        self.action = action
    }

    public var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Text(title)
                Image(systemName: "forward.end.fill")
                    .font(.footnote)
            }
            .font(.callout.weight(.semibold))
            .foregroundStyle(.white)
            .padding(.horizontal, 20)
            .padding(.vertical, 13)
            .background {
                ZStack(alignment: .leading) {
                    Capsule().fill(.ultraThinMaterial)
                    if let progress {
                        GeometryReader { geo in
                            Rectangle()
                                .fill(.white.opacity(0.22))
                                .frame(width: geo.size.width * min(max(progress, 0), 1))
                                .animation(.linear(duration: 0.25), value: progress)
                        }
                    }
                }
                .clipShape(.capsule)
            }
            .overlay(Capsule().strokeBorder(.white.opacity(0.25), lineWidth: 1))
            .environment(\.colorScheme, .dark)
            .shadow(color: .black.opacity(0.3), radius: 12, y: 4)
        }
        .buttonStyle(PlayerPressStyle())
        .transition(.move(edge: .trailing).combined(with: .opacity))
    }
}

/// What plays next, with a countdown, near the end of an item (desktop: the "Up next" row in NowPlaying.svelte).
public struct UpNextCard<Thumbnail: View>: View {
    private let title: String
    private let subtitle: String?
    private let countdown: Double?
    private let secondsLeft: Int?
    private let onPlayNow: () -> Void
    private let onStopAfter: (() -> Void)?
    private let thumbnail: Thumbnail

    /// - Parameters:
    ///   - countdown: How much of the countdown has run, 0 to 1; nil for no countdown.
    ///   - secondsLeft: Shown on the play button while counting down.
    public init(
        title: String, subtitle: String? = nil, countdown: Double? = nil, secondsLeft: Int? = nil,
        onPlayNow: @escaping () -> Void, onStopAfter: (() -> Void)? = nil, @ViewBuilder thumbnail: () -> Thumbnail
    ) {
        self.title = title
        self.subtitle = subtitle
        self.countdown = countdown
        self.secondsLeft = secondsLeft
        self.onPlayNow = onPlayNow
        self.onStopAfter = onStopAfter
        self.thumbnail = thumbnail()
    }

    public var body: some View {
        HStack(alignment: .top, spacing: 14) {
            thumbnail
                .frame(width: 128, height: 72)
                .clipShape(.rect(cornerRadius: 10))
                .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(.white.opacity(0.12)))
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 8) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Up Next")
                        .font(.caption2.weight(.semibold))
                        .textCase(.uppercase)
                        .tracking(0.8)
                        .foregroundStyle(.white.opacity(0.6))
                    Text(title)
                        .font(.subheadline.weight(.semibold))
                        .lineLimit(2)
                    if let subtitle {
                        Text(subtitle)
                            .font(.caption)
                            .foregroundStyle(.white.opacity(0.7))
                            .lineLimit(1)
                    }
                }
                HStack(spacing: 8) {
                    Button(action: onPlayNow) {
                        HStack(spacing: 6) {
                            Image(systemName: "play.fill")
                            Text(secondsLeft.map { "Play in \($0)" } ?? "Play Now")
                                .monospacedDigit()
                                .contentTransition(.numericText(countsDown: true))
                        }
                        .font(.footnote.weight(.semibold))
                        .foregroundStyle(.black)
                        .padding(.horizontal, 14)
                        .padding(.vertical, 8)
                        .background {
                            GeometryReader { geo in
                                ZStack(alignment: .leading) {
                                    Capsule().fill(.white.opacity(countdown == nil ? 1 : 0.7))
                                    if let countdown {
                                        Rectangle().fill(.white)
                                            .frame(width: geo.size.width * min(max(countdown, 0), 1))
                                            .animation(.linear(duration: 0.25), value: countdown)
                                    }
                                }
                                .clipShape(.capsule)
                            }
                        }
                    }
                    .buttonStyle(PlayerPressStyle())
                    if let onStopAfter {
                        Button("Stop After This", action: onStopAfter)
                            .font(.footnote.weight(.semibold))
                            .foregroundStyle(.white)
                            .padding(.horizontal, 12)
                            .padding(.vertical, 8)
                            .background(.white.opacity(0.14), in: .capsule)
                            .buttonStyle(PlayerPressStyle())
                    }
                }
            }
        }
        .foregroundStyle(.white)
        .padding(12)
        .frame(maxWidth: 400, alignment: .leading)
        .background(.ultraThinMaterial, in: .rect(cornerRadius: 18))
        .overlay(RoundedRectangle(cornerRadius: 18).strokeBorder(.white.opacity(0.12)))
        .environment(\.colorScheme, .dark)
        .shadow(color: .black.opacity(0.35), radius: 18, y: 8)
        .transition(.move(edge: .trailing).combined(with: .opacity))
    }
}

// MARK: - Keyboard

extension View {
    /// The hardware keyboard on iPad: Space plays or pauses, ← and → skip 5 seconds, ↑ and ↓ change
    /// the volume, Esc closes (the close button carries Esc itself while the chrome shows).
    public func playerKeyboardShortcuts(state: PlayerTransportState, actions: PlayerTransportActions, interaction: PlayerInteraction) -> some View {
        background {
            PlayerKeyCommands(state: state, actions: actions, interaction: interaction)
        }
    }
}

private struct PlayerKeyCommands: View {
    let state: PlayerTransportState
    let actions: PlayerTransportActions
    let interaction: PlayerInteraction

    var body: some View {
        Group {
            Button(state.isPaused ? "Play" : "Pause") {
                actions.togglePause()
                interaction.showChrome()
            }
            .keyboardShortcut(.space, modifiers: [])
            Button("Back 5 Seconds") {
                actions.skip(-5, in: state)
                interaction.showChrome()
            }
            .keyboardShortcut(.leftArrow, modifiers: [])
            Button("Forward 5 Seconds") {
                actions.skip(5, in: state)
                interaction.showChrome()
            }
            .keyboardShortcut(.rightArrow, modifiers: [])
            Button("Volume Up") { interaction.adjustVolume(by: 0.0625) }
                .keyboardShortcut(.upArrow, modifiers: [])
            Button("Volume Down") { interaction.adjustVolume(by: -0.0625) }
                .keyboardShortcut(.downArrow, modifiers: [])
            if !interaction.chromeVisible {
                Button("Close", action: actions.close)
                    .keyboardShortcut(.cancelAction)
            }
        }
        .frame(width: 0, height: 0)
        .opacity(0)
        .accessibilityHidden(true)
    }
}
