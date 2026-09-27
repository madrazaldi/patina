import AppKit
import Foundation
import MediaPlayer
import Observation

/// OS media controls (Now Playing in Control Center, media keys, AirPods):
/// publishes `app.nowPlaying` to MPNowPlayingInfoCenter and routes remote
/// commands to mpv. Off when `settings.player.media_controls` is false. The
/// Rust side only drives Discord for this app, so this is the only publisher.
@MainActor
final class MediaRemote {
    static let shared = MediaRemote()

    private var started = false
    private var enabled = false
    /// Artwork for the current item, keyed by its image path.
    private var artwork: (key: String, art: MPMediaItemArtwork)?
    private var artworkLoading: String?
    /// The last key that failed to load, so position ticks don't refetch it.
    private var artworkFailed: String?

    private var app: AppModel { AppModel.shared }

    func start() {
        guard !started else { return }
        started = true
        observe()
    }

    /// Re-run `update` whenever anything it reads changes.
    private func observe() {
        withObservationTracking {
            update()
        } onChange: {
            Task { @MainActor in MediaRemote.shared.observe() }
        }
    }

    private func update() {
        let wanted = app.settings?.player.media_controls ?? true
        let np = app.nowPlaying
        if wanted != enabled {
            enabled = wanted
            wanted ? register() : unregister()
        }
        guard enabled, let np else {
            clear()
            return
        }
        publish(np)
    }

    private func clear() {
        let center = MPNowPlayingInfoCenter.default()
        if center.nowPlayingInfo != nil { center.nowPlayingInfo = nil }
        center.playbackState = enabled ? .stopped : .unknown
        artwork = nil
        artworkLoading = nil
    }

    private func publish(_ np: NowPlaying) {
        let item = np.item
        var info: [String: Any] = [:]
        switch item.kind {
        case "episode":
            info[MPMediaItemPropertyTitle] = item.name
            info[MPMediaItemPropertyArtist] = item.grandparentTitle ?? ""
            info[MPMediaItemPropertyAlbumTitle] = item.episodeCode
            info[MPNowPlayingInfoPropertyMediaType] = MPNowPlayingInfoMediaType.video.rawValue
        case "track":
            info[MPMediaItemPropertyTitle] = item.name
            info[MPMediaItemPropertyArtist] = item.originalTitle ?? item.grandparentTitle ?? ""
            info[MPMediaItemPropertyAlbumTitle] = item.parentTitle ?? ""
            info[MPNowPlayingInfoPropertyMediaType] = MPNowPlayingInfoMediaType.audio.rawValue
        default:
            info[MPMediaItemPropertyTitle] = item.name
            if let y = item.year { info[MPMediaItemPropertyArtist] = String(y) }
            info[MPNowPlayingInfoPropertyMediaType] = MPNowPlayingInfoMediaType.video.rawValue
        }
        info[MPNowPlayingInfoPropertyIsLiveStream] = np.live
        if !np.live && np.durationMs > 0 {
            info[MPMediaItemPropertyPlaybackDuration] = Double(np.durationMs) / 1000
        }
        info[MPNowPlayingInfoPropertyElapsedPlaybackTime] = Double(np.positionMs) / 1000
        info[MPNowPlayingInfoPropertyPlaybackRate] = np.paused ? 0.0 : 1.0
        info[MPNowPlayingInfoPropertyDefaultPlaybackRate] = 1.0
        if np.queueLen > 0 {
            info[MPNowPlayingInfoPropertyPlaybackQueueIndex] = np.queuePos
            info[MPNowPlayingInfoPropertyPlaybackQueueCount] = np.queueLen
        }

        let path = np.local ? nil : (item.kind == "episode" ? item.thumb ?? item.posterPath : item.posterPath)
        let key = np.local ? "download:\(item.ratingKey ?? "")" : (path ?? "")
        if let artwork, artwork.key == key {
            info[MPMediaItemPropertyArtwork] = artwork.art
        } else if !key.isEmpty, artworkLoading != key, artworkFailed != key {
            loadArtwork(key: key, path: path, np: np)
        }

        let center = MPNowPlayingInfoCenter.default()
        center.nowPlayingInfo = info
        center.playbackState = np.paused ? .paused : .playing

        let commands = MPRemoteCommandCenter.shared()
        commands.nextTrackCommand.isEnabled = np.queuePos + 1 < np.queueLen
        commands.previousTrackCommand.isEnabled = true
        commands.changePlaybackPositionCommand.isEnabled = !np.live && np.durationMs > 0
        commands.skipForwardCommand.isEnabled = !np.live
        commands.skipBackwardCommand.isEnabled = !np.live
    }

