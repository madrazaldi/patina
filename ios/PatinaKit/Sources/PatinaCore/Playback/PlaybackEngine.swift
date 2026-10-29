// What the core asks of a video engine: the iOS stand-in for `desktop/mpv`.
//
// The desktop player drives an mpv process over JSON IPC and lets mpv own the
// playlist, the OSD and the keys. On iOS the app owns all of that: the core's
// player keeps the play queue and loads one item at a time, and the screen draws
// the controls. `PatinaPlayer` implements this protocol over AetherEngine; the
// core tests use a fake. Times are seconds; stream indexes are the container
// stream indexes Plex reports as `Stream.index` (AetherEngine's `TrackInfo.id`).

import Foundation

/// One item to play.
nonisolated public struct EngineLoad: Sendable, Equatable {
    public var url: URL
    /// Where to start; 0 plays from the beginning.
    public var startSeconds: Double
    /// Sent with every request for the media and its sidecars (`X-Plex-Token` and the client headers).
    public var httpHeaders: [String: String]
    /// Container stream index of the audio track to play; nil lets the engine choose.
    public var audioStreamIndex: Int?
    /// The subtitle to show from the first frame; nil shows none.
    public var subtitle: EngineSubtitle?
    /// Subtitle files that are not in the container (Plex's external streams).
    public var externalSubtitles: [ExternalSubtitle]
    /// Music: no video pipeline.
    public var audioOnly: Bool
    /// A live channel: no duration, seeking limited to what is buffered.
    public var live: Bool
    /// What the lock screen and Control Center show.
    public var nowPlaying: NowPlayingCard

    public init(
        url: URL, startSeconds: Double = 0, httpHeaders: [String: String] = [:], audioStreamIndex: Int? = nil,
        subtitle: EngineSubtitle? = nil, externalSubtitles: [ExternalSubtitle] = [], audioOnly: Bool = false,
        live: Bool = false, nowPlaying: NowPlayingCard = NowPlayingCard(title: "")
    ) {
        self.url = url
        self.startSeconds = startSeconds
        self.httpHeaders = httpHeaders
        self.audioStreamIndex = audioStreamIndex
        self.subtitle = subtitle
        self.externalSubtitles = externalSubtitles
        self.audioOnly = audioOnly
        self.live = live
        self.nowPlaying = nowPlaying
    }
}

/// A subtitle choice.
nonisolated public enum EngineSubtitle: Sendable, Hashable {
    /// An embedded stream, by container stream index.
    case stream(Int)
    /// `EngineLoad.externalSubtitles[i]`.
    case external(Int)
}

nonisolated public struct ExternalSubtitle: Sendable, Equatable {
    public var url: URL
    public var title: String
    /// ISO 639 code, when Plex knows it.
    public var language: String?
    public var forced: Bool
    public var hearingImpaired: Bool
    /// `srt`, `ass`, `vtt`, ... when the URL does not say.
    public var format: String?

    public init(url: URL, title: String, language: String? = nil, forced: Bool = false, hearingImpaired: Bool = false, format: String? = nil) {
        self.url = url
        self.title = title
        self.language = language
        self.forced = forced
        self.hearingImpaired = hearingImpaired
        self.format = format
    }
}

/// Lock screen and Control Center metadata.
nonisolated public struct NowPlayingCard: Sendable, Equatable {
    public var title: String
    /// Show and episode, or artist and album.
    public var subtitle: String?
    /// Full URL of the artwork, already carrying whatever it needs to load.
    public var artwork: URL?
    public var durationSeconds: Double?

    public init(title: String, subtitle: String? = nil, artwork: URL? = nil, durationSeconds: Double? = nil) {
        self.title = title
        self.subtitle = subtitle
        self.artwork = artwork
        self.durationSeconds = durationSeconds
    }
}

/// A track as the engine found it in the file.
nonisolated public struct EngineTrack: Sendable, Equatable, Identifiable {
    public enum Kind: Sendable { case audio, subtitle }
    /// Container stream index, or `externalSubtitleBase + i` for `externalSubtitles[i]`.
    public var id: Int
    public var kind: Kind
    public var title: String
    public var language: String?
    public var codec: String?
    public var channels: Int?
    public var isDefault: Bool
    public var isForced: Bool
    public var isExternal: Bool

    public static let externalSubtitleBase = 100_000

    public init(id: Int, kind: Kind, title: String, language: String? = nil, codec: String? = nil, channels: Int? = nil, isDefault: Bool = false, isForced: Bool = false, isExternal: Bool = false) {
        self.id = id
        self.kind = kind
        self.title = title
        self.language = language
        self.codec = codec
        self.channels = channels
        self.isDefault = isDefault
        self.isForced = isForced
        self.isExternal = isExternal
    }
}

