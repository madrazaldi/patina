// STUB: the playback agent replaces this file with the port of desktop/core/failure.ts.
// Only what the foundation calls, and the `PlaybackFailure` type (all its fields) for
// `CoreEvent.playbackFailure`.

import Foundation
import PlexKit

public nonisolated struct PlaybackFailure: Sendable, Equatable {
    /// Distinguishes this failure from earlier ones.
    public var id: Int
    public var item: Metadata
    /// A sentence for people.
    public var reason: String
    /// What the player (or the underlying error) said.
    public var detail: String?
    public var kind: Failure.Kind
    public var phase: Failure.Phase
    /// Where trying again resumes.
    public var positionMs: Int
    /// The version (media) that failed.
    public var mediaIndex: Int
    /// The quality preset that was in use.
    public var quality: String
    /// The next step down, when transcoding more aggressively is an option.
    public var lowerQuality: Failure.QualityChoice?
    public var canRetry: Bool
    /// Items after the failed one that stay queued.
    public var remaining: Int
}

public enum Failure {
    public nonisolated enum Kind: String, Sendable, Equatable {
        /// The stream could not be loaded or was cut off: the connection or the server.
        case network
        /// The player could not make sense of the file.
        case format
        /// The player could not open an audio or video device.
        case output
        case other
    }

    /// Whether playback never got going or broke after it had.
    public nonisolated enum Phase: String, Sendable, Equatable {
        case start, playing
    }

    public nonisolated struct QualityChoice: Sendable, Equatable {
        public var id: String
        public var label: String
    }

    /// The account, profile or server changed: a failure from another one can't be acted on here.
    public static func sessionChanged(_ app: AppState, _ session: Session?) {}

    /// The server answered again after a network failure: try once more, on our own.
    public static func onReconnect(_ app: AppState) {}
}
