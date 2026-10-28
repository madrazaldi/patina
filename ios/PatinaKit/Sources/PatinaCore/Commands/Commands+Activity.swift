// Port of the "Activity", "Server administration" and "Server administration: libraries,
// stats, diagnostics" sections of desktop/commands/index.ts.

import Foundation
import PlexKit

extension Commands {
    /// Library maintenance (`section_action`).
    public enum SectionAction: String, Sendable {
        /// Look for new files.
        case scan
        /// Re-download metadata.
        case refresh
        case analyze
        case emptyTrash
    }

    /// Server maintenance (`server_action`).
    public enum ServerAction: String, Sendable {
        case scanAll, cleanBundles, optimizeDatabase
    }

    /// What `saveDiagnostics` saves.
    public enum Diagnostics: String, Sendable {
        case logs, database
    }

    // MARK: Activity

    /// Playback sessions on the server.
    public static func sessions(_ app: AppState) async throws -> [Metadata] {
        try await app.server().sessions()
    }

    public static func terminateSession(_ app: AppState, id: String) async throws {
        try await app.server().terminateSession(id, reason: "Stopped by the server owner.")
    }

    /// Page `page` of the server's watch history, newest first.
    public static func history(_ app: AppState, page: Int) async throws -> Ops.Lists.HistoryPage {
        try await Ops.Lists.history(app, page)
    }

    /// Delete a history entry by its `historyKey`.
    public static func deleteHistory(_ app: AppState, key: String) async throws {
        try await Ops.Lists.deleteHistory(app, key)
    }

    /// Background activities and transcoder sessions.
    public static func tasks(_ app: AppState) async throws -> Ops.Server.Tasks {
        try await Ops.Server.tasks(app)
    }

    public static func cancelActivity(_ app: AppState, uuid: String) async throws {
        try await app.server().cancelActivity(uuid)
    }

    /// Stop a transcoder session by its key.
    public static func killTranscode(_ app: AppState, key: String) async throws {
        try await Ops.Server.killTranscode(app, key)
    }

    // MARK: Server administration

    public static func serverOverview(_ app: AppState) async throws -> Ops.Server.Overview {
        try await Ops.Server.serverOverview(app)
    }

    public static func checkForUpdates(_ app: AppState) async throws {
        try await app.server().checkForUpdates(true)
    }

    public static func applyUpdate(_ app: AppState, tonight: Bool = false) async throws {
        try await app.server().applyUpdate(tonight, skip: false)
    }

    public static func sectionAction(_ app: AppState, id: String, action: SectionAction) async throws {
        let s = try app.server()
        switch action {
        case .scan: try await s.refreshSection(id, force: false)
        case .refresh: try await s.refreshSection(id, force: true)
        case .analyze: try await s.analyzeSection(id)
        case .emptyTrash: try await s.emptyTrash(id)
        }
    }

    public static func serverAction(_ app: AppState, action: ServerAction) async throws {
        let s = try app.server()
        switch action {
        case .scanAll: try await s.refreshAllSections(false)
        case .cleanBundles: try await s.cleanBundles()
        case .optimizeDatabase: try await s.optimizeDatabase()
        }
    }

    /// The butler's tasks with their switches, and the maintenance window.
    public static func maintenance(_ app: AppState) async throws -> Ops.Server.Maintenance {
        try await Ops.Server.maintenance(app)
    }

    public static func butlerTask(_ app: AppState, name: String, start: Bool) async throws {
        let s = try app.server()
        if start { try await s.startButlerTask(name) } else { try await s.stopButlerTask(name) }
    }

    /// The server's settings by group (scheduled-task settings are in `maintenance`).
    public static func prefs(_ app: AppState) async throws -> [Ops.Server.PrefGroup] {
        try await Ops.Server.settings(app)
    }

    public static func setPref(_ app: AppState, id: String, value: String) async throws {
        try await app.server().setPref(id, value: value)
    }

    public static func usersDevices(_ app: AppState) async throws -> Ops.Server.UsersDevices {
        try await Ops.Server.usersDevices(app)
    }

    // MARK: Server administration: libraries, stats, diagnostics

    /// Agents and scanners for a new library of `kind` (`movie`, `show`, `artist`, `photo`).
    public static func libraryChoices(_ app: AppState, kind: String) async throws -> Ops.Server.Choices {
        try await Ops.Server.libraryChoices(app, kind)
    }

    public static func createLibrary(_ app: AppState, form: Ops.Server.LibraryForm) async throws {
        try await Ops.Server.createLibrary(app, form)
    }

    /// Rename a library and/or change its folders.
    public static func editLibrary(_ app: AppState, id: String, name: String, locations: [String]) async throws {
        try await Ops.Server.editLibrary(app, id, name, locations)
    }

    public static func deleteLibrary(_ app: AppState, id: String) async throws {
        try await Ops.Server.deleteLibrary(app, id)
    }

    public static func cancelScan(_ app: AppState, id: String) async throws {
        try await app.server().cancelRefreshSection(id)
    }

    public static func libraryPrefs(_ app: AppState, id: String) async throws -> [Ops.Server.Pref] {
        try await Ops.Server.libraryPrefs(app, id)
    }

    public static func setLibraryPref(_ app: AppState, id: String, pref: String, value: String) async throws {
        try await app.server().setSectionPrefs(id, params: Params().set(pref, value))
    }

    public static func butlerAll(_ app: AppState, start: Bool) async throws {
        let s = try app.server()
        if start { try await s.startAllButlerTasks() } else { try await s.stopAllButlerTasks() }
    }

    /// Bandwidth and plays since `since` (unix seconds). `timespan` is Plex's bucket size
    /// code: 6 for hours, 4 for days.
    public static func serverStats(_ app: AppState, timespan: Int, since: Int) async throws -> Ops.Server.Stats {
        try await Ops.Server.serverStats(app, timespan, since)
    }

    /// Save the server's logs or database to the downloads folder (Documents/Downloads, in the
    /// Files app). Returns the file, for a share sheet; the desktop revealed its folder instead.
    public static func saveDiagnostics(_ app: AppState, what: Diagnostics) async throws -> URL {
        try await Ops.Server.saveDiagnostics(app, what.rawValue)
    }
}