nonisolated public struct EngineChapter: Sendable, Equatable {
    public var title: String
    public var startSeconds: Double

    public init(title: String, startSeconds: Double) {
        self.title = title
        self.startSeconds = startSeconds
    }
}

/// Why playback failed, for the failure actions (see `failure.ts`).
nonisolated public enum EngineFailureKind: Sendable, Equatable {
    /// The connection dropped or the server stopped answering: retry when it is back.
    case network
    /// The format, codec or file cannot be played: offer another version or a transcode.
    case unsupported
    case other
}

/// What the screen's diagnostics panel shows (desktop: `controls.ts` `Diagnostics`).
nonisolated public struct EngineDiagnostics: Sendable, Equatable {
    public var buffering: Bool
    /// Seconds buffered ahead of the playhead.
    public var bufferedSeconds: Double?
    /// "VideoToolbox", "dav1d", ...
    public var decoder: String?
    /// "SDR", "HDR10", "Dolby Vision", ...
    public var videoFormat: String?
    /// "Direct (AVPlayer)", "Remux", "Software", ...
    public var route: String?
    public var droppedFrames: Int?
    public var videoSize: String?

    public init(buffering: Bool = false, bufferedSeconds: Double? = nil, decoder: String? = nil, videoFormat: String? = nil, route: String? = nil, droppedFrames: Int? = nil, videoSize: String? = nil) {
        self.buffering = buffering
        self.bufferedSeconds = bufferedSeconds
        self.decoder = decoder
        self.videoFormat = videoFormat
        self.route = route
        self.droppedFrames = droppedFrames
        self.videoSize = videoSize
    }
}

/// What the engine reports, in order, for the item it is playing.
nonisolated public enum EngineEvent: Sendable, Equatable {
    /// The item opened: what it holds.
    case loaded(durationSeconds: Double?, tracks: [EngineTrack], chapters: [EngineChapter])
    /// About 4 times a second while the clock moves, and after every seek.
    case time(positionSeconds: Double)
    case paused(Bool)
    case diagnostics(EngineDiagnostics)
    /// The audio or subtitle track changed (by the user in the screen, or by the engine).
    case audioTrack(Int?)
    case subtitleTrack(EngineSubtitle?)
    /// The item played to its end.
    case ended
    /// The item could not be played, or stopped playing.
    case failed(message: String, kind: EngineFailureKind)
}

/// A lock screen, Control Center or headphone command, which the core handles like the
/// equivalent `control` command.
nonisolated public enum RemoteCommand: Sendable, Equatable {
    case play, pause, togglePause, next, previous, stop
    case seek(toSeconds: Double)
    case skip(bySeconds: Double)
}

/// A video engine. One instance lives for the app's lifetime; `load` replaces
/// whatever was playing. Every call is made on the main actor.
public protocol PlaybackEngine: AnyObject {
    /// Set by the core before the first load; receives every `EngineEvent`.
    var onEvent: ((EngineEvent) -> Void)? { get set }
    /// Set by the core; receives the lock screen and headphone commands.
    var onRemoteCommand: ((RemoteCommand) -> Void)? { get set }

    /// Open and start playing an item. Returns once it is playing or failed (a failure
    /// is also reported as `.failed`). Throws `CancellationError` when replaced.
    func load(_ request: EngineLoad) async throws
    func play()
    func pause()
    func seek(toSeconds: Double) async
    /// 0.25 to 4; the engine may clamp (AVPlayer plays video up to 2x).
    func setRate(_ rate: Double)
    /// 0 to 1.
    func setVolume(_ volume: Double)
    func setMuted(_ muted: Bool)
    /// Switch the audio track; the position is kept.
    func selectAudio(streamIndex: Int) async
    /// Show a subtitle, or none.
    func selectSubtitle(_ subtitle: EngineSubtitle?)
    /// Stop and release the item. The engine is idle afterwards.
    func stop()

    /// The playhead now, in seconds.
    var positionSeconds: Double { get }
    var durationSeconds: Double? { get }
    var isPaused: Bool { get }
    /// The largest rate the current item supports.
    var maxRate: Double { get }

    /// Which of the lock screen's next and previous buttons work (the queue is the core's).
    func setRemoteQueue(hasPrevious: Bool, hasNext: Bool)
}

extension PlaybackEngine {
    public func setRemoteQueue(hasPrevious: Bool, hasNext: Bool) {}
}
