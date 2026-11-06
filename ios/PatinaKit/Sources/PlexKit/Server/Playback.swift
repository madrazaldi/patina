// Port of desktop/plex/server/playback.ts.
// Timeline reporting and the universal transcoder (port of crates/plex/src/server/playback.rs).

import Foundation

public enum TimelineState: String, Sendable, Hashable, Codable {
    case playing, paused, buffering, stopped

    /// `TimelineState::as_str`
    public func asStr() -> String { rawValue }
}

/// A playback progress report. Plex uses these to track resume points,
/// mark items watched and show the session on the dashboard.
public struct TimelineUpdate: Sendable, Hashable {
    public var ratingKey: String
    public var key: String
    public var state: TimelineState
    public var timeMs: Int
    public var durationMs: Int
    public var playQueueItemId: Int?
    /// The session id used for transcoding, if any.
    public var session: String?

    public init(
        ratingKey: String, key: String, state: TimelineState, timeMs: Int, durationMs: Int,
        playQueueItemId: Int? = nil, session: String? = nil
    ) {
        self.ratingKey = ratingKey
        self.key = key
        self.state = state
        self.timeMs = timeMs
        self.durationMs = durationMs
        self.playQueueItemId = playQueueItemId
        self.session = session
    }
}

/// Options for the universal transcoder (defaults as in the Rust `Default`).
public struct TranscodeOptions: Sendable, Hashable {
    public var session: String
    public var mediaIndex: Int
    public var partIndex: Int
    /// Maximum video bitrate in kbps.
    public var maxVideoBitrate: Int?
    /// e.g. `1920x1080`.
    public var videoResolution: String?
    public var videoQuality: Int?
    public var directStream: Bool
    public var directStreamAudio: Bool
    /// `burn`, `sidecar`, `embedded`, `none` or `auto`.
    public var subtitles: String
    public var subtitleSize: Int?
    public var audioBoost: Int?
    public var offsetSecs: Double
    public var `protocol`: String

    public init(
        session: String = UUID().uuidString.lowercased(), mediaIndex: Int = 0, partIndex: Int = 0,
        maxVideoBitrate: Int? = nil, videoResolution: String? = nil, videoQuality: Int? = nil,
        directStream: Bool = true, directStreamAudio: Bool = true, subtitles: String = "auto",
        subtitleSize: Int? = nil, audioBoost: Int? = nil, offsetSecs: Double = 0, protocol: String = "hls"
    ) {
        self.session = session
        self.mediaIndex = mediaIndex
        self.partIndex = partIndex
        self.maxVideoBitrate = maxVideoBitrate
        self.videoResolution = videoResolution
        self.videoQuality = videoQuality
        self.directStream = directStream
        self.directStreamAudio = directStreamAudio
        self.subtitles = subtitles
        self.subtitleSize = subtitleSize
        self.audioBoost = audioBoost
        self.offsetSecs = offsetSecs
        self.protocol = `protocol`
    }
}

extension PlexServer {
    /// Report playback progress.
    public func timeline(_ u: TimelineUpdate) async throws {
        let p = Params()
            .set("ratingKey", u.ratingKey)
            .set("key", u.key)
            .set("state", u.state.asStr())
            .set("time", max(u.timeMs, 0))
            .set("duration", max(u.durationMs, 0))
            .set("identifier", "com.plexapp.plugins.library")
            .set("hasMDE", 1)
            .opt("playQueueItemID", u.playQueueItemId)
            .opt("X-Plex-Session-Identifier", u.session)
        try await empty("GET", "/:/timeline", p)
    }

    private func transcodeParams(_ path: String, _ o: TranscodeOptions) -> Params {
        let info = clientInfo
        return Params()
            .set("path", path)
            .set("mediaIndex", o.mediaIndex)
            .set("partIndex", o.partIndex)
            .set("protocol", o.protocol)
            .set("fastSeek", 1)
            .set("directPlay", 0)
            .set("directStream", o.directStream ? 1 : 0)
            .set("directStreamAudio", o.directStreamAudio ? 1 : 0)
            .set("subtitles", o.subtitles)
            .opt("subtitleSize", o.subtitleSize)
            .opt("audioBoost", o.audioBoost)
            .opt("maxVideoBitrate", o.maxVideoBitrate)
            .opt("videoBitrate", o.maxVideoBitrate)
            .opt("videoResolution", o.videoResolution)
            .opt("videoQuality", o.videoQuality)
            .set("offset", String(format: "%.3f", o.offsetSecs))
            .set("copyts", 1)
            .set("mediaBufferSize", 102400)
            .set("session", o.session)
            .set("X-Plex-Session-Identifier", o.session)
            .set("X-Plex-Client-Identifier", info.identifier)
            .set("X-Plex-Product", info.product)
            .set("X-Plex-Version", info.version)
            .set("X-Plex-Device", info.device)
            // The "Chrome" profile gives broadly compatible HLS (h264/hevc + aac/ac3).
            .set("X-Plex-Platform", "Chrome")
            .set(
                "X-Plex-Client-Profile-Extra",
                "add-transcode-target-audio-codec(type=videoProfile&context=streaming&protocol=hls&audioCodec=ac3,eac3)"
            )
    }

    /// Ask the server what it would do (direct play/stream/transcode) for these options.
    public func transcodeDecision(_ path: String, options: TranscodeOptions) async throws -> MediaContainer {
        try await get("/video/:/transcode/universal/decision", transcodeParams(path, options))
    }

    /// HLS playlist URL for a transcoded stream of `path` (`/library/metadata/<id>`).
    public func transcodeUrl(_ path: String, options: TranscodeOptions) throws -> URL {
        try authedUrl("/video/:/transcode/universal/start.m3u8", transcodeParams(path, options))
    }

    /// Transcoded audio stream URL (for music).
    public func audioTranscodeUrl(_ path: String, options: TranscodeOptions) throws -> URL {
        var p = transcodeParams(path, options)
        p.entries.removeAll { $0.key == "protocol" }
        p.push("protocol", "http")
        p.push("musicBitrate", options.maxVideoBitrate ?? 320)
        return try authedUrl("/music/:/transcode/universal/start.mp3", p)
    }

    /// Keep a transcode session alive while paused.
    public func pingTranscode(_ session: String) async throws {
        try await empty("GET", "/video/:/transcode/universal/ping", Params().set("session", session))
    }

    public func stopTranscode(_ session: String) async throws {
        try await empty("GET", "/video/:/transcode/universal/stop", Params().set("session", session))
    }

    /// Active transcoder sessions.
    public func transcodeSessions() async throws -> [TranscodeSession] {
        try await get("/transcode/sessions").transcodeSession
    }

    public func killTranscodeSession(_ key: String) async throws {
        try await delete("/transcode/sessions/\(key)")
    }
}
