// The touch layer over the picture (no desktop twin: mpv took mouse and keys itself).
//
// Tap shows or hides the controls; double-tap on the left or right third skips ten seconds, and
// each further tap there adds ten more; a sideways drag scrubs; an up-or-down drag changes the
// volume (right half) or the brightness (left half); pinch switches fit and fill; and a long
// press plays at 2x until the finger lifts. UIKit recognizers, because they report velocity and
// arbitrate between one another far better than SwiftUI's gestures.

import MediaPlayer
import SwiftUI
import UIKit

public struct PlayerGestureLayer: View {
    private let state: PlayerTransportState
    private let actions: PlayerTransportActions
    private let interaction: PlayerInteraction
    @Binding private var fill: PlayerVideoFill

    public init(state: PlayerTransportState, actions: PlayerTransportActions, interaction: PlayerInteraction, fill: Binding<PlayerVideoFill>) {
        self.state = state
        self.actions = actions
        self.interaction = interaction
        _fill = fill
    }

    public var body: some View {
        GestureView(state: state, actions: actions, interaction: interaction, fill: $fill)
            .overlay { PlayerGestureFeedback(interaction: interaction) }
            .accessibilityHidden(true)
    }
}

/// Ripples, the skip counter, the volume and brightness HUD, and the 2x badge.
struct PlayerGestureFeedback: View {
    let interaction: PlayerInteraction

    var body: some View {
        GeometryReader { geo in
            ZStack {
                if let skip = interaction.skip {
                    SkipFeedback(skip: skip, size: geo.size)
                        .transition(.opacity)
                }
                ForEach(interaction.ripples) { ripple in
                    RippleView()
                        .position(ripple.location)
                }
                VStack(spacing: 8) {
                    if interaction.isSpeedBoosted {
                        PlayerHUDChip {
                            Image(systemName: "forward.fill")
                                .symbolEffect(.pulse, options: .repeating)
                            Text("2×").monospacedDigit()
                        }
                        .transition(.scale(scale: 0.8).combined(with: .opacity))
                    }
                    if let hud = interaction.hud {
                        HUDView(hud: hud)
                            .transition(.scale(scale: 0.85).combined(with: .opacity))
                    }
                    Spacer()
                }
                .padding(.top, 16)
                .frame(maxWidth: .infinity)
                .safeAreaPadding(.top)
            }
        }
        .allowsHitTesting(false)
    }
}

/// A small frosted capsule, for the HUD and badges.
struct PlayerHUDChip<Content: View>: View {
    @ViewBuilder var content: Content

    var body: some View {
        HStack(spacing: 8) { content }
            .font(.subheadline.weight(.semibold))
            .foregroundStyle(.white)
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .background(.ultraThinMaterial, in: .capsule)
            .environment(\.colorScheme, .dark)
            .shadow(color: .black.opacity(0.25), radius: 10, y: 4)
    }
}

private struct HUDView: View {
    let hud: PlayerInteraction.HUD

    var body: some View {
        switch hud {
        case .volume(let value):
            meter(value, symbol: value == 0 ? "speaker.slash.fill" : "speaker.wave.3.fill", label: "Volume")
        case .brightness(let value):
            meter(value, symbol: "sun.max.fill", label: "Brightness")
        case .fill(let fill):
            PlayerHUDChip {
                Image(systemName: fill.systemImage)
                Text(fill.label)
            }
        }
    }

    private func meter(_ value: Double, symbol: String, label: String) -> some View {
        PlayerHUDChip {
            Image(systemName: symbol, variableValue: value)
                .frame(width: 22)
                .contentTransition(.symbolEffect(.replace))
            Capsule()
                .fill(.white.opacity(0.25))
                .frame(width: 120, height: 5)
                .overlay(alignment: .leading) {
                    Capsule().fill(.white).frame(width: 120 * value)
                }
                .animation(.spring(response: 0.2, dampingFraction: 0.9), value: value)
        }
        .accessibilityElement()
        .accessibilityLabel(label)
        .accessibilityValue(Text(value, format: .percent.precision(.fractionLength(0))))
    }
}

private struct SkipFeedback: View {
    let skip: PlayerInteraction.Skip
    let size: CGSize

