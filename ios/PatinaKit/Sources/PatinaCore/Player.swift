// STUB: the playback agent replaces this file with the port of desktop/core/player.ts.
// Only what the foundation calls: `control`.

import Foundation

public enum Player {
    public nonisolated enum Control: Sendable, Equatable {
        case togglePause
        case play
        case pause
        case next
        case previous
        case stop
        case seekRelative(seconds: Double)
        case seekTo(seconds: Double)
    }

    /// Drive the running player; nothing happens when nothing plays.
    public static func control(_ cmd: Control) {}
}
