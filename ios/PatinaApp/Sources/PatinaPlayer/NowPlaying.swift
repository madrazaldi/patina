// The lock screen and Control Center card, and the remote commands behind it (desktop: mpv's
// MPRIS / media keys, which mpv handled itself; here the core handles them).
//
// AetherEngine publishes Now Playing through an `MPNowPlayingSession` on the AVPlayer paths (the
// native video path once `ownsVideoNowPlayingSession` is set, and the audio-only path always).
// While such a session is active the system reads that session's info and sends commands to its
// command center, so the card's identity is staged through the engine and the commands are
// registered there. The software path has no AVPlayer and no session: there the card and the
// commands go through the shared centers, with the elapsed time written by hand.

import AetherEngine
import MediaPlayer
import PatinaCore
import UIKit

final class NowPlayingController {
    private enum Target: Equatable {
        case videoSession(MPNowPlayingSession)
        case audioSession(MPNowPlayingSession)
        case shared

        var commandCenter: MPRemoteCommandCenter {
            switch self {
            case .videoSession(let session), .audioSession(let session): session.remoteCommandCenter
            case .shared: .shared()
            }
        }
    }

    private unowned let engine: AetherEngine
    var onCommand: ((RemoteCommand) -> Void)?
    var skipSeconds: (backward: Double, forward: Double) = (10, 10) {
        didSet { if let target { registerCommands(on: target) } }
    }
    var hasPrevious = true { didSet { commandsEnabled() } }
    var hasNext = true { didSet { commandsEnabled() } }

    private var card: NowPlayingCard?
    private var audioOnly = false
    private var live = false
    private var artwork: MPMediaItemArtwork?
    private var artworkURL: URL?
    private var artworkTask: Task<Void, Never>?
    private var target: Target?
    private var registered: [(MPRemoteCommand, Any)] = []
    private var commandCenter: MPRemoteCommandCenter?

    init(engine: AetherEngine) {
        self.engine = engine
        // Read when the engine builds its native host, so before the first load.
        engine.ownsVideoNowPlayingSession = true
    }

    /// A new item: its card replaces the previous one.
    func begin(_ card: NowPlayingCard, audioOnly: Bool, live: Bool) {
        self.card = card
        self.audioOnly = audioOnly
        self.live = live
        if card.artwork != artworkURL {
            artworkURL = card.artwork
            artwork = nil
            loadArtwork(card.artwork)
        }
        refresh()
    }

    /// The engine changed hosts (a load, a reroute, a reload): follow its session.
    func refresh() {
        guard card != nil else { return }
        let next: Target =
            if let session = engine.audioNowPlayingSession { .audioSession(session) }
            else if let session = engine.videoNowPlayingSession { .videoSession(session) }
            else { .shared }
        if next != target {
            if target == .shared { clearShared() }
            registerCommands(on: next)
            target = next
        }
        publish()
    }

    /// Elapsed time and rate, for the shared center only: a session reads them off its player.
    func playbackChanged() {
        guard target == .shared else { return }
        publish()
    }

    func end() {
        artworkTask?.cancel()
        unregisterCommands()
        if target == .shared { clearShared() }
        engine.setVideoNowPlayingInfo([:])
        engine.setAudioNowPlayingInfo([:])
        target = nil
        card = nil
    }

    // MARK: - Info

    private func identity() -> [String: Any] {
        guard let card else { return [:] }
        var info: [String: Any] = [
            MPMediaItemPropertyTitle: card.title,
            MPNowPlayingInfoPropertyMediaType: NSNumber(value: (audioOnly ? MPNowPlayingInfoMediaType.audio : .video).rawValue),
            MPNowPlayingInfoPropertyIsLiveStream: NSNumber(value: live),
        ]
        if let subtitle = card.subtitle { info[MPMediaItemPropertyArtist] = subtitle }
        if let artwork { info[MPMediaItemPropertyArtwork] = artwork }
        return info
    }

