// Port of desktop/test/downloads.test.ts (the download and offline tests of
// crates/core/tests/reliability.rs). The TypeScript runs each half in a fresh `bun test`
// process against tools/mock_pms.py; here the halves are two `AppState`s on the same files,
// and the first one's worker is stopped mid-transfer (`Downloads.stop`) as if the process
// exited. The transfer is the foreground one (the environment brings its own URL session).

import Foundation
import PlexKit
import Testing

@testable import PatinaCore

private let MOVIE_BYTES = 8 * 1024 * 1024
private let movie = Data(repeating: 42, count: MOVIE_BYTES)

private func session(_ mock: MockPMS, _ token: String) throws -> Session {
    Session(
        tv: nil, server: try PlexServer(info: testInfo, baseUrl: mock.url, token: token, session: MockPMS.session),
        serverName: "Mock", serverId: mock.machine, user: nil, owned: true, servers: [])
}

private func first(_ paths: Paths, _ key: String = "100") throws -> Download {
    try #require(Downloads.list(paths).first { $0.item.ratingKey == key })
}

@Suite(.serialized) struct DownloadsIntegrationTests {
    @Test func interruptedDownloadsAndOfflineSync() async throws {
        try await downloadReliability(.normal)
    }

    @Test func resumedDownloadWithoutValidatorRestarts() async throws {
        try await downloadReliability(.missing)
    }

    @Test func resumedDownloadWithChangedValidatorRestarts() async throws {
        try await downloadReliability(.changed)
    }

    func downloadReliability(_ validator: MediaFile.Validator) async throws {
        let env = TestEnv()
        let paths = env.paths
        let mock = MockPMS(missingMetadata: ["999998"])
        mock.serve(MediaFile(movie, delay: 0.012, dropOnce: true, validator: validator))
        defer { mock.stop() }

        // download_and_offline_worker
        let app = env.app()
        app.setSession(try session(mock, "alice"), [])
        _ = try await Downloads.enqueue(app, ratingKey: "100")
        try await until("the download to start", timeout: .seconds(15)) { ((try? first(paths))?.received ?? 0) > 0 }
        var d = try first(paths)
        Downloads.pause(app, d.id)
        try await Task.sleep(for: .milliseconds(350))
        #expect(try first(paths).state == .paused)
        let partial = try #require(d.file) + ".part"
        let size = try FileManager.default.attributesOfItem(atPath: partial)[.size] as? Int ?? 0
        #expect(size > 0)
        if validator != .normal {
            // Simulate a partial from a different source with the same length.
            let f = try #require(FileHandle(forUpdatingAtPath: partial))
            try f.write(contentsOf: Data(count: 1024))
            try f.close()
        }
        Downloads.retry(app, d.id)
        try await until("the download to finish", timeout: .seconds(30)) {
            let s = (try? first(paths).state) ?? .failed
            return s == .done || s == .failed
        }
        d = try first(paths)
        #expect(d.state == .done, "\(d.error ?? "")")
        #expect(try Data(contentsOf: URL(fileURLWithPath: try #require(d.file))) == movie)

        app.setSession(try session(mock, "bob"), [])
        // Both a 404 and an empty metadata response must leave later outbox entries usable.
        for key in ["999998", "999999"] {
            var missing = d
            missing.id = "\(d.id):\(key)"
            missing.item.ratingKey = key
            try Offline.record(paths, missing, 45000, 180_000, false, 0.9)
        }
        try Offline.record(paths, d, 90000, 180_000, false, 0.9)
        #expect(Offline.position(paths, d) == 90000)
        try await Offline.sync(app)
        let server = try session(mock, "alice").server
        let item = try await server.metadata("100")
        #expect(item?.viewOffset == 61000, "other profile must not replay Alice's progress")
        app.setSession(try session(mock, "alice"), [])
        try await syncUntil(app, server) { $0.viewOffset == 90000 }
        let progress = try Storage.readJson(paths.config.appendingPathComponent("offline-progress.json")) {
            [Offline.Progress]()
        }
        for key in ["999998", "999999"] {
            let missing = try #require(progress.first { $0.ratingKey == key })
            #expect(!missing.pending, "missing titles must be retired from the outbox")
            #expect(missing.position == 45000, "local resume points must survive removal from Plex")
        }
        try Offline.record(paths, d, 179_000, 180_000, true, 0.9)
        try await syncUntil(app, server) { Metadata.isWatched($0) }
        #expect(Offline.position(paths, d) == 0)
        // Leave a second transfer unfinished, then stop this app's worker as if it exited.
        _ = try await Downloads.enqueue(app, ratingKey: "101")
        try await until("the second transfer to start") {
            Downloads.list(paths).contains { $0.item.ratingKey == "101" && $0.state == .downloading && $0.received > 0 }
        }
        await Downloads.stop(app)

        // restart_worker
        let again = env.app()
        again.setSession(try session(mock, "alice"), [])
        Downloads.resume(again)
        try await until("the restart to finish", timeout: .seconds(15)) {
            let s = (try? first(paths, "101").state) ?? .failed
            return s == .done || s == .failed
        }
        let resumed = try first(paths, "101")
        #expect(resumed.state == .done, "\(resumed.error ?? "")")
        #expect(try Data(contentsOf: URL(fileURLWithPath: try #require(resumed.file))) == movie)

        let ranges = mock.events().compactMap { $0["range"] }
        #expect(ranges.contains { $0.hasPrefix("bytes=") }, "resume must issue a Range request: \(ranges)")
    }

    /// The background synchronizer may own the lease; retry until the server acknowledges the change.
    func syncUntil(_ app: AppState, _ server: PlexServer, _ expected: (Metadata) -> Bool) async throws {
        let began = ContinuousClock.now
        while true {
            try await Offline.sync(app)
            if let item = try await server.metadata("100"), expected(item) { return }
            if ContinuousClock.now - began >= .seconds(10) { throw TimedOut(what: "offline progress to be replayed") }
            try await Task.sleep(for: .milliseconds(100))
        }
    }
}
