// Port of desktop/core/state.test.ts. Payloads are the enum's associated values, so only the
// names are checked against what the webview listens for.

import Foundation
import PlexKit
import Testing

@testable import PatinaCore

@Suite struct StateTests {
    let env = TestEnv()

    @Test func eventNamesAreWhatTheWebviewListensFor() {
        let cases: [(CoreEvent, String)] = [
            (.connection(online: false), "connection"),
            (.settingsChanged, "settings-changed"),
            (.stage(.ready), "stage"),
            (.revision(3), "revision"),
            (.changes(.section("1", "10")), "library-changed"),
            (.toast(Toast(text: "hi", error: false)), "toast"),
            (.sessionsChanged, "sessions-changed"),
            (.session(nil, sections: []), "session"),
            (.nowPlaying(nil), "now-playing"),
            (.playbackState(nil), "playback-state"),
            (.playbackFailure(nil), "playback-failure"),
        ]
        for (event, name) in cases { #expect(eventName(event) == name) }
        #expect(Changes.section("1", "10") == Changes(all: false, sections: ["1"], items: ["10"]))
    }

    @Test func settersStoreTheValueAndThenEmit() {
        let seen = Recorder()
        let app = env.app(seen)
        defer { app.dispose() }
        #expect(app.snapshot().stage == .booting)
        app.setStage(.login)
        app.toast("a")
        app.toastUndo("b", 7)
        app.error("c")
        app.bumpWith(.everything())
        #expect(seen.events.map(eventName) == ["stage", "toast", "toast", "toast", "library-changed", "revision"])
        #expect(seen.events[2] == .toast(Toast(text: "b", error: false, undo: 7)))
        #expect(seen.events[3] == .toast(Toast(text: "c", error: true)))
        let snap = app.snapshot()
        #expect(snap.stage == .login)
        #expect(snap.revision == 1)
        #expect(snap.connected == true)
        #expect(throws: CoreError("Not connected to a server.")) { try app.server() }
        #expect(throws: CoreError("Not signed in to plex.tv.")) { try app.plexTv() }
        app.setConnected(false)
        #expect(seen.events.last == .connection(online: false))
    }

    @Test func settingsUpdatesAreSavedToTheFileAndReRead() {
        let app = env.app()
        defer { app.dispose() }
        app.updateSettings { $0.player.quality = "4m-720" }
        #expect(app.settings().player.quality == "4m-720")
        #expect(Config.loadSettings(env.environment).player.quality == "4m-720")
    }

    @Test func spawnLogsFailuresInsteadOfLosingThem() async {
        let app = env.app()
        defer { app.dispose() }
        #expect(await app.spawn { 5 }.value == 5)
        #expect(await app.spawn { () async throws -> Int in throw CoreError("boom") }.value == nil)
        await app.settled()
    }

    @Test func theClientInfoDescribesTheDevice() {
        let app = env.app()
        defer { app.dispose() }
        let info = app.clientInfo()
        #expect(info.product == "Patina")
        #expect(info.version == Config.VERSION)
        #expect(info.platform == "iOS")
        #expect(info.device == "iPhone")
        #expect(info.deviceName == "Test iPhone")
        #expect(info.identifier == app.settings().clientId)
    }

    @Test func thePollerAnnouncesSettingsAndStopsOnDispose() async throws {
        let ticking = TestEnv(tick: .milliseconds(20))
        let seen = Recorder()
        let app = ticking.app(seen)
        try await until("the first tick") { seen.events.contains(.settingsChanged) }
        app.dispose()
        try await Task.sleep(for: .milliseconds(60))
        let count = seen.events.count
        try await Task.sleep(for: .milliseconds(100))
        #expect(seen.events.count == count)
    }
}
