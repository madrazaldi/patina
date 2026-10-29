// The plain values and closures the player's chrome and gestures are driven by (no desktop twin:
// mpv drew its own OSD). They know nothing of the core, so any screen can feed them: the core's
// player, the lab, a preview.

import CoreGraphics
import SwiftUI

/// How the picture fills the screen.
nonisolated public enum PlayerVideoFill: String, CaseIterable, Sendable {
    /// The whole picture, letterboxed.
    case fit
    /// Covers the screen, cropping the edges.
    case fill
    /// The fitted picture enlarged by a third, which crops black bars burnt into the picture itself.
    case zoom

    public var label: String {
        switch self {
        case .fit: "Fit"
        case .fill: "Fill"
        case .zoom: "Zoom"
        }
    }

    public var systemImage: String {
        switch self {
        case .fit: "arrow.down.right.and.arrow.up.left"
        case .fill: "arrow.up.left.and.arrow.down.right"
        case .zoom: "plus.magnifyingglass"
        }
    }

    static let zoomScale: CGFloat = 4 / 3
}

/// A stretch of the item the scrub bar marks, and the scrubber snaps to the edges of.
nonisolated public struct PlayerMarkerRange: Equatable, Sendable, Identifiable {
    public enum Kind: Sendable { case intro, credits, other }
    public var kind: Kind
    public var start: Double
    public var end: Double
    public var id: String { "\(kind)-\(start)" }

    public init(kind: Kind, start: Double, end: Double) {
        self.kind = kind
        self.start = start
        self.end = end
    }
}

/// What the controls show. Rebuilt by the screen from whatever it plays.
public struct PlayerTransportState: Equatable {
    public var position: Double
    /// nil while unknown and for live channels.
    public var duration: Double?
    /// How far the media is buffered, on the `position` axis.
    public var buffered: Double?
    public var isPaused: Bool
    /// Loading, seeking or buffering: the play button shows a spinner.
    public var isWaiting: Bool
    public var isLive: Bool
    public var canSeek: Bool
    public var hasPrevious: Bool
    public var hasNext: Bool
    /// Chapter starts, in seconds.
    public var chapters: [Double]
    public var markers: [PlayerMarkerRange]
    /// Whether `PlayerTransportActions.thumbnail` can answer.
    public var supportsThumbnails: Bool

    public init(
        position: Double = 0, duration: Double? = nil, buffered: Double? = nil, isPaused: Bool = true,
        isWaiting: Bool = false, isLive: Bool = false, canSeek: Bool = true, hasPrevious: Bool = false,
        hasNext: Bool = false, chapters: [Double] = [], markers: [PlayerMarkerRange] = [], supportsThumbnails: Bool = false
    ) {
        self.position = position
        self.duration = duration
        self.buffered = buffered
        self.isPaused = isPaused
        self.isWaiting = isWaiting
        self.isLive = isLive
        self.canSeek = canSeek
        self.hasPrevious = hasPrevious
        self.hasNext = hasNext
        self.chapters = chapters
        self.markers = markers
        self.supportsThumbnails = supportsThumbnails
    }

    /// Whether the scrubber can work: something seekable with a length.
    var isScrubbable: Bool { canSeek && !isLive && (duration ?? 0) > 0 }

    /// Where the scrubber snaps: chapter starts and marker edges.
    var snapPoints: [Double] {
        (chapters + markers.flatMap { [$0.start, $0.end] }).filter { $0 > 0 }.sorted()
    }
}

/// What the controls do. Optional actions hide their buttons.
public struct PlayerTransportActions {
    public var togglePause: () -> Void
    /// Seek to an absolute position.
    public var seek: (Double) -> Void
    public var previous: (() -> Void)?
    public var next: (() -> Void)?
    public var close: () -> Void
    /// Hold-for-speed: true while the finger is down (play at 2x), false on release (back to the speed before).
    public var speedBoost: ((Bool) -> Void)?
    /// A still at a position, for the scrub bubble.
    public var thumbnail: ((Double) async -> CGImage?)?
    /// A cheap seek while dragging, used when there are no thumbnails.
    public var scrubPreview: ((Double) -> Void)?

    public init(
        togglePause: @escaping () -> Void, seek: @escaping (Double) -> Void, previous: (() -> Void)? = nil,
        next: (() -> Void)? = nil, close: @escaping () -> Void, speedBoost: ((Bool) -> Void)? = nil,
        thumbnail: ((Double) async -> CGImage?)? = nil, scrubPreview: ((Double) -> Void)? = nil
    ) {
        self.togglePause = togglePause
        self.seek = seek
        self.previous = previous
        self.next = next
        self.close = close
        self.speedBoost = speedBoost
        self.thumbnail = thumbnail
        self.scrubPreview = scrubPreview
    }

    /// Skip relative to `position`, inside the item.
    func skip(_ seconds: Double, in state: PlayerTransportState) {
        guard state.canSeek else { return }
        var target = max(0, state.position + seconds)
        if let duration = state.duration { target = min(target, max(0, duration - 1)) }
        seek(target)
    }
}

extension PlayerTransportState {
    /// The state of an `AetherPlaybackEngine`, for a screen that plays through one directly.
    public init(player: AetherPlaybackEngine, hasPrevious: Bool = false, hasNext: Bool = false, markers: [PlayerMarkerRange] = []) {
        self.init(
            position: player.position, duration: player.duration, buffered: player.bufferedPosition,
            isPaused: player.isPaused, isWaiting: player.phase.isWaiting, isLive: player.isLive,
            canSeek: player.hasItem && (player.duration ?? 0) > 0, hasPrevious: hasPrevious, hasNext: hasNext,
            chapters: player.chapters.map(\.startSeconds), markers: markers,
            supportsThumbnails: player.supportsScrubThumbnails
        )
    }
}

/// `12:03`, `1:02:03`.
nonisolated enum PlayerTime {
    static func format(_ seconds: Double) -> String {
        let total = Int(max(0, seconds.isFinite ? seconds : 0).rounded(.down))
        let h = total / 3600, m = total / 60 % 60, s = total % 60
        return h > 0 ? String(format: "%d:%02d:%02d", h, m, s) : String(format: "%d:%02d", m, s)
    }

    /// `+1:24`, `−0:30`.
    static func delta(_ seconds: Double) -> String {
        (seconds < 0 ? "−" : "+") + format(abs(seconds))
    }

    /// "12 minutes, 3 seconds", for VoiceOver.
    static func spoken(_ seconds: Double) -> String {
        Duration.seconds(max(0, seconds.rounded())).formatted(.units(allowed: [.hours, .minutes, .seconds], width: .wide))
    }
}
