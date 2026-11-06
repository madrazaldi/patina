// A `PlaybackEngine` for the player tests (no TypeScript twin: the desktop tests drive a real
// mpv). It records what the core asks for; the test plays the engine's part by sending the
// events an engine would (`open`, `time`, `end`, `fail`).

import Foundation
import PlexKit

@testable import PatinaCore

final class FakeEngine: PlaybackEngine {
    var onEvent: ((EngineEvent) -> Void)?
    var onRemoteCommand: ((RemoteCommand) -> Void)?

    var loads: [EngineLoad] = []
    var seeks: [Double] = []
    var positionSeconds: Double = 0
    var durationSeconds: Double?
    var isPaused = false
    var maxRate: Double = 2
    var rate: Double = 1
    var volume: Double = 1
    var muted = false
    var audio: Int?
    var subtitle: EngineSubtitle?
    var stops = 0
    var remote: (hasPrevious: Bool, hasNext: Bool)?
    /// What the mock server had logged when each load arrived.
    var loggedAtLoad: [[String]] = []
    var mock: MockPMS?

    func load(_ request: EngineLoad) async throws {
        loads.append(request)
        loggedAtLoad.append(mock?.logged() ?? [])
        positionSeconds = request.startSeconds
        durationSeconds = nil
        isPaused = false
        rate = 1
        audio = request.audioStreamIndex
        subtitle = request.subtitle
    }

    func play() {
        isPaused = false
        onEvent?(.paused(false))
    }

    func pause() {
        isPaused = true
        onEvent?(.paused(true))
    }

    func seek(toSeconds seconds: Double) async {
        seeks.append(seconds)
        positionSeconds = seconds
        onEvent?(.time(positionSeconds: seconds))
    }

    func setRate(_ rate: Double) {
        self.rate = min(max(rate, 0.25), maxRate)
    }

    func setVolume(_ volume: Double) {
        self.volume = volume
    }

    func setMuted(_ muted: Bool) {
        self.muted = muted
    }

    func selectAudio(streamIndex: Int) async {
        audio = streamIndex
        onEvent?(.audioTrack(streamIndex))
    }

    func selectSubtitle(_ subtitle: EngineSubtitle?) {
        self.subtitle = subtitle
        onEvent?(.subtitleTrack(subtitle))
    }

    func stop() {
        stops += 1
    }

    func setRemoteQueue(hasPrevious: Bool, hasNext: Bool) {
        remote = (hasPrevious, hasNext)
    }

    // MARK: Playing the engine's part

    /// The item opened.
    func open(duration: Double = 180, chapters: [EngineChapter] = []) {
        durationSeconds = duration
        onEvent?(.loaded(durationSeconds: duration, tracks: [], chapters: chapters))
        onEvent?(.audioTrack(audio))
        onEvent?(.subtitleTrack(subtitle))
        onEvent?(.paused(false))
        onEvent?(.time(positionSeconds: positionSeconds))
    }

    func time(_ seconds: Double) {
        positionSeconds = seconds
        onEvent?(.time(positionSeconds: seconds))
    }

    func end() {
        onEvent?(.ended)
    }

    func fail(_ message: String = "cannot open", _ kind: EngineFailureKind = .unsupported) {
        onEvent?(.failed(message: message, kind: kind))
    }
}
