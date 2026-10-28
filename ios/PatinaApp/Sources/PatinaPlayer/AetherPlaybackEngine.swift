// The core's `PlaybackEngine` over AetherEngine: the iOS stand-in for `desktop/mpv`.
//
// The core drives it through the protocol and hears back through `onEvent`; the screen reads the
// observable state below instead of parsing events. AetherEngine is an ObservableObject whose
// clock ticks about ten times a second; here each value is its own observable property, so a view
// that shows the title does not redraw with the clock.

import AetherEngine
import Combine
import CoreGraphics
import Foundation
import Observation
import PatinaCore

/// What the player is doing, for the screen's spinner and play button.
nonisolated public enum PlayerPhase: Equatable, Sendable {
    case idle, loading, playing, paused, seeking, buffering, ended
    /// The source stopped delivering; `reconnecting` while the reader is still retrying.
    case stalled(reconnecting: Bool)
    case failed(String)

    public var isWaiting: Bool {
        switch self {
        case .loading, .seeking, .buffering, .stalled: true
        default: false
        }
    }
}

@Observable
public final class AetherPlaybackEngine: PlaybackEngine {
    // MARK: PlaybackEngine

    @ObservationIgnored public var onEvent: ((EngineEvent) -> Void)?
    @ObservationIgnored public var onRemoteCommand: ((RemoteCommand) -> Void)?

    // MARK: Observable state for the screen

    /// The playhead, about ten times a second.
    public private(set) var position: Double = 0
    /// nil while unknown and for live channels.
    public private(set) var duration: Double?
    /// How far the media is buffered, on the `position` axis.
    public private(set) var bufferedPosition: Double = 0
    public private(set) var isPaused = true
    public private(set) var phase: PlayerPhase = .idle
    /// The subtitle cues to draw now.
    public private(set) var subtitleCues: [SubtitleCue] = []
    /// The picture's presented size (coded size under the pixel aspect), nil before the first frame and for audio.
    public private(set) var videoSize: CGSize?
    /// Whether `scrubThumbnail(at:)` can answer for this item.
    public private(set) var supportsScrubThumbnails = false
    public private(set) var rate: Double = 1
    public private(set) var volume: Double = 1
    public private(set) var isMuted = false
    public private(set) var audioTracks: [EngineTrack] = []
    public private(set) var subtitleTracks: [EngineTrack] = []
    public private(set) var audioTrackID: Int?
    public private(set) var subtitle: EngineSubtitle?
    public private(set) var chapters: [EngineChapter] = []
    public private(set) var diagnostics = PatinaCore.EngineDiagnostics()
    public private(set) var isLive = false
    public private(set) var isAudioOnly = false
    /// True from `load` until `stop`, whatever the item is doing.
    public private(set) var hasItem = false

    public var videoAspect: CGFloat? {
        guard let videoSize, videoSize.height > 0 else { return nil }
        return videoSize.width / videoSize.height
    }

    // MARK: Parts

    /// The engine itself, for `PlayerSurface` to bind its view to.
    @ObservationIgnored public let engine: AetherEngine
    @ObservationIgnored public let pictureInPicture: PictureInPicture
    @ObservationIgnored private let nowPlaying: NowPlayingController

    // MARK: Per item

    @ObservationIgnored private var generation = 0
    @ObservationIgnored private var request: EngineLoad?
    /// `.loaded` was sent for the item; events before it would belong to no item the core knows.
    @ObservationIgnored private var itemLoaded = false
    @ObservationIgnored private var failureReported = false
    @ObservationIgnored private var endReported = false
    @ObservationIgnored private var lastTimeEvent: ContinuousClock.Instant?
    @ObservationIgnored private var allCues: [SubtitleCue] = []
    @ObservationIgnored private var subscriptions: Set<AnyCancellable> = []

    public init() throws {
        engine = try AetherEngine()
        engine.backgroundPlaybackEnabled = true
        nowPlaying = NowPlayingController(engine: engine)
        pictureInPicture = PictureInPicture(engine: engine)
        nowPlaying.onCommand = { [weak self] command in self?.onRemoteCommand?(command) }
        observe()
    }

    // MARK: - PlaybackEngine

