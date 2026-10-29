// What the player's touch layer, scrub bar and keyboard share while the user handles the
// controls (no desktop twin: mpv's OSD did this). One instance per player screen.

import AVFoundation
import MediaPlayer
import Observation
import SwiftUI
import UIKit

@Observable
public final class PlayerInteraction {
    /// The heads-up display shown for a moment after a change that has no chrome of its own.
    public enum HUD: Equatable {
        case volume(Double)
        case brightness(Double)
        case fill(PlayerVideoFill)
    }

    /// How finely a drag moves the playhead; the further the finger strays from the bar, the finer.
    public enum ScrubPrecision: Equatable {
        case normal, half, quarter, fine

        var factor: Double {
            switch self {
            case .normal: 1
            case .half: 0.5
            case .quarter: 0.25
            case .fine: 0.1
            }
        }

        var label: String? {
            switch self {
            case .normal: nil
            case .half: "Half-Speed Scrubbing"
            case .quarter: "Quarter-Speed Scrubbing"
            case .fine: "Fine Scrubbing"
            }
        }

        init(verticalOffset: CGFloat) {
            switch abs(verticalOffset) {
            case ..<50: self = .normal
            case ..<110: self = .half
            case ..<170: self = .quarter
            default: self = .fine
            }
        }
    }

    public struct Scrub: Equatable {
        /// Where the playhead was when the drag began.
        public var origin: Double
        /// Where it lands on release.
        public var target: Double
        public var precision: ScrubPrecision
        /// Snapped to a chapter or marker edge.
        public var snapped: Bool
        /// Unsnapped position, so a drag can leave a snap point again.
        var raw: Double
        var duration: Double
    }

    public enum SkipSide: Equatable { case backward, forward }

    /// Repeated double-taps on one side, adding up.
    public struct Skip: Equatable {
        public var side: SkipSide
        public var seconds: Double
        /// Counts taps, so each one replays the ripple.
        public var taps: Int
        public var origin: Double
    }

    public struct Ripple: Identifiable, Equatable {
        public let id: Int
        public let location: CGPoint
    }

    // MARK: State

    public private(set) var chromeVisible = true
    public private(set) var scrub: Scrub?
    public private(set) var scrubThumbnail: CGImage?
    public private(set) var hud: HUD?
    public private(set) var skip: Skip?
    public private(set) var ripples: [Ripple] = []
    public private(set) var isSpeedBoosted = false
    /// The chrome stays up while this is set (a menu is open, say).
    public var keepsChromeVisible = false {
        didSet { keepsChromeVisible ? cancelAutoHide() : scheduleAutoHide() }
    }

    /// How long the chrome stays up without a touch while playing.
    @ObservationIgnored public var autoHideDelay: Duration = .seconds(3)

    @ObservationIgnored private var isPlaying = false
    @ObservationIgnored private var autoHideTask: Task<Void, Never>?
    @ObservationIgnored private var hudTask: Task<Void, Never>?
    @ObservationIgnored private var skipTask: Task<Void, Never>?
    @ObservationIgnored private var thumbnailTask: Task<Void, Never>?
    @ObservationIgnored private var thumbnailWanted: Double?
    @ObservationIgnored private var lastPreview: ContinuousClock.Instant?
    @ObservationIgnored private var nextRipple = 0
    @ObservationIgnored private let selection = UISelectionFeedbackGenerator()
    @ObservationIgnored private let impact = UIImpactFeedbackGenerator(style: .light)
    @ObservationIgnored let systemVolume = SystemVolume()
    @ObservationIgnored weak var screen: UIScreen?

    public init() {
        systemVolume.onExternalChange = { [weak self] volume in self?.show(.volume(volume)) }
    }

    // MARK: Chrome

    /// The player started or stopped playing: the chrome hides itself only while playing.
    public func setPlaying(_ playing: Bool) {
        isPlaying = playing
        if playing { scheduleAutoHide() } else { cancelAutoHide(); setChrome(true) }
    }

    public func toggleChrome() {
        setChrome(!chromeVisible)
        if chromeVisible { scheduleAutoHide() }
    }

    public func showChrome() {
        setChrome(true)
        scheduleAutoHide()
    }

    public func hideChrome() {
        guard scrub == nil, !keepsChromeVisible else { return }
        setChrome(false)
    }

    /// A touch on the chrome: keep it up a while longer.
    public func poke() {
        if chromeVisible { scheduleAutoHide() }
    }

    private func setChrome(_ visible: Bool) {
        guard visible != chromeVisible else { return }
        withAnimation(.spring(response: 0.32, dampingFraction: 0.9)) { chromeVisible = visible }
    }

    private func scheduleAutoHide() {
        cancelAutoHide()
        guard isPlaying, chromeVisible, !keepsChromeVisible else { return }
        let delay = autoHideDelay
        autoHideTask = Task { [weak self] in
            try? await Task.sleep(for: delay)
            // A menu hangs off its button in the chrome, and hiding the chrome closes it: wait until
            // it is closed, then give the controls their full delay again.
            guard Self.isMenuOpen() else { self?.autoHideFired(); return }
            while !Task.isCancelled, Self.isMenuOpen() { try? await Task.sleep(for: .milliseconds(300)) }
            try? await Task.sleep(for: delay)
            self?.autoHideFired()
        }
    }

