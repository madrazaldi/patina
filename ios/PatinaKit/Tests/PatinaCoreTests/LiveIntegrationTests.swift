// Port of desktop/test/live.test.ts: the listener lifecycle against fake servers that hold
// their notification stream open.

import Foundation
import PlexKit
import Testing

@testable import PatinaCore

private func session(_ url: String) throws -> Session {
    Session(
        tv: nil, server: try PlexServer(info: testInfo, baseUrl: url, token: "token", session: MockPMS.session),
        serverName: "Fake", serverId: url, user: nil, owned: true, servers: [])
}

@Suite struct LiveIntegrationTests {
    @Test func repeatedServerSwitchesLeaveOneListener() async throws {
        let app = TestEnv().app()
        defer { app.dispose() }
        let servers = [MockPMS(), MockPMS(), MockPMS()]
        defer { servers.forEach { $0.stop() } }
        func total() -> Int { servers.reduce(0) { $0 + $1.openStreams } }

        // Switch between servers over and over, as the session flow does: new session, then a fresh listener.
        for i in 0..<12 {
            let fake = servers[i % servers.count]
            app.setSession(try session(fake.url), [])
            Live.watch(app)
            try await until("the new listener to connect") { fake.openStreams == 1 }
        }
        let last = servers[11 % servers.count]
        #expect(Live.listening(app))
        // Every earlier listener closed its connection; only the current server has one.
        try await until("old listeners to hang up") { total() == 1 }
        #expect(last.openStreams == 1)

        // Signing out leaves none.
        Live.stop(app)
        try await until("the listener to hang up") { total() == 0 }
        try await until("the listener to end") { !Live.listening(app) }

        // Replacing a listener that is waiting out a backoff after its server went away also leaves one.
        servers.forEach { $0.stop() }
        app.setSession(try session("http://127.0.0.1:9"), [])
        Live.watch(app)
        try await Task.sleep(for: .milliseconds(100))
        Live.watch(app)
        Live.stop(app)
        try await Task.sleep(for: .milliseconds(100))
        #expect(!Live.listening(app))
    }

    @Test func libraryNotificationsBumpTheRevisionOnce() async throws {
        let mock = MockPMS()
        let events: [String] = [
            #"{"TimelineEntry":[{"identifier":"com.plexapp.plugins.library","sectionID":"1","itemID":"7","type":1,"state":5}]}"#,
            #"{"TimelineEntry":[{"identifier":"com.plexapp.plugins.library","sectionID":"2","itemID":"8","type":1,"state":9}]}"#,
        ]
        mock.on { _, req in
            guard req.path == "/:/eventsource/notifications" else { return nil }
            let body = events.map { "event: timeline\ndata: \($0)\n\n" }.joined()
            return MockResponse(headers: ["Content-Type": "text/event-stream"], body: Data(body.utf8), hang: true)
        }
        let seen = Recorder()
        let app = TestEnv().app(seen)
        defer {
            Live.stop(app)
            app.dispose()
        }
        app.setSession(try session(mock.url), [])
        Live.watch(app)
        try await until("the coalesced refetch") { seen.events.contains { if case .changes = $0 { true } else { false } } }
        let changes = seen.events.compactMap { if case .changes(let c) = $0 { c } else { nil } }
        #expect(changes == [Changes(all: false, sections: ["1", "2"], items: ["7", "8"])])
    }
}