    public func load(_ request: EngineLoad) async throws {
        generation += 1
        let generation = generation
        self.request = request
        itemLoaded = false
        failureReported = false
        endReported = false
        lastTimeEvent = nil
        hasItem = true
        isLive = request.live
        isAudioOnly = request.audioOnly
        position = request.startSeconds
        duration = request.live ? nil : request.nowPlaying.durationSeconds
        bufferedPosition = 0
        allCues = []
        subtitleCues = []
        videoSize = nil
        phase = .loading
        rate = 1
        nowPlaying.begin(request.nowPlaying, audioOnly: request.audioOnly, live: request.live)

        await AudioSession.activate(audioOnly: request.audioOnly)
        guard generation == self.generation, !Task.isCancelled else { throw CancellationError() }

        var options = LoadOptions(
            httpHeaders: request.httpHeaders, isLive: request.live, audioOnly: request.audioOnly,
            externalSubtitles: request.externalSubtitles.map { EngineMapping.externalTrack($0, headers: request.httpHeaders) }
        )
        options.initialSubtitleTrackID = request.subtitle.map(EngineMapping.subtitleTrackID)
        do {
            try await engine.load(
                url: request.url,
                startPosition: request.startSeconds > 0 ? request.startSeconds : nil,
                options: options,
                audioSourceStreamIndex: request.audioStreamIndex.map(Int32.init)
            )
        } catch is CancellationError {
            throw CancellationError()
        } catch {
            guard generation == self.generation else { throw CancellationError() }
            fail(engine.errorInfo?.message ?? error.localizedDescription,
                 kind: engine.errorInfo.map { EngineMapping.failureKind($0) } ?? EngineMapping.failureKind(of: error))
            return
        }
        guard generation == self.generation else { throw CancellationError() }
        // The engine may have failed after opening (state .error), which the state sink reported.
        guard !failureReported else { return }

        itemLoaded = true
        readTracks()
        chapters = engine.mediaChapters.map(EngineMapping.chapter)
        readDuration(engine.duration)
        emit(.loaded(durationSeconds: duration, tracks: audioTracks + subtitleTracks, chapters: chapters))
        audioTrackID = engine.activeAudioTrackIndex
        subtitle = EngineMapping.subtitle(trackID: engine.activeSubtitleTrackIndex)
        emit(.audioTrack(audioTrackID))
        emit(.subtitleTrack(subtitle))
        emit(.paused(isPaused))
        emit(.time(positionSeconds: engine.currentTime))
        updateDiagnostics()
        updateThumbnailSupport()
        nowPlaying.refresh()
    }

    public func play() {
        engine.play()
    }

    public func pause() {
        engine.pause()
    }

    public func togglePause() {
        isPaused ? play() : pause()
    }

    public func seek(toSeconds seconds: Double) async {
        let target = clampedSeekTarget(seconds)
        position = target
        await engine.seek(to: target)
        emitTime(force: true)
        nowPlaying.playbackChanged()
    }

    public func setRate(_ rate: Double) {
        let clamped = min(max(rate, 0.25), maxRate)
        self.rate = clamped
        engine.setRate(Float(clamped))
        nowPlaying.playbackChanged()
    }

    public func setVolume(_ volume: Double) {
        self.volume = min(max(volume, 0), 1)
        applyVolume()
    }

    public func setMuted(_ muted: Bool) {
        isMuted = muted
        applyVolume()
    }

    public func selectAudio(streamIndex: Int) async {
        guard streamIndex != engine.activeAudioTrackIndex else { return }
        engine.selectAudioTrack(index: streamIndex)
        // The switch rebuilds the session at the playhead; it is done when the engine names the new
        // track and the rebuilt session is ready. The engine logs and ignores a switch it refuses.
        let deadline = ContinuousClock.now + .seconds(15)
        while ContinuousClock.now < deadline, !Task.isCancelled, hasItem {
            if engine.activeAudioTrackIndex == streamIndex, engine.isSessionReady { break }
            try? await Task.sleep(for: .milliseconds(100))
        }
    }

    public func selectSubtitle(_ subtitle: EngineSubtitle?) {
        if let subtitle {
            engine.selectSubtitleTrack(index: EngineMapping.subtitleTrackID(subtitle))
        } else {
            engine.clearSubtitle()
        }
    }

    public func stop() {
        generation += 1
        request = nil
        itemLoaded = false
        hasItem = false
        if pictureInPicture.isActive { pictureInPicture.stop() }
        engine.stop()
        nowPlaying.end()
        AudioSession.deactivate()
        position = 0
        duration = nil
        bufferedPosition = 0
        allCues = []
        subtitleCues = []
        videoSize = nil
        audioTracks = []
        subtitleTracks = []
        chapters = []
        audioTrackID = nil
        subtitle = nil
        supportsScrubThumbnails = false
        phase = .idle
        isPaused = true
        rate = 1
    }