    private func autoHideFired() {
        guard !Task.isCancelled, scrub == nil, isPlaying else { return }
        hideChrome()
    }

    /// Whether a menu is open. SwiftUI says nothing of it; UIKit hosts an open menu in a container
    /// view of its own at the top of the window.
    static func isMenuOpen() -> Bool {
        UIApplication.shared.connectedScenes.contains { scene in
            (scene as? UIWindowScene)?.windows.contains { window in
                !window.isHidden && window.subviews.contains { NSStringFromClass(type(of: $0)).hasSuffix("ContextMenuContainerView") }
            } ?? false
        }
    }

    private func cancelAutoHide() {
        autoHideTask?.cancel()
        autoHideTask = nil
    }

    // MARK: Scrubbing

    /// A drag on the picture or the bar began.
    func beginScrub(state: PlayerTransportState) -> Bool {
        guard state.isScrubbable, let duration = state.duration else { return false }
        cancelAutoHide()
        impact.prepare()
        selection.prepare()
        scrubThumbnail = nil
        withAnimation(.spring(response: 0.28, dampingFraction: 0.85)) {
            chromeVisible = true
            scrub = Scrub(origin: state.position, target: state.position, precision: .normal, snapped: false, raw: state.position, duration: duration)
        }
        return true
    }

    /// The finger moved `dx` points along a track `width` points wide, `verticalOffset` points away
    /// from where it started. With a `velocity` (points a second) the drag accelerates; without one
    /// (the bar) the knob follows the finger.
    func moveScrub(dx: CGFloat, velocity: CGFloat?, verticalOffset: CGFloat, width: CGFloat,
                   state: PlayerTransportState, actions: PlayerTransportActions) {
        guard var scrub, width > 0 else { return }
        let precision = ScrubPrecision(verticalOffset: verticalOffset)
        let perPoint = precision.factor * (velocity.map { Self.secondsPerPoint(duration: scrub.duration, width: width, velocity: $0) }
            ?? scrub.duration / Double(width))
        scrub.raw = min(max(scrub.raw + Double(dx) * perPoint, 0), scrub.duration)

        // Snap to the nearest chapter or marker edge within a few points, once per arrival.
        let radius = perPoint * 8
        let snap = state.snapPoints.min { abs($0 - scrub.raw) < abs($1 - scrub.raw) }.flatMap { abs($0 - scrub.raw) <= radius ? $0 : nil }
        if let snap {
            if !scrub.snapped || scrub.target != snap { selection.selectionChanged() }
            scrub.target = snap
            scrub.snapped = true
        } else {
            scrub.target = scrub.raw
            scrub.snapped = false
        }
        if precision != scrub.precision { impact.impactOccurred(intensity: 0.5) }
        scrub.precision = precision
        self.scrub = scrub
        follow(scrub.target, state: state, actions: actions)
    }

    /// Jump the scrub to an absolute position (a tap on the bar, a key).
    func setScrub(to seconds: Double, state: PlayerTransportState, actions: PlayerTransportActions) {
        guard var scrub else { return }
        scrub.raw = min(max(seconds, 0), scrub.duration)
        scrub.target = scrub.raw
        scrub.snapped = false
        self.scrub = scrub
        follow(scrub.target, state: state, actions: actions)
    }

    /// The finger lifted: seek, unless `cancelled`.
    func endScrub(cancelled: Bool = false, actions: PlayerTransportActions) {
        guard let scrub else { return }
        thumbnailTask?.cancel()
        thumbnailTask = nil
        thumbnailWanted = nil
        if !cancelled {
            actions.seek(scrub.target)
        } else {
            actions.scrubPreview?(scrub.origin)
        }
        withAnimation(.spring(response: 0.3, dampingFraction: 0.9)) {
            self.scrub = nil
            scrubThumbnail = nil
        }
        scheduleAutoHide()
    }

    /// Slow drags move a little, fast ones a lot: at walking pace a full-width drag covers a tenth
    /// of the item, flung it covers all of it.
    nonisolated static func secondsPerPoint(duration: Double, width: CGFloat, velocity: CGFloat) -> Double {
        let full = duration / Double(width)
        let speed = Double(abs(velocity))
        let t = min(max((speed - 60) / (1400 - 60), 0), 1)
        let eased = t * t * (3 - 2 * t)
        return max(full * (0.1 + 0.9 * eased), 0.04)
    }

    /// Keep the bubble's thumbnail (or, without thumbnails, the picture itself) near the target.
    private func follow(_ target: Double, state: PlayerTransportState, actions: PlayerTransportActions) {
        if state.supportsThumbnails, let thumbnail = actions.thumbnail {
            thumbnailWanted = target
            guard thumbnailTask == nil else { return }
            thumbnailTask = Task { [weak self] in
                // One request at a time, always for the latest position.
                while let self, let wanted = thumbnailWanted, !Task.isCancelled {
                    thumbnailWanted = nil
                    if let image = await thumbnail(wanted), !Task.isCancelled, self.scrub != nil {
                        scrubThumbnail = image
                    }
                }
                // A cancelled run was replaced or ended by `endScrub`, which owns the slot now.
                if !Task.isCancelled { self?.thumbnailTask = nil }
            }
        } else if let preview = actions.scrubPreview {
            let now = ContinuousClock.now
            if let last = lastPreview, now - last < .milliseconds(250) { return }
            lastPreview = now
            preview(target)
        }
    }

