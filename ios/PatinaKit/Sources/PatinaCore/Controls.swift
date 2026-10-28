// STUB: the playback agent replaces this file with the port of desktop/core/controls.ts.
// Only the `PlaybackState` type (all its fields) for `CoreEvent.playbackState`.

import Foundation

public nonisolated struct PlaybackState: Sendable, Equatable {
    /// Percent.
    public var volume: Double
    public var muted: Bool
    public var speed: Double
    public var `repeat`: Controls.Repeat
    public var sleep: Controls.SleepState?
    public var chapters: [Controls.ChapterView]
    /// Index into `chapters` of the playing chapter.
    public var chapter: Int?
    public var diagnostics: Controls.Diagnostics
}

public enum Controls {
    public nonisolated enum Repeat: String, Sendable, Equatable {
        case off, one, all
    }

    public nonisolated struct ChapterView: Sendable, Equatable {
        public var title: String
        public var timeMs: Int
    }

    /// How the server is reached.
    public nonisolated struct ConnectionInfo: Sendable, Equatable {
        /// `local`, `remote`, `relay` or `offline` (a downloaded file).
        public var kind: String
        public var address: String?
    }

    public nonisolated struct Diagnostics: Sendable, Equatable {
        /// Playback is stalled waiting for data.
        public var buffering: Bool
        /// How full the buffer is while stalled, in percent.
        public var bufferPercent: Double?
        /// Seconds of media ahead of the playhead that are already buffered.
        public var bufferedSecs: Double?
        /// The hardware decoder in use; `no` is software decoding, `nil` is unknown (or audio only).
        public var hwdec: String?
        /// Frames the video output dropped.
        public var droppedFrames: Int
        /// Frames the decoder dropped.
        public var decoderDroppedFrames: Int
        public var connection: ConnectionInfo?
        /// Where playback restarted from a local checkpoint after the connection failed, if it did.
        public var recoveredFromMs: Int?
    }

    public nonisolated struct SleepState: Sendable, Equatable {
        /// `minutes` or `endOfItem`.
        public var mode: String
        /// When the timer ends, in milliseconds since the epoch (`minutes` only).
        public var endsAtMs: Int?
    }
}
