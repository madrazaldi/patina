// How AetherEngine's values read as the core's `PlaybackEngine` values (no desktop twin: mpv
// reported its own property names). Pure functions, so the adapter stays about wiring.

import AetherEngine
import CoreGraphics
import Foundation
import PatinaCore

enum EngineMapping {
    /// AetherEngine names a track by `TrackInfo.id`: the container stream index, or
    /// `externalSubtitleTrackIDBase + i` for `LoadOptions.externalSubtitles[i]`. The core's ids are the same.
    static func subtitleTrackID(_ subtitle: EngineSubtitle) -> Int {
        switch subtitle {
        case .stream(let index): index
        case .external(let index): AetherEngine.externalSubtitleTrackIDBase + index
        }
    }

    static func subtitle(trackID: Int?) -> EngineSubtitle? {
        guard let trackID else { return nil }
        let base = AetherEngine.externalSubtitleTrackIDBase
        return trackID >= base ? .external(trackID - base) : .stream(trackID)
    }

    static func externalTrack(_ sub: ExternalSubtitle, headers: [String: String]) -> ExternalSubtitleTrack {
        ExternalSubtitleTrack(
            url: sub.url, name: sub.title, language: sub.language, isForced: sub.forced,
            isHearingImpaired: sub.hearingImpaired, httpHeaders: headers.isEmpty ? nil : headers,
            formatHint: sub.format
        )
    }

    static func track(_ info: TrackInfo, kind: EngineTrack.Kind) -> EngineTrack {
        EngineTrack(
            id: info.id, kind: kind, title: info.name, language: info.language,
            codec: info.codec.isEmpty ? nil : info.codec, channels: info.channels > 0 ? info.channels : nil,
            isDefault: info.isDefault, isForced: info.isForced, isExternal: info.isExternal
        )
    }

    static func chapter(_ info: ChapterInfo) -> EngineChapter {
        EngineChapter(title: info.name, startSeconds: info.startSeconds)
    }

    /// What the failure actions can do about it: a network failure is retried when the server is
    /// back, an unsupported one offers another version or a transcode.
    static func failureKind(_ info: PlaybackErrorInfo?) -> EngineFailureKind {
        guard let info else { return .other }
        if info.underlyingDomain == NSURLErrorDomain, info.kind != .sourceCertificateRejected { return .network }
        switch info.kind {
        case .liveSourceUnavailable, .vodSourceFailed, .sourceRateLimited, .liveReloadNeverReady, .hlsIngestFailed:
            return .network
        case .sourceRefused:
            // A 5xx is the server struggling; a 4xx is a verdict on the request.
            return (info.underlyingCode ?? 0) >= 500 ? .network : .other
        case .sourceOpenFailed, .customSourceProbeFailed, .hlsFormatUnsupported, .hlsPlaylistOnRawLivePath,
             .dolbyVisionRequiresHardware, .demuxedAudioLiveUnsupported, .nativeItemFailed,
             .noPlayableTrackWithinBudget, .masterPlaylistRejected, .softwarePipelineFailed,
             .audioBridgeProducedNoOutput:
            return .unsupported
        default:
            return .other
        }
    }

    static func failureKind(of error: any Error) -> EngineFailureKind {
        let ns = error as NSError
        return ns.domain == NSURLErrorDomain ? .network : .unsupported
    }

    static func route(_ route: VideoRoute) -> String? {
        switch route {
        case .none: nil
        case .remoteBypass: "Direct (AVPlayer)"
        case .loopback: "Remux (AVPlayer)"
        case .software: "Software"
        case .audio: "Audio"
        }
    }

    static func format(_ format: VideoFormat) -> String {
        switch format {
        case .sdr: "SDR"
        case .hdr10: "HDR10"
        case .hdr10Plus: "HDR10+"
        case .dolbyVision: "Dolby Vision"
        case .hlg: "HLG"
        }
    }

    /// The presented size: the coded size under the pixel aspect ratio.
    static func displaySize(width: Int32, height: Int32, pixelAspect: Double, software: CGSize?) -> CGSize? {
        if let software, software.width > 0, software.height > 0 { return software }
        guard width > 0, height > 0 else { return nil }
        return CGSize(width: Double(width) * max(pixelAspect, 0.01), height: Double(height))
    }

    /// The cues to draw at `time`, in the order they arrived.
    static func activeCues(_ cues: [SubtitleCue], at time: Double) -> [SubtitleCue] {
        cues.filter { $0.startTime <= time && time < $0.endTime }
    }
}

extension AetherPlaybackEngine {
    /// The id in `subtitleTracks` of a subtitle choice, for a screen's picker.
    public static func trackID(of subtitle: EngineSubtitle) -> Int {
        EngineMapping.subtitleTrackID(subtitle)
    }

    /// The subtitle choice of an id in `subtitleTracks`; nil is off.
    public static func subtitle(trackID: Int?) -> EngineSubtitle? {
        EngineMapping.subtitle(trackID: trackID)
    }
}