    private func loadArtwork(key: String, path: String?, np: NowPlaying) {
        artworkLoading = key
        let downloadId = np.local
            ? app.downloads.first { $0.item.ratingKey == np.item.ratingKey && $0.poster != nil }?.id
            : nil
        Task {
            let data: Data?
            if let path { data = await Core.image(path, width: 600, height: 600) }
            else if let downloadId { data = await Core.downloadImage(downloadId) }
            else { data = nil }
            guard artworkLoading == key else { return }
            artworkLoading = nil
            guard let data, let image = NSImage(data: data) else { artworkFailed = key; return }
            let art = MPMediaItemArtwork(boundsSize: image.size) { _ in image }
            artwork = (key, art)
            // Re-publish with the artwork.
            if enabled, let current = app.nowPlaying { publish(current) }
        }
    }

    // MARK: Commands

    private func register() {
        let c = MPRemoteCommandCenter.shared()
        let send = { (control: Control) -> MPRemoteCommandHandlerStatus in
            Task { @MainActor in
                do { try await API.control(control) } catch { AppModel.shared.error(error) }
            }
            return .success
        }
        let noItem = { MPRemoteCommandHandlerStatus.noActionableNowPlayingItem }
        c.playCommand.addTarget { _ in AppModel.shared.nowPlaying == nil ? noItem() : send(.play) }
        c.pauseCommand.addTarget { _ in AppModel.shared.nowPlaying == nil ? noItem() : send(.pause) }
        c.togglePlayPauseCommand.addTarget { _ in AppModel.shared.nowPlaying == nil ? noItem() : send(.togglePause) }
        c.nextTrackCommand.addTarget { _ in send(.next) }
        c.previousTrackCommand.addTarget { _ in send(.previous) }
        c.stopCommand.addTarget { _ in send(.stop) }
        c.skipForwardCommand.preferredIntervals = [10]
        c.skipBackwardCommand.preferredIntervals = [10]
        c.skipForwardCommand.addTarget { e in
            send(.seekRelative((e as? MPSkipIntervalCommandEvent)?.interval ?? 10))
        }
        c.skipBackwardCommand.addTarget { e in
            send(.seekRelative(-((e as? MPSkipIntervalCommandEvent)?.interval ?? 10)))
        }
        c.changePlaybackPositionCommand.addTarget { e in
            guard let e = e as? MPChangePlaybackPositionCommandEvent else { return .commandFailed }
            return send(.seekTo(e.positionTime))
        }
        for cmd in allCommands(c) { cmd.isEnabled = true }
    }

    private func unregister() {
        let c = MPRemoteCommandCenter.shared()
        for cmd in allCommands(c) {
            cmd.removeTarget(nil)
            cmd.isEnabled = false
        }
        clear()
    }

    private func allCommands(_ c: MPRemoteCommandCenter) -> [MPRemoteCommand] {
        [c.playCommand, c.pauseCommand, c.togglePlayPauseCommand, c.nextTrackCommand, c.previousTrackCommand,
         c.stopCommand, c.skipForwardCommand, c.skipBackwardCommand, c.changePlaybackPositionCommand]
    }
}
