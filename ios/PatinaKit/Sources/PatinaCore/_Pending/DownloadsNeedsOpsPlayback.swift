// PENDING (downloads agent): `ops/playback.ts` is ported elsewhere. Only what
// `Commands.playDownloads` calls; delete this file when the real `Ops.Playback` lands.

import Foundation

extension Ops {
    public enum Playback {
        /// Play downloads offline, in the order given.
        public static func playDownloads(_ app: AppState, _ ids: [String]) throws {
            throw CoreError("Playing downloads is not available yet.")
        }
    }
}
