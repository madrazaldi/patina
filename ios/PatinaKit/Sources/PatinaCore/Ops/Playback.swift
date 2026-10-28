// Port of desktop/core/ops/playback.ts.
// Starting playback from things that aren't library items.

import Foundation
import PlexKit

nonisolated private let log = logger("playback")

extension Ops {
    public enum Playback {
        /// The finished downloads among `ids`, in that order (`downloads.files`, which the
        /// downloads port provides; done here against the `Download` type until then).
        static func files(_ paths: Paths, _ ids: [String]) -> [Download] {
            let all = Downloads.list(paths)
            return ids.compactMap { id in
                all.first { $0.id == id && $0.state == .done }.flatMap { d in
                    d.file.map { FileManager.default.fileExists(atPath: $0) } == true ? d : nil
                }
            }
        }

        /// Play downloads offline, in the order given.
        public static func playDownloads(_ app: AppState, _ ids: [String]) throws {
            let files = files(app.paths, ids)
            let account = app.session().map(Offline.account) ?? app.settings().auth.accountId ?? ""
            if files.contains(where: { $0.account != "" && $0.account != account }) {
                throw CoreError("These downloads belong to another Plex profile. Switch to that profile to play them.")
            }
            if files.isEmpty { throw CoreError("The downloaded file is missing.") }
            Player.playFiles(app, files)
        }

        /// Tune a Live TV channel and play it; `title` names it if the tuner doesn't.
        public static func livetvWatch(_ app: AppState, _ dvr: String, _ channel: String, _ title: String) async throws {
            var item = try await LiveTv.tune(app.server(), dvr, channel)
            if item.title == nil { item.title = title }
            Player.play(app, Player.PlayRequest(item: item, resume: false, live: true))
        }

        /// The queue that was playing when Patina last stopped (for this account and server),
        /// to offer a Resume.
        public static func savedQueue(_ app: AppState) -> SavedQueueView? {
            guard let s = app.session() else { return nil }
            return Recovery.loadQueue(app.paths, Scope.of(s)).flatMap(Recovery.savedQueueView)
        }

        /// Play the saved queue again from where it stopped.
        public static func resumeSaved(_ app: AppState) throws {
            guard let s = app.session() else { throw CoreError("Not connected to a server.") }
            guard let saved = Recovery.loadQueue(app.paths, Scope.of(s)) else {
                throw CoreError("There is no saved queue to resume.")
            }
            let item = saved.items[saved.current]
            let selection = saved.items.count > 1 ? saved.items : []
            Player.playFrom(
                app, Player.PlayRequest(item: item, selection: selection, resume: true, quality: saved.quality),
                saved.positionMs)
        }

        /// Forget the saved queue.
        public static func forgetSaved(_ app: AppState) {
            guard let s = app.session() else { return }
            do {
                try Recovery.clearQueue(app.paths, Scope.of(s))
            } catch {
                log.debug("could not forget the saved queue: \(errorText(error))")
            }
        }
    }
}