    public var positionSeconds: Double { engine.currentTime }
    public var durationSeconds: Double? { duration }
    public var maxRate: Double { Double(engine.maxSupportedRate) }

    // MARK: - Beyond the protocol

    /// A still of the picture at `seconds` from what the session already holds, for the scrub bubble.
    /// nil when that part is not buffered.
    public func scrubThumbnail(at seconds: Double, maxWidth: Int = 320) async -> CGImage? {
        guard supportsScrubThumbnails else { return nil }
        return await engine.scrubThumbnail(atSeconds: seconds, maxWidth: maxWidth)
    }

    /// Which of the lock screen's next and previous buttons work.
    public func setRemoteQueue(hasPrevious: Bool, hasNext: Bool) {
        nowPlaying.hasPrevious = hasPrevious
        nowPlaying.hasNext = hasNext
    }

    /// The lock screen's skip buttons, in seconds (10 and 10 by default).
    public var remoteSkipSeconds: (backward: Double, forward: Double) {
        get { nowPlaying.skipSeconds }
        set { nowPlaying.skipSeconds = newValue }
    }

    // MARK: - Engine to state and events

    private func observe() {
        let clock = engine.clock
        clock.$currentTime
            .sink { [weak self] time in self?.clockTicked(time) }
            .store(in: &subscriptions)
        clock.$sourceTime
            .sink { [weak self] time in self?.sourceTimeTicked(time) }
            .store(in: &subscriptions)
        clock.$bufferedPosition
            .sink { [weak self] buffered in self?.bufferedPosition = buffered }
            .store(in: &subscriptions)
        engine.$state
            .sink { [weak self] state in self?.stateChanged(state) }
            .store(in: &subscriptions)
        engine.$playbackPhase
            .sink { [weak self] phase in self?.phaseChanged(phase) }
            .store(in: &subscriptions)
        engine.$duration
            .sink { [weak self] duration in self?.readDuration(duration) }
            .store(in: &subscriptions)
        engine.$subtitleCues
            .sink { [weak self] cues in
                guard let self else { return }
                allCues = cues
                setActiveCues(at: engine.clock.sourceTime)
            }
            .store(in: &subscriptions)
        engine.$audioTracks.combineLatest(engine.$subtitleTracks)
            .sink { [weak self] _, _ in Task { @MainActor in self?.readTracks() } }
            .store(in: &subscriptions)
        engine.$mediaChapters
            .sink { [weak self] chapters in self?.chapters = chapters.map(EngineMapping.chapter) }
            .store(in: &subscriptions)
        engine.$activeAudioTrackIndex
            .removeDuplicates()
            .sink { [weak self] id in
                guard let self, itemLoaded, id != audioTrackID else { return }
                audioTrackID = id
                emit(.audioTrack(id))
            }
            .store(in: &subscriptions)
        engine.$activeSubtitleTrackIndex
            .removeDuplicates()
            .sink { [weak self] id in
                guard let self else { return }
                let subtitle = EngineMapping.subtitle(trackID: id)
                guard itemLoaded, subtitle != self.subtitle else { return }
                self.subtitle = subtitle
                emit(.subtitleTrack(subtitle))
            }
            .store(in: &subscriptions)
        // Published in willSet, so read the engine on the next turn, when every field has its new value.
        Publishers.MergeMany(
            engine.$sourceVideoWidth.map { _ in () }.eraseToAnyPublisher(),
            engine.$softwareDisplaySize.map { _ in () }.eraseToAnyPublisher(),
            engine.$videoRoute.map { _ in () }.eraseToAnyPublisher(),
            engine.$activeVideoDecoder.map { _ in () }.eraseToAnyPublisher(),
            engine.$videoFormat.map { _ in () }.eraseToAnyPublisher(),
            engine.$isSessionReady.map { _ in () }.eraseToAnyPublisher(),
            engine.diagnostics.$liveTelemetry.map { _ in () }.eraseToAnyPublisher()
        )
        .receive(on: DispatchQueue.main)
        .sink { [weak self] in
            guard let self else { return }
            videoSize = isAudioOnly ? nil : EngineMapping.displaySize(
                width: engine.sourceVideoWidth, height: engine.sourceVideoHeight,
                pixelAspect: engine.sourceVideoPixelAspectRatio, software: engine.softwareDisplaySize
            )
            updateDiagnostics()
            updateThumbnailSupport()
        }
        .store(in: &subscriptions)
        engine.$videoRoute.combineLatest(engine.$currentAVPlayer.map { $0 != nil })
            .receive(on: DispatchQueue.main)
            .sink { [weak self] _ in self?.nowPlaying.refresh() }
            .store(in: &subscriptions)
    }

