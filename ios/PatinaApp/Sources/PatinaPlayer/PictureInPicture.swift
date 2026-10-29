// Picture in Picture over AetherEngine's layers (no desktop twin: mpv's window is already a window).
//
// The native path plays into the engine's own `AVPlayerLayer`; the software path into an
// `AVSampleBufferDisplayLayer`, whose window asks the engine for its time range and transport.
// The controller is rebuilt when the engine changes route, never while the window is up. The
// engine is told the window is up (`pictureInPictureActive`) so it keeps the pipeline alive in
// the background and hands the item over in place on the next load.

import AVKit
import AetherEngine
import Combine
import Observation

@Observable
public final class PictureInPicture {
    /// Whether `start()` can open the window now.
    public private(set) var isPossible = false
    public private(set) var isActive = false
    /// Called when the user taps the window's "return to app" button: bring the player back on screen.
    @ObservationIgnored public var onRestore: (() -> Void)?

    @ObservationIgnored private unowned let engine: AetherEngine
    @ObservationIgnored private var controller: AVPictureInPictureController?
    @ObservationIgnored private var layer: CALayer?
    @ObservationIgnored private var possibleObservation: NSKeyValueObservation?
    @ObservationIgnored private var delegate: Delegate?
    @ObservationIgnored private var sampleBufferDelegate: SampleBufferDelegate?
    @ObservationIgnored private var subscriptions: Set<AnyCancellable> = []

    init(engine: AetherEngine) {
        self.engine = engine
        guard AVPictureInPictureController.isPictureInPictureSupported() else { return }
        engine.$videoRoute.combineLatest(engine.$currentAVPlayer.map { $0 != nil }, engine.$softwarePiPSource.map { $0 != nil })
            .receive(on: DispatchQueue.main)
            .sink { [weak self] _ in self?.rebuild() }
            .store(in: &subscriptions)
    }

    public func start() {
        guard let controller, controller.isPictureInPicturePossible else { return }
        controller.startPictureInPicture()
    }

    public func stop() {
        controller?.stopPictureInPicture()
    }

    public func toggle() {
        isActive ? stop() : start()
    }

    private func rebuild() {
        guard !isActive else { return }
        let wanted: CALayer? =
            switch engine.videoRoute {
            case .software: engine.softwarePiPSource?.layer
            case .loopback, .remoteBypass: engine.nativePlayerLayer
            case .none, .audio: nil
            }
        guard wanted !== layer else { return }
        possibleObservation = nil
        controller = nil
        layer = wanted
        isPossible = false

        let source: AVPictureInPictureController.ContentSource
        if let playerLayer = wanted as? AVPlayerLayer {
            source = .init(playerLayer: playerLayer)
            sampleBufferDelegate = nil
        } else if let displayLayer = wanted as? AVSampleBufferDisplayLayer, let pip = engine.softwarePiPSource {
            let playback = SampleBufferDelegate(source: pip)
            sampleBufferDelegate = playback
            source = .init(sampleBufferDisplayLayer: displayLayer, playbackDelegate: playback)
        } else {
            return
        }
        let controller = AVPictureInPictureController(contentSource: source)
        controller.canStartPictureInPictureAutomaticallyFromInline = true
        let delegate = Delegate(owner: self)
        controller.delegate = delegate
        self.delegate = delegate
        self.controller = controller
        possibleObservation = controller.observe(\.isPictureInPicturePossible, options: [.initial, .new]) { @Sendable [weak self] controller, _ in
            let possible = controller.isPictureInPicturePossible
            Task { @MainActor in self?.isPossible = possible }
        }
    }

    fileprivate func activeChanged(_ active: Bool) {
        isActive = active
        engine.pictureInPictureActive = active
        if !active { rebuild() }
    }

    /// The window calls back on the main thread.
    private final class Delegate: NSObject, AVPictureInPictureControllerDelegate {
        weak var owner: PictureInPicture?

        init(owner: PictureInPicture) { self.owner = owner }

        nonisolated func pictureInPictureControllerWillStartPictureInPicture(_ controller: AVPictureInPictureController) {
            MainActor.assumeIsolated { owner?.activeChanged(true) }
        }

        nonisolated func pictureInPictureControllerDidStopPictureInPicture(_ controller: AVPictureInPictureController) {
            MainActor.assumeIsolated { owner?.activeChanged(false) }
        }

        nonisolated func pictureInPictureController(_ controller: AVPictureInPictureController, failedToStartPictureInPictureWithError error: any Error) {
            MainActor.assumeIsolated { owner?.activeChanged(false) }
        }

        nonisolated func pictureInPictureController(
            _ controller: AVPictureInPictureController,
            restoreUserInterfaceForPictureInPictureStopWithCompletionHandler completionHandler: @escaping (Bool) -> Void
        ) {
            MainActor.assumeIsolated { owner?.onRestore?() }
            completionHandler(true)
        }
    }

    /// Transport for the software path's window, answered on the axis of the frames it shows.
    private final class SampleBufferDelegate: NSObject, AVPictureInPictureSampleBufferPlaybackDelegate {
        let source: SoftwarePiPSource

        init(source: SoftwarePiPSource) { self.source = source }

        nonisolated func pictureInPictureController(_ controller: AVPictureInPictureController, setPlaying playing: Bool) {
            MainActor.assumeIsolated { source.setPlaying(playing) }
        }

        nonisolated func pictureInPictureControllerTimeRangeForPlayback(_ controller: AVPictureInPictureController) -> CMTimeRange {
            MainActor.assumeIsolated { source.timeRange() }
        }

        nonisolated func pictureInPictureControllerIsPlaybackPaused(_ controller: AVPictureInPictureController) -> Bool {
            MainActor.assumeIsolated { source.isPaused }
        }

        nonisolated func pictureInPictureController(_ controller: AVPictureInPictureController, didTransitionToRenderSize newRenderSize: CMVideoDimensions) {}

        nonisolated func pictureInPictureController(
            _ controller: AVPictureInPictureController, skipByInterval skipInterval: CMTime,
            completion completionHandler: @escaping @Sendable () -> Void
        ) {
            MainActor.assumeIsolated { source.skip(by: skipInterval.seconds) }
            completionHandler()
        }
    }
}