    var body: some View {
        let forward = skip.side == .forward
        let width = size.width * 0.38
        ZStack {
            // A wide arc hugging the side, like the edge of a ripple.
            Ellipse()
                .fill(.white.opacity(0.12))
                .frame(width: width * 2, height: size.height * 1.4)
                .offset(x: forward ? width : -width)
            VStack(spacing: 6) {
                HStack(spacing: -4) {
                    ForEach(0..<3) { i in
                        Image(systemName: forward ? "arrowtriangle.forward.fill" : "arrowtriangle.backward.fill")
                            .font(.system(size: 14))
                            .phaseAnimator([0.35, 1], trigger: skip.taps) { image, phase in
                                image.opacity(phase)
                            } animation: { _ in .easeInOut(duration: 0.18).delay(Double(forward ? i : 2 - i) * 0.07) }
                    }
                }
                Text("\(forward ? "+" : "−")\(Int(skip.seconds)) s")
                    .font(.headline.monospacedDigit())
                    .contentTransition(.numericText(value: skip.seconds))
            }
            .foregroundStyle(.white)
            .shadow(color: .black.opacity(0.4), radius: 6)
            .frame(width: width)
        }
        .frame(width: width, height: size.height)
        .clipped()
        .position(x: forward ? size.width - width / 2 : width / 2, y: size.height / 2)
        .animation(.spring(response: 0.25, dampingFraction: 0.8), value: skip.seconds)
    }
}

private struct RippleView: View {
    @State private var expanded = false

    var body: some View {
        Circle()
            .fill(.white.opacity(expanded ? 0 : 0.35))
            .frame(width: 180, height: 180)
            .scaleEffect(expanded ? 1 : 0.15)
            .onAppear {
                withAnimation(.easeOut(duration: 0.6)) { expanded = true }
            }
    }
}

// MARK: - The recognizers

private struct GestureView: UIViewRepresentable {
    let state: PlayerTransportState
    let actions: PlayerTransportActions
    let interaction: PlayerInteraction
    @Binding var fill: PlayerVideoFill

    func makeCoordinator() -> GestureCoordinator {
        GestureCoordinator()
    }

    func makeUIView(context: Context) -> GestureHostView {
        let view = GestureHostView()
        let coordinator = context.coordinator
        coordinator.attach(to: view, interaction: interaction)
        return view
    }

    func updateUIView(_ view: GestureHostView, context: Context) {
        let coordinator = context.coordinator
        coordinator.state = state
        coordinator.actions = actions
        coordinator.interaction = interaction
        coordinator.fill = $fill
    }
}

final class GestureHostView: UIView {
    var onWindow: ((UIWindow?) -> Void)?

    override func didMoveToWindow() {
        super.didMoveToWindow()
        onWindow?(window)
    }
}

final class GestureCoordinator: NSObject, UIGestureRecognizerDelegate {
    private enum PanMode { case scrub, volume, brightness }

    var state = PlayerTransportState()
    var actions = PlayerTransportActions(togglePause: {}, seek: { _ in }, close: {})
    var interaction: PlayerInteraction?
    var fill: Binding<PlayerVideoFill>?

    private weak var view: UIView?
    private var panMode: PanMode?
    private var lastPanX: CGFloat = 0
    private var pendingTap: (side: PlayerInteraction.SkipSide, time: ContinuousClock.Instant)?
    private var pendingTapTask: Task<Void, Never>?
    private var pinchHandled = false