    private func publish() {
        guard let target else { return }
        let identity = identity()
        switch target {
        case .videoSession:
            engine.setVideoNowPlayingInfo(identity)
        case .audioSession:
            engine.setAudioNowPlayingInfo(identity)
        case .shared:
            var info = identity
            let paused = engine.state != .playing
            let duration = engine.duration > 0 ? engine.duration : card?.durationSeconds
            if let duration, !live { info[MPMediaItemPropertyPlaybackDuration] = NSNumber(value: duration) }
            info[MPNowPlayingInfoPropertyElapsedPlaybackTime] = NSNumber(value: engine.currentTime)
            info[MPNowPlayingInfoPropertyPlaybackRate] = NSNumber(value: paused ? 0 : 1)
            let center = MPNowPlayingInfoCenter.default()
            center.nowPlayingInfo = info
            center.playbackState = paused ? .paused : .playing
        }
    }

    private func clearShared() {
        let center = MPNowPlayingInfoCenter.default()
        center.nowPlayingInfo = nil
        center.playbackState = .stopped
    }

    private func loadArtwork(_ url: URL?) {
        artworkTask?.cancel()
        guard let url else { return }
        artworkTask = Task { [weak self] in
            guard let image = await Self.fetchImage(url), !Task.isCancelled, let self, self.artworkURL == url else { return }
            // The system may ask for the bitmap off the main thread, so the handler must be Sendable
            // and hand back an image that is already decoded.
            self.artwork = MPMediaItemArtwork(boundsSize: image.size) { @Sendable _ in image }
            self.publish()
        }
    }

    @concurrent nonisolated private static func fetchImage(_ url: URL) async -> UIImage? {
        guard let (data, response) = try? await URLSession.shared.data(from: url),
              (response as? HTTPURLResponse).map({ (200..<300).contains($0.statusCode) }) ?? true
        else { return nil }
        return await UIImage(data: data)?.byPreparingForDisplay()
    }

    // MARK: - Commands

    private func registerCommands(on target: Target) {
        unregisterCommands()
        let center = target.commandCenter
        commandCenter = center
        let send: @Sendable (RemoteCommand) -> Void = { [weak self] command in
            Task { @MainActor in self?.onCommand?(command) }
        }
        func add(_ command: MPRemoteCommand, _ map: @escaping @Sendable (MPRemoteCommandEvent) -> RemoteCommand?) {
            command.isEnabled = true
            let token = command.addTarget { @Sendable event in
                guard let mapped = map(event) else { return .commandFailed }
                send(mapped)
                return .success
            }
            registered.append((command, token))
        }
        add(center.playCommand) { _ in .play }
        add(center.pauseCommand) { _ in .pause }
        add(center.togglePlayPauseCommand) { _ in .togglePause }
        add(center.stopCommand) { _ in .stop }
        add(center.nextTrackCommand) { _ in .next }
        add(center.previousTrackCommand) { _ in .previous }
        add(center.changePlaybackPositionCommand) { event in
            (event as? MPChangePlaybackPositionCommandEvent).map { .seek(toSeconds: $0.positionTime) }
        }
        center.skipBackwardCommand.preferredIntervals = [NSNumber(value: skipSeconds.backward)]
        center.skipForwardCommand.preferredIntervals = [NSNumber(value: skipSeconds.forward)]
        add(center.skipBackwardCommand) { event in
            (event as? MPSkipIntervalCommandEvent).map { .skip(bySeconds: -$0.interval) }
        }
        add(center.skipForwardCommand) { event in
            (event as? MPSkipIntervalCommandEvent).map { .skip(bySeconds: $0.interval) }
        }
        commandsEnabled()
    }

    private func commandsEnabled() {
        guard let center = commandCenter else { return }
        center.nextTrackCommand.isEnabled = hasNext
        center.previousTrackCommand.isEnabled = hasPrevious
        center.changePlaybackPositionCommand.isEnabled = !live
    }

    private func unregisterCommands() {
        for (command, token) in registered {
            command.removeTarget(token)
            command.isEnabled = false
        }
        registered = []
        commandCenter = nil
    }
}