    // MARK: Double-tap skips

    /// One more double-tap on `side`: add ten seconds to the run and seek there.
    func skipTapped(_ side: SkipSide, at location: CGPoint, state: PlayerTransportState, actions: PlayerTransportActions) {
        guard state.canSeek else { return }
        var run = skip?.side == side ? skip! : Skip(side: side, seconds: 0, taps: 0, origin: state.position)
        run.seconds += 10
        run.taps += 1
        var target = run.origin + (side == .forward ? run.seconds : -run.seconds)
        target = max(0, target)
        if let duration = state.duration { target = min(target, max(0, duration - 1)) }
        withAnimation(.spring(response: 0.25, dampingFraction: 0.8)) { skip = run }
        nextRipple += 1
        ripples.append(Ripple(id: nextRipple, location: location))
        let ripple = nextRipple
        Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(650))
            self?.ripples.removeAll { $0.id == ripple }
        }
        impact.impactOccurred(intensity: 0.6)
        actions.seek(target)
        skipTask?.cancel()
        skipTask = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(800))
            guard !Task.isCancelled else { return }
            withAnimation(.easeOut(duration: 0.25)) { self?.skip = nil }
        }
    }

    /// Whether another tap on `side` continues the current run.
    func continuesSkip(_ side: SkipSide) -> Bool {
        skip?.side == side
    }

    // MARK: Hold for speed

    func setSpeedBoost(_ on: Bool, actions: PlayerTransportActions) {
        guard on != isSpeedBoosted, let boost = actions.speedBoost else { return }
        if on { impact.impactOccurred() }
        withAnimation(.spring(response: 0.3, dampingFraction: 0.8)) { isSpeedBoosted = on }
        boost(on)
    }

    // MARK: Volume, brightness and the HUD

    public func show(_ hud: HUD) {
        if self.hud == nil {
            withAnimation(.spring(response: 0.25, dampingFraction: 0.85)) { self.hud = hud }
        } else {
            self.hud = hud
        }
        hudTask?.cancel()
        hudTask = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(900))
            guard !Task.isCancelled else { return }
            withAnimation(.easeOut(duration: 0.3)) { self?.hud = nil }
        }
    }

    /// Change the system volume by `delta` (0 to 1).
    public func adjustVolume(by delta: Double) {
        let volume = systemVolume.set(systemVolume.volume + delta)
        show(.volume(volume))
    }

    /// Change the screen's brightness by `delta` (0 to 1).
    public func adjustBrightness(by delta: Double) {
        guard let screen else { return }
        let value = min(max(Double(screen.brightness) + delta, 0), 1)
        screen.brightness = CGFloat(value)
        show(.brightness(value))
    }
}

/// The system volume, set through a hidden `MPVolumeView` (the only way an app may) and read back
/// from the audio session. While that view is on screen the system shows no volume HUD of its
/// own, so a change from the hardware buttons is reported for the player's HUD instead.
final class SystemVolume {
    var onExternalChange: ((Double) -> Void)?
    private(set) var volume: Double = Double(AVAudioSession.sharedInstance().outputVolume)
    private weak var slider: UISlider?
    private var observation: NSKeyValueObservation?
    private var ownChangeUntil: ContinuousClock.Instant?

    init() {
        observation = AVAudioSession.sharedInstance().observe(\.outputVolume, options: [.new]) { @Sendable [weak self] _, change in
            guard let value = change.newValue else { return }
            Task { @MainActor in self?.observed(Double(value)) }
        }
    }

    /// The view that carries the slider; it must be in a window.
    func makeView() -> MPVolumeView {
        let view = MPVolumeView(frame: CGRect(x: -100, y: -100, width: 40, height: 40))
        view.alpha = 0.001
        view.isUserInteractionEnabled = false
        view.showsVolumeSlider = true
        slider = view.subviews.compactMap { $0 as? UISlider }.first
        return view
    }

    func attach(_ view: MPVolumeView) {
        if slider == nil { slider = view.subviews.compactMap { $0 as? UISlider }.first }
    }

    @discardableResult
    func set(_ value: Double) -> Double {
        let clamped = min(max(value, 0), 1)
        volume = clamped
        ownChangeUntil = .now + .milliseconds(400)
        slider?.setValue(Float(clamped), animated: false)
        slider?.sendActions(for: .valueChanged)
        return clamped
    }

    private func observed(_ value: Double) {
        // The system rounds the volume to its own steps and reports our changes back rounded. Taking
        // those reports would undo every small step of a drag, so the volume would stick on a step.
        if let until = ownChangeUntil, ContinuousClock.now < until { return }
        guard abs(value - volume) > 0.001 else { return }
        volume = value
        onExternalChange?(value)
    }
}
