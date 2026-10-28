// Port of desktop/core/config.test.ts.

import Foundation
import PlexKit
import Testing

@testable import PatinaCore

@Suite struct ConfigTests {
    let env = TestEnv()

    private var path: URL { Config.configFile(env.paths) }

    private func document() throws -> [String: JSONValue] {
        try JSONValue.parse(Data(contentsOf: path)).objectValue ?? [:]
    }

    @Test func oldSettingsAndSavedViewsRoundTrip() throws {
        var settings = try Config.parseSettings(#"{"ui": {"poster_size": 180}}"#)
        #expect(settings.ui.savedViews == [])
        #expect(settings.ui.posterSize == 180)
        settings.ui.savedViews.append([
            "id": "view-1", "name": "Unwatched drama", "scope": "alice:home", "library": "1", "mediaType": 0,
            "sort": "titleSort", "desc": false, "unwatched": true, "toggles": [],
            "picked": [["filter": "genre", "title": "Genre", "value": "10", "label": "Drama"]],
        ])
        settings.ui.homeLayouts["alice:home"] = HomeLayout(compactContinue: true)
        let text = Config.settingsToJson(settings).serialized()
        #expect(try Config.parseSettings(text) == settings)
    }

    @Test func partialSettingsUpdatePreservesOtherFields() throws {
        var player = PlayerSettings()
        player.quality = "4m-720"
        try Config.patch(&player, ["continuous": false])
        #expect(player.quality == "4m-720")
        #expect(player.continuous == false)
        #expect(throws: (any Error).self) { try Config.patch(&player, ["continuous": "wrong"]) }
        #expect(player.continuous == false)
        #expect(throws: CoreError("Unknown setting: nope")) { try Config.patch(&player, ["nope": 1]) }
        #expect(throws: CoreError("Settings patch must be an object")) { try Config.patch(&player, 3) }
    }

    /// The desktop test checks the TOML layout; here the JSON document: the TOML keys, no
    /// `None` fields, no tokens.
    @Test func theFileLayoutMatchesTheTomlKeys() throws {
        let settings = Config.loadSettings(env.environment)
        let doc = try document()
        #expect(Set(doc.keys) == ["client_id", "auth", "server", "player", "ui", "tui", "seerr"])
        #expect(doc["client_id"] == .string(settings.clientId))
        #expect(doc["auth"] == [:])
        #expect(doc["player"]?["watched_threshold"] == 0.9)
        #expect(doc["player"]?["skip_key"] == "TAB")
        #expect(doc["seerr"] == ["url": ""])
    }

    @Test func tokensGoToTheSecretStoreNeverToTheFile() throws {
        let first = Config.loadSettings(env.environment)
        let saved = try Config.updateSettings(env.environment, first) { s in
            s.auth.token = "plex-token"
            s.auth.accountId = "alice"
            s.server.manualUrl = "http://nas:32400"
            s.server.manualToken = "server-token"
            s.seerr = SeerrSettings(url: "http://seerr.local:5055", apiKey: "seerr-key")
        }
        let text = try String(contentsOf: path, encoding: .utf8)
        for secret in ["plex-token", "server-token", "seerr-key", "\"token\"", "manual_token", "api_key"] {
            #expect(!text.contains(secret))
        }
        #expect(
            env.secrets.all == [
                "auth.token": "plex-token", "server.manual_token": "server-token", "seerr.api_key": "seerr-key",
            ])
        // Read back with the secrets.
        #expect(Config.loadSettings(env.environment) == saved)
        // Clearing a token deletes it.
        _ = try Config.updateSettings(env.environment, saved) { $0.auth.token = nil }
        #expect(env.secrets.get("auth.token") == nil)
    }

    @Test func aFreshInstallForgetsTokensAPreviousInstallLeft() throws {
        // The Keychain outlives the app's files.
        try env.secrets.set("auth.token", "stale")
        let settings = Config.loadSettings(env.environment)
        #expect(settings.auth.token == nil)
        #expect(env.secrets.get("auth.token") == nil)
    }