    func attach(to view: GestureHostView, interaction: PlayerInteraction) {
        self.view = view
        self.interaction = interaction
        let volumeView = interaction.systemVolume.makeView()
        view.addSubview(volumeView)
        view.onWindow = { [weak interaction, weak volumeView] window in
            interaction?.screen = window?.windowScene?.screen
            if let volumeView { interaction?.systemVolume.attach(volumeView) }
        }

        let tap = UITapGestureRecognizer(target: self, action: #selector(tapped(_:)))
        let pan = UIPanGestureRecognizer(target: self, action: #selector(panned(_:)))
        pan.maximumNumberOfTouches = 1
        let pinch = UIPinchGestureRecognizer(target: self, action: #selector(pinched(_:)))
        let press = UILongPressGestureRecognizer(target: self, action: #selector(pressed(_:)))
        press.minimumPressDuration = 0.45
        press.allowableMovement = 14
        for recognizer in [tap, pan, pinch, press] as [UIGestureRecognizer] {
            recognizer.delegate = self
            view.addGestureRecognizer(recognizer)
        }
    }

    // MARK: Taps

    /// Taps in the middle third show or hide the controls at once. On the sides a tap waits a moment
    /// to see whether a second follows: two make a skip, and while a skip run is showing every tap
    /// on that side adds to it.
    @objc private func tapped(_ recognizer: UITapGestureRecognizer) {
        guard let view, let interaction else { return }
        let location = recognizer.location(in: view)
        let third = view.bounds.width / 3
        let side: PlayerInteraction.SkipSide? =
            location.x < third ? .backward : location.x > third * 2 ? .forward : nil
        let now = ContinuousClock.now

        guard let side, state.canSeek else {
            pendingTapTask?.cancel()
            pendingTap = nil
            interaction.toggleChrome()
            return
        }
        if interaction.continuesSkip(side) {
            interaction.skipTapped(side, at: location, state: state, actions: actions)
            return
        }
        if let pending = pendingTap, pending.side == side, now - pending.time < .milliseconds(300) {
            pendingTapTask?.cancel()
            pendingTap = nil
            interaction.skipTapped(side, at: location, state: state, actions: actions)
            return
        }
        pendingTap = (side, now)
        pendingTapTask?.cancel()
        pendingTapTask = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(300))
            guard !Task.isCancelled, let self, pendingTap != nil else { return }
            pendingTap = nil
            self.interaction?.toggleChrome()
        }
    }

    // MARK: Pans

    @objc private func panned(_ recognizer: UIPanGestureRecognizer) {
        guard let view, let interaction else { return }
        let translation = recognizer.translation(in: view)
        let velocity = recognizer.velocity(in: view)
        switch recognizer.state {
        case .began:
            if abs(velocity.x) > abs(velocity.y) {
                guard interaction.beginScrub(state: state) else { panMode = nil; return }
                panMode = .scrub
                lastPanX = translation.x
            } else {
                panMode = recognizer.location(in: view).x > view.bounds.midX ? .volume : .brightness
                lastPanX = translation.y
            }
        case .changed:
            switch panMode {
            case .scrub:
                interaction.moveScrub(
                    dx: translation.x - lastPanX, velocity: velocity.x, verticalOffset: translation.y,
                    width: view.bounds.width, state: state, actions: actions
                )
                lastPanX = translation.x
            case .volume, .brightness:
                // A drag over 70% of the height covers the whole range.
                let delta = -Double(translation.y - lastPanX) / Double(max(view.bounds.height * 0.7, 1))
                lastPanX = translation.y
                if panMode == .volume { interaction.adjustVolume(by: delta) } else { interaction.adjustBrightness(by: delta) }
            case nil:
                break
            }
        case .ended:
            if panMode == .scrub { interaction.endScrub(actions: actions) }
            panMode = nil
        case .cancelled, .failed:
            if panMode == .scrub { interaction.endScrub(cancelled: true, actions: actions) }
            panMode = nil
        default:
            break
        }
    }

    // MARK: Pinch and press

    @objc private func pinched(_ recognizer: UIPinchGestureRecognizer) {
        guard let interaction, let fill else { return }
        switch recognizer.state {
        case .began:
            pinchHandled = false
        case .changed:
            guard !pinchHandled else { return }
            let next: PlayerVideoFill? = recognizer.scale > 1.12 ? .fill : recognizer.scale < 0.88 ? .fit : nil
            guard let next else { return }
            pinchHandled = true
            if next != fill.wrappedValue {
                withAnimation(.spring(response: 0.4, dampingFraction: 0.85)) { fill.wrappedValue = next }
                UIImpactFeedbackGenerator(style: .medium).impactOccurred()
            }
            interaction.show(.fill(next))
        default:
            break
        }
    }

    @objc private func pressed(_ recognizer: UILongPressGestureRecognizer) {
        guard let interaction else { return }
        switch recognizer.state {
        case .began where !state.isPaused: interaction.setSpeedBoost(true, actions: actions)
        case .ended, .cancelled, .failed: interaction.setSpeedBoost(false, actions: actions)
        default: break
        }
    }

    // MARK: UIGestureRecognizerDelegate

    func gestureRecognizerShouldBegin(_ g: UIGestureRecognizer) -> Bool {
        if g is UILongPressGestureRecognizer { return actions.speedBoost != nil && !state.isPaused }
        return true
    }
}
