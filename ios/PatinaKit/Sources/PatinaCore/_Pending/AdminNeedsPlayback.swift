// STUB (admin agent): what Commands+LiveTv calls from desktop/core/ops/playback.ts. The
// playback agent's port of that module replaces this file; delete it when merging.

import Foundation

extension Ops {
    public enum Playback {
        /// Tune a Live TV channel and play it; `title` names it if the tuner doesn't.
        public static func livetvWatch(_ app: AppState, _ dvr: String, _ channel: String, _ title: String) async throws {
            throw CoreError("Live TV playback is not ported yet.")
        }
    }
}
