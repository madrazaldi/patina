// Port of desktop/test/session.test.ts: the sign-in flow against the mock server (manual
// server, restore on startup, sign out).

import Foundation
import PlexKit
import Testing

@testable import PatinaCore

@Suite struct SessionIntegrationTests {
    @Test func aManualServerConnectsIsRememberedRestoresAndSignsOut() async throws {
        let mock = MockPMS()
        let env = TestEnv()
        let seen = Recorder()
        let app = env.app(seen)
        defer {
            Live.stop(app)
            app.dispose()
        }
        await Session.restore(app)
        #expect(app.snapshot().stage == .login)

        try await Session.signInManual(app, mock.url.replacingOccurrences(of: "http://", with: ""), " x ")
        #expect(app.snapshot().stage == .ready)
        #expect(app.session()?.owned == true)
        #expect(app.session()?.serverName == "Mock Server")
        #expect(app.sections().count > 0)
        let settings = app.settings()
        #expect(settings.server.manualUrl == mock.url)
        #expect(settings.server.manualToken == "x")
        #expect(env.secrets.get("server.manual_token") == "x")
        #expect(settings.auth.accountId?.isEmpty == false)
        #expect(Images.currentServer() != nil)
        #expect(Live.listening(app))
        #expect(seen.events.contains { if case .session = $0 { true } else { false } })
        try await until("the listener to connect") { mock.openStreams == 1 }

        // A new launch restores the saved manual server.
        let again = env.app()
        await Session.restore(again)
        #expect(again.snapshot().stage == .ready)
        #expect(again.session()?.serverName.isEmpty == false)
        Live.stop(again)
        again.dispose()

        Session.signOut(app)
        #expect(app.snapshot().stage == .login)
        #expect(app.session() == nil)
        #expect(app.settings().server.manualUrl == nil)
        #expect(env.secrets.get("server.manual_token") == nil)
        #expect(Images.currentServer() == nil)
        #expect(!Live.listening(app))
        try await until("the listeners to hang up") { mock.openStreams == 0 }
    }

    @Test func aManualServerThatIsDownShowsAnErrorOnTheForm() async throws {
        let app = TestEnv().app()
        defer { app.dispose() }
        await #expect {
            try await Session.signInManual(app, "127.0.0.1:9", "x")
        } throws: { error in
            errorText(error).hasPrefix("contacting server: ")
        }
        #expect(app.settings().server.manualUrl == nil)
    }
}