    private func clockTicked(_ time: Double) {
        position = time
        emitTime(force: false)
    }

    private func sourceTimeTicked(_ time: Double) {
        setActiveCues(at: time)
    }

    private func setActiveCues(at time: Double) {
        let active = EngineMapping.activeCues(allCues, at: time)
        if active.map(\.id) != subtitleCues.map(\.id) { subtitleCues = active }
    }

    private func emitTime(force: Bool) {
        guard itemLoaded else { return }
        let now = ContinuousClock.now
        if !force, let last = lastTimeEvent, now - last < .milliseconds(250) { return }
        lastTimeEvent = now
        emit(.time(positionSeconds: engine.currentTime))
    }

    private func stateChanged(_ state: AetherPlaybackState) {
        switch state {
        case .playing:
            setPaused(false)
        case .paused, .idle:
            setPaused(true)
        case .ended:
            setPaused(true)
            guard itemLoaded, !endReported else { return }
            endReported = true
            emit(.ended)
        case .error(let message):
            setPaused(true)
            // `errorInfo` is assigned before `state`, so it is this failure's.
            if hasItem { fail(message, kind: EngineMapping.failureKind(engine.errorInfo)) }
        case .loading, .seeking:
            break
        }
        // A pause or a resume moves the shared card's clock (the sessions follow their player).
        Task { @MainActor [weak self] in self?.nowPlaying.playbackChanged() }
    }

    private func phaseChanged(_ phase: PlaybackPhase) {
        self.phase = switch phase {
        case .idle: hasItem ? .loading : .idle
        case .loading: .loading
        case .playing: .playing
        case .paused: .paused
        case .seeking: .seeking
        case .rebuffering: .buffering
        case .stalled(let reconnecting): .stalled(reconnecting: reconnecting)
        case .ended: .ended
        case .error(let message): .failed(message)
        }
        Task { @MainActor [weak self] in self?.updateDiagnostics() }
    }

    private func setPaused(_ paused: Bool) {
        guard paused != isPaused else { return }
        isPaused = paused
        if itemLoaded { emit(.paused(paused)) }
    }

    private func fail(_ message: String, kind: EngineFailureKind) {
        guard !failureReported else { return }
        failureReported = true
        phase = .failed(message)
        emit(.failed(message: message, kind: kind))
    }

    private func readDuration(_ seconds: Double) {
        guard !isLive else { duration = nil; return }
        if seconds > 0, seconds.isFinite { duration = seconds }
    }

    private func readTracks() {
        audioTracks = engine.audioTracks.map { EngineMapping.track($0, kind: .audio) }
        subtitleTracks = engine.subtitleTracks.map { EngineMapping.track($0, kind: .subtitle) }
    }

    private func updateThumbnailSupport() {
        supportsScrubThumbnails = hasItem && !isAudioOnly && engine.supportsCacheBackedStills
    }

    private func updateDiagnostics() {
        let telemetry = engine.diagnostics.liveTelemetry
        let size = videoSize.map { "\(Int($0.width.rounded()))×\(Int($0.height.rounded()))" }
        let buffered = telemetry?.forwardBufferSeconds ?? max(0, engine.clock.bufferedPosition - engine.currentTime)
        let next = PatinaCore.EngineDiagnostics(
            buffering: phase.isWaiting,
            // Whole tenths, so the panel is not redrawn by every tick of the buffer.
            bufferedSeconds: hasItem ? (buffered * 10).rounded() / 10 : nil,
            decoder: isAudioOnly ? engine.activeAudioDecoder : engine.activeVideoDecoder,
            videoFormat: isAudioOnly || videoSize == nil ? nil : EngineMapping.format(engine.videoFormat),
            route: EngineMapping.route(engine.videoRoute),
            droppedFrames: telemetry?.droppedFrameCount,
            videoSize: size
        )
        guard next != diagnostics else { return }
        diagnostics = next
        if itemLoaded { emit(.diagnostics(next)) }
    }

    private func applyVolume() {
        engine.volume = Float(isMuted ? 0 : volume)
    }

    private func clampedSeekTarget(_ seconds: Double) -> Double {
        var target = max(0, seconds)
        if let duration { target = min(target, max(0, duration - 0.5)) }
        if isLive, let range = engine.seekableLiveRange { target = min(max(target, range.lowerBound), range.upperBound) }
        return target
    }

    private func emit(_ event: EngineEvent) {
        onEvent?(event)
    }
}
