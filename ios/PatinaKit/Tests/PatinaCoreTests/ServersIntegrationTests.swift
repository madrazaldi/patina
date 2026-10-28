// Port of desktop/test/servers.test.ts: several mock servers, keeping each item with its own
// server, and undo. The cross-server search tests belong with Ops.XSearch and the playback
// failure test with Failure; their owners port them.

import Foundation
import PlexKit
import Testing

@testable import PatinaCore

private func resource(_ m: MockPMS) -> Resource {
    var r = Resource(
        name: m.name, clientIdentifier: m.machine, provides: "server", accessToken: "t", owned: true, presence: true)
    r.connections = [Connection(protocol: "http", address: "127.0.0.1", port: m.port, uri: m.url, local: true, relay: false)]
    return r
}

private func sessionFor(_ current: MockPMS, _ others: [MockPMS]) throws -> Session {
    let http = Http(info: testInfo, session: MockPMS.session)
    return Session(
        tv: PlexTv(info: testInfo, token: "x", http: http),
        server: try PlexServer(http: http, baseUrl: current.url, token: "alice"), serverName: current.name,
        serverId: current.machine, user: nil, owned: true, servers: ([current] + others).map(resource))
}

/// An app connected to `current`, on an account that also has the `others`.
private func appFor(_ env: TestEnv, _ current: MockPMS, _ others: [MockPMS]) throws -> AppState {
    let app = env.app()
    app.setSession(try sessionFor(current, others), [])
    return app
}

@Suite struct ServersIntegrationTests {
    let env = TestEnv()

    @Test func watchStateChangesAndTheirUndoStayOnTheirOwnServer() async throws {
        let alpha = MockPMS(name: "Alpha", machine: "alpha-watch", seed: 0)
        let beta = MockPMS(name: "Beta", machine: "beta-watch", seed: 1)
        let app = try appFor(env, alpha, [beta])
        defer { app.dispose() }
        let here = try app.server()
        let there = try await Servers.serverFor(app, "beta-watch")
        func watched(_ s: PlexServer) async throws -> Bool { Metadata.isWatched(try await s.metadata("112")!) }

        // Marking the other server's item leaves this server's item with the same key alone.
        try await Undo.setWatched(app, "beta-watch", ["112"], true)
        #expect(try await watched(there))
        #expect(try await !watched(here))
        let last = Undo.last(app)
        #expect(last != nil)  // the change is undoable
        #expect(last?.label.contains("watched") == true)
        try await Undo.undo(app, nil)
        #expect(try await !watched(there))
        #expect(Undo.last(app) == nil)

        // Bulk: several at once, undone together. Whatever was watched before stays watched.
        let keys = ["112", "113", "114"]
        func states() async throws -> [Bool] {
            var out: [Bool] = []
            for key in keys { out.append(Metadata.isWatched(try await here.metadata(key)!)) }
            return out
        }
        let before = try await states()
        #expect(before.contains(false))  // the test needs an unwatched item
        try await Undo.setWatched(app, nil, keys, true)
        #expect(try await states() == [true, true, true])
        try await Undo.undo(app, nil)
        #expect(try await states() == before)
    }

    @Test func playlistEntriesComeBackWhereTheyWereAndResumePointsReturn() async throws {
        let alpha = MockPMS(name: "Alpha", machine: "alpha-list", seed: 0)
        let app = try appFor(env, alpha, [])
        defer { app.dispose() }
        let server = try app.server()
        func order() async throws -> [String] {
            try await server.playlistItems("950", start: 0, size: 100).metadata.map(Metadata.ratingKey)
        }
        #expect(try await order() == ["104", "105", "1012"])

        // The middle entry goes back between its neighbours.
        try await Undo.removeFromPlaylist(app, "950", 7105)
        #expect(try await order() == ["104", "1012"])
        try await Undo.undo(app, nil)
        #expect(try await order() == ["104", "105", "1012"])

        // So does the first.
        try await Undo.removeFromPlaylist(app, "950", 7104)
        #expect(try await order() == ["105", "1012"])
        try await Undo.undo(app, nil)
        #expect(try await order() == ["104", "105", "1012"])

        // Hiding an item from Continue Watching and clearing its progress give the resume point back.
        func offset() async throws -> Int { try await server.metadata("100")?.viewOffset ?? 0 }
        let before = try await offset()
        #expect(before > 0)  // the mock item has a resume point
        try await Undo.clearProgress(app, "100")
        #expect(try await offset() == 0)
        try await Undo.undo(app, nil)
        #expect(try await offset() == before)

        try await Undo.removeFromContinueWatching(app, "100")
        try await Undo.undo(app, nil)
        #expect(try await offset() == before)
    }

    @Test func undoHistoryIsDroppedWhenTheServerChanges() async throws {
        let alpha = MockPMS(name: "Alpha", machine: "alpha-scope", seed: 0)
        let beta = MockPMS(name: "Beta", machine: "beta-scope", seed: 1)
        let app = try appFor(env, alpha, [beta])
        defer { app.dispose() }
        try await Undo.setWatched(app, nil, ["112"], true)
        #expect(Undo.last(app) != nil)

        // A different session on the same state: the old entries must not apply to the new server.
        app.setSession(try sessionFor(beta, [alpha]), [])
        #expect(Undo.last(app) == nil)
        await #expect(throws: (any Error).self) { try await Undo.undo(app, nil) }
    }

    @Test func anActionFinishingAfterASwitchIsNotUndoableOnTheNewServer() async throws {
        let alpha = MockPMS(name: "Alpha", machine: "alpha-late", seed: 0, reportDelay: 1)
        let beta = MockPMS(name: "Beta", machine: "beta-late", seed: 1)
        let app = try appFor(env, alpha, [beta])
        defer { app.dispose() }
        // Alpha holds the scrobble; the switch to Beta happens while it is in flight.
        let marking = Task { try await Undo.setWatched(app, nil, ["112"], true) }
        try await Task.sleep(for: .milliseconds(300))
        app.setSession(try sessionFor(beta, [alpha]), [])
        try await marking.value
        // Undo would unmark Beta's "112", a different item.
        #expect(Undo.last(app) == nil)
    }

    @Test func otherServersAreReachedAndNamedThroughTheAccount() async throws {
        let alpha = MockPMS(name: "Alpha", machine: "alpha-names", seed: 0)
        let beta = MockPMS(name: "Beta", machine: "beta-names", seed: 1)
        let app = try appFor(env, alpha, [beta])
        defer { app.dispose() }
        #expect(Servers.isCurrent(app, nil))
        #expect(Servers.isCurrent(app, "alpha-names"))
        #expect(!Servers.isCurrent(app, "beta-names"))
        #expect(Servers.nameOf(app, "beta-names") == "Beta")
        #expect(Servers.nameOf(app, nil) == "Alpha")
        let there = try await Servers.serverFor(app, "beta-names")
        #expect(there.machineIdentifier == "beta-names")
        // The connection is kept.
        #expect(try await Servers.serverFor(app, "beta-names") === there)
        await #expect(throws: CoreError("That server is no longer on this account.")) {
            try await Servers.serverFor(app, "gone")
        }
        // Items from another server are tagged with it, and the current server's are not.
        var items = [Metadata(ratingKey: "1")]
        Servers.tag(app, &items, "beta-names")
        #expect(Metadata.serverId(items[0]) == "beta-names")
        Servers.tag(app, &items, "alpha-names")
        #expect(Metadata.serverId(items[0]) == nil)
    }
}