    @Test func seerrSettingsRoundTripAndDefaultToEmpty() throws {
        #expect(try Config.parseSettings("").seerr == SeerrSettings())
        let settings = try Config.parseSettings(#"{"seerr": {"url": "http://seerr.local:5055", "api_key": "k"}}"#)
        #expect(settings.seerr == SeerrSettings(url: "http://seerr.local:5055", apiKey: "k"))
        try Config.saveSettings(env.environment, settings)
        #expect(Config.loadSettings(env.environment).seerr == settings.seerr)
        #expect(throws: (any Error).self) { try Config.parseSettings(#"{"seerr": {"url": 3}}"#) }
    }

    @Test func theClientIdIsCreatedOnceAndKept() {
        let first = Config.loadSettings(env.environment)
        let second = Config.loadSettings(env.environment)
        #expect(second.clientId == first.clientId)
        #expect(path.deletingLastPathComponent().path == env.paths.config.path)
    }

    @Test func unknownKeysAndSectionsSurviveARewrite() throws {
        _ = Config.loadSettings(env.environment)
        var doc = try document()
        var player = doc["player"]?.objectValue ?? [:]
        player["future_option"] = "x"
        doc["player"] = .object(player)
        doc["extras"] = ["keep": [1, 2]]
        try Data(JSONValue.object(doc).serialized().utf8).write(to: path)
        let s = try Config.updateSettings(env.environment, Config.loadSettings(env.environment)) {
            $0.player.quality = "2m-720"
        }
        #expect(s.player.quality == "2m-720")
        let after = try document()
        #expect(after["player"]?["future_option"] == "x")
        #expect(after["extras"] == ["keep": [1, 2]])
        #expect(after["player"]?["quality"] == "2m-720")
    }

    @Test func anUnreadableFileIsNeverOverwrittenWithDefaults() throws {
        let first = Config.loadSettings(env.environment)
        try Data("this is = = not json".utf8).write(to: path)
        #expect(throws: CoreError("Settings could not be read; the existing file was preserved.")) {
            try Config.updateSettings(env.environment, first) { _ in }
        }
        #expect(try String(contentsOf: path, encoding: .utf8) == "this is = = not json")
        // Reload falls back to the settings in memory.
        #expect(Config.reloadSettings(env.environment, first) == first)
    }

    @Test func aWronglyTypedFieldFailsTheWholeFileLikeSerde() throws {
        #expect(throws: (any Error).self) { try Config.parseSettings(#"{"player": {"continuous": "yes"}}"#) }
        #expect(throws: (any Error).self) { try Config.parseSettings(#"{"ui": {"poster_size": -3}}"#) }
        #expect(try Config.parseSettings(#"{"player": {"watched_threshold": 1}}"#).player.watchedThreshold == 1)
    }

    @Test func reloadKeepsTheClientIdWhenTheFileHasNone() throws {
        let mine = Config.loadSettings(env.environment)
        try Data(#"{"tui": {"hide_watched": true}}"#.utf8).write(to: path)
        let re = Config.reloadSettings(env.environment, mine)
        #expect(re.clientId == mine.clientId)
        #expect(re.tui.hideWatched == true)
    }

    @Test func exportsForSubSettingsStayUsable() {
        #expect(PlayerSettings().skipKey == "TAB")
        #expect(HomeLayout().order == [])
    }

    @Test func theVersionMatchesTheWorkspaceVersion() throws {
        // tools/release.sh bumps both; the packaged app reads Cargo.toml.
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let cargo = try String(contentsOf: root.appendingPathComponent("Cargo.toml"), encoding: .utf8)
        let version = cargo.firstMatch(of: #/(?m)^version = "([^"]+)"/#).map { String($0.1) }
        #expect(version == Config.VERSION)
    }
}
