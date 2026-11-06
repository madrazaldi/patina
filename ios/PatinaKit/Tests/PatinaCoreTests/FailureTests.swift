// Port of desktop/core/failure.test.ts, and the playback-failure case of
// desktop/test/servers.test.ts.

import Foundation
import PlexKit
import Testing

@testable import PatinaCore

@Suite struct FailureTests {
    @Test func explainsWhatThePlayerSays() {
        #expect(Failure.explain("loading failed").0 == .network)
        #expect(Failure.explain("unrecognized file format").0 == .format)
        #expect(Failure.explain("no audio or video data played").0 == .format)
        #expect(Failure.explain("audio output initialization failed").0 == .output)
        #expect(Failure.explain("video output initialization failed").0 == .output)
        #expect(Failure.explain("").0 == .other)
        // What the engine reports maps onto the same kinds.
        #expect(Failure.explainEngine(.network).0 == .network)
        #expect(Failure.explainEngine(.unsupported).0 == .format)
        #expect(Failure.explainEngine(.other).0 == .other)
    }

    @Test func lowersQualityOneStepAtATime() {
        #expect(Failure.lowerQuality("original")?.id == "20m-1080")
        #expect(Failure.lowerQuality("20m-1080")?.id == "12m-1080")
        #expect(Failure.lowerQuality("2m-720")?.id == "1m-480")
        #expect(Failure.lowerQuality("1m-480") == nil)
        #expect(Failure.lowerQuality("nonsense")?.id == "20m-1080")
    }

    @Test func networkErrorsAreRecognisedThroughTheErrorChain() {
        let inner = PlexError.noConnection("Home")
        let e = ChainedError("creating play queue: \(inner.message)", cause: inner)
        #expect(Failure.explainError(e).0 == .network)
        #expect(Failure.explainError(e).1 == "creating play queue: no reachable connection for server \"Home\"")
        #expect(Failure.explainError(CoreError("the player is missing")).0 == .other)
    }
}

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

@Suite struct FailureServersTests {
    @Test func aPlaybackFailureIsNotRetriedOnAnotherServer() async throws {
        let alpha = MockPMS(name: "Alpha", machine: "alpha-failure", seed: 0)
        let beta = MockPMS(name: "Beta", machine: "beta-failure", seed: 1)
        defer {
            alpha.stop()
            beta.stop()
        }
        let app = TestEnv().app()
        defer { app.dispose() }
        app.setSession(try sessionFor(alpha, [beta]), [])
        let item = try #require(try await app.server().metadata("112"))
        let onAlpha = Scope.of(app.session()!)
        func report() -> Failure.Report {
            Failure.Report(
                item: item, error: "loading failed", phase: .start, positionMs: 61_000, mediaIndex: 0,
                quality: "original", remaining: 0, request: Player.PlayRequest(item: item), scope: onAlpha)
        }

        // Connecting to the same server again keeps the failure on offer.
        Failure.record(app, report())
        app.setSession(app.session(), [])
        #expect(Failure.current(app) != nil)
        #expect(Failure.current(app)?.kind == .network)
        #expect(Failure.current(app)?.detail == "loading failed")

        // Another server: its rating key "112" is a different item, so the failure is dropped.
        app.setSession(try sessionFor(beta, [alpha]), [])
        #expect(Failure.current(app) == nil)
        #expect(throws: CoreError.self) { try Failure.recover(app, .retry) }

        // Alpha's playback failing late, after the switch, is not offered on Beta either.
        Failure.record(app, report())
        #expect(Failure.current(app) == nil)
    }
}
