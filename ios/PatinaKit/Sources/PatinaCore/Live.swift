// Port of desktop/core/live.ts.
// Live updates from the server's notification stream (SSE).
//
// Library changes and playback on other devices bump the revision so views refetch;
// background activities (scans, analysis) are mirrored for the sidebar and the Activity
// page; session changes nudge the Activity page. The listener reconnects on its own and
// stops when the session changes.

import Foundation
import PlexKit

nonisolated private let log = logger("live")

public enum Live {
    /// Changes collected while a refetch is scheduled.
    final class Pending {
        var scheduled = false
        var changes = Changes()
    }

    /// Start listening to the current server, replacing any previous listener.
    ///
    /// The previous listener is cancelled, not just told to stop: it may be parked waiting for
    /// the old server's next event, which can take arbitrarily long (and holds its connection
    /// open meanwhile).
    public static func watch(_ app: AppState) {
        app.liveGeneration += 1
        let generation = app.liveGeneration
        // Replacing the slot is synchronous, so concurrent calls cannot leave two running.
        app.liveTask?.abort()
        let task = LiveTask()
        app.liveTask = task
        task.task = app.spawn {
            await run(app, generation)
            task.finished = true
        }
    }

    private static func run(_ app: AppState, _ generation: Int) async {
        func current() -> Bool {
            !Task.isCancelled && app.liveGeneration == generation
        }
        let pending = Pending()
        var backoff: Double = 2
        while current() {
            guard let server = try? app.server() else { return }
            let list = try? await server.activities()
            if !current() { return }
            if let list {
                app.setConnected(true)
                app.setActivities(list.compactMap(activityView))
            }
            do {
                let stream = try await server.notifications([])
                if !current() { return }
                app.setConnected(true)
                backoff = 2
                var lastSessions = Date().timeIntervalSince1970 - 5
                // Leaving the loop (or the task being cancelled) closes the connection.
                for try await n in stream {
                    if !current() { return }
                    lastSessions = handle(app, n.event, n.data, pending, lastSessions)
                }
            } catch {
                if !current() { return }
                log.debug("notifications unavailable: \(errorText(error))")
            }
            if !current() { return }
            let reachable = (try? await server.capabilities()) != nil
            if !current() { return }
            app.setConnected(reachable)
            try? await Task.sleep(for: .seconds(backoff))
            backoff = min(backoff * 2, 60)
        }
    }

    /// Stop listening (signing out).
    public static func stop(_ app: AppState) {
        app.liveGeneration += 1
        let old = app.liveTask
        app.liveTask = nil
        old?.abort()
        app.setActivities([])
    }

    /// Whether a notification listener is running.
    public static func listening(_ app: AppState) -> Bool {
        guard let task = app.liveTask else { return false }
        return !task.finished
    }

    /// Handles one notification; returns the (possibly updated) time sessions were last
    /// announced, in seconds since 1970.
    static func handle(
        _ app: AppState, _ event: String, _ data: JSONValue, _ pending: Pending, _ lastSessions: TimeInterval
    ) -> TimeInterval {
        var lastSessions = lastSessions
        switch event {
        case "timeline":
            // Items added, updated or deleted in a library. State 5 is "done", 9 "deleted".
            for e in entries(data, "TimelineEntry") {
                let state = e["state"]?.doubleValue
                let library = e["identifier"] == "com.plexapp.plugins.library"
                let changed = library && (state == 5 || state == 9 || e["metadataState"] == "deleted")
                if changed { bumpSoon(app, pending, text(.object(e), "sectionID"), text(.object(e), "itemID")) }
            }
        case "activity":
            for n in entries(data, "ActivityNotification") {
                guard let raw = n["Activity"], raw.objectValue != nil,
                    let activity = try? Activity(json: raw), let view = activityView(activity)
                else { continue }
                let ended = n["event"] == "ended"
                let library = view.kind.hasPrefix("library.") || view.kind.hasPrefix("media.")
                let section = view.sectionId
                app.updateActivities { list in
                    list.removeAll { $0.uuid == view.uuid }
                    if !ended { list.append(view) }
                }
                if ended && library {
                    bumpSoon(app, pending, section, nil)
                    app.spawn { try await Session.reloadSections(app) }
                }
            }
        case "playing", "transcodeSession.start", "transcodeSession.end", "transcodeSession.update":
            let now = Date().timeIntervalSince1970
            if now - lastSessions >= 1 {
                lastSessions = now
                app.emit(.sessionsChanged)
            }
            // Something stopped playing (here or on another device): watch state may have changed.
            for n in entries(data, "PlaySessionStateNotification") where n["state"] == "stopped" {
                bumpSoon(app, pending, text(.object(n), "librarySectionID"), text(.object(n), "ratingKey"))
            }
        default:
            break
        }
        return lastSessions
    }

    /// A string or number field as a string.
    public nonisolated static func text(_ v: JSONValue, _ key: String) -> String? {
        switch v[key] {
        case .string(let s)? where !s.isEmpty: s
        case .number(let n)?: JSONValue.number(n).serialized()
        default: nil
        }
    }

    /// Coalesce bursts of changes (a scan adds many items) into one refetch that names every
    /// section and item touched. A change with no section means "anywhere".
    private static func bumpSoon(_ app: AppState, _ pending: Pending, _ section: String?, _ item: String?) {
        pending.changes.add(section, item)
        if pending.scheduled { return }
        pending.scheduled = true
        app.spawn {
            try? await Task.sleep(for: .milliseconds(1500))
            pending.scheduled = false
            let changes = pending.changes
            pending.changes = Changes()
            app.bumpWith(changes)
        }
    }

    /// Notification payloads arrive as `{Key: [...]}` or `{Key: {...}}`, sometimes wrapped in a
    /// `NotificationContainer`.
    public nonisolated static func entries(_ data: JSONValue, _ key: String) -> [[String: JSONValue]] {
        guard case .object(let outer) = data else { return [] }
        let inner = outer["NotificationContainer"] ?? data
        switch inner[key] {
        case .array(let a)?: return a.compactMap(\.objectValue)
        case .object(let o)?: return [o]
        default: return []
        }
    }

    public nonisolated static func activityView(_ a: Activity) -> ServerActivity? {
        guard let uuid = a.uuid else { return nil }
        let section: String? =
            switch a.context["librarySectionID"] {
            case nil, .null?: nil
            case .string(let s)?: s
            case let v?: v.serialized()
            }
        return ServerActivity(
            uuid: uuid, kind: a.type ?? "", title: a.title ?? "",
            subtitle: a.subtitle.flatMap { $0.isEmpty ? nil : $0 }, progress: a.progress ?? 0,
            cancellable: a.cancellable, sectionId: section)
    }
}
