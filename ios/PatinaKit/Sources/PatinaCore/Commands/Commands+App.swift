// Port of the "App" section of desktop/commands/index.ts.

import Foundation
import PlexKit

extension Commands {
    /// The settings the front end shows.
    public struct AppSettings: Sendable, Equatable {
        public var player: PlayerSettings
        public var ui: UiSettings
    }

    public static func snapshot(_ app: AppState) -> Snapshot {
        app.snapshot()
    }

    public static func info(_ app: AppState) -> Ops.Setup.Info {
        Ops.Setup.info(app.paths)
    }

    public static func settings(_ app: AppState) -> AppSettings {
        let s = app.settings()
        return AppSettings(player: s.player, ui: s.ui)
    }

    /// Change some player settings: `player` holds the changed keys (`["continuous": false]`).
    public static func setPlayerSettings(_ app: AppState, player: JSONValue) throws {
        try app.tryUpdateSettings { try Config.patch(&$0.player, player) }
    }

    /// Change some appearance settings: `ui` holds the changed keys (`["accent": "amber"]`).
    public static func setUiSettings(_ app: AppState, ui: JSONValue) throws {
        try app.tryUpdateSettings { try Config.patch(&$0.ui, ui) }
    }

    public static func openUrl(_ host: any Host, url: String) async throws {
        try await host.openUrl(url)
    }
}
