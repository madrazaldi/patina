// PENDING: the one declaration of desktop/core/livetv.ts that `Ops.Playback.livetvWatch`
// calls. The admin agent ports livetv.ts; delete this file when that port lands (its
// `LiveTv.tune` takes its place).

import Foundation
import PlexKit

public enum LiveTv {
    /// Tune a channel: the item of the new live session.
    public static func tune(_ server: PlexServer, _ dvr: String, _ channel: String) async throws -> Metadata {
        throw CoreError("Live TV is not available yet.")
    }
}
