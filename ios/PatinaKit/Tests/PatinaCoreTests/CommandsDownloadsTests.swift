// Port of the download and smart-download cases of desktop/test/commands.test.ts, and a
// round trip through the smart-download commands against MockPMS.

import Foundation
import PlexKit
import Testing

@testable import PatinaCore

@Suite struct CommandsDownloadsTests {
    func connected(_ mock: MockPMS) throws -> (TestEnv, AppState) {
        let env = TestEnv()
        let app = env.app()
        app.setSession(
            Session(
                tv: nil,
                server: try PlexServer(info: testInfo, baseUrl: mock.url, token: "alice", session: MockPMS.session),
                serverName: mock.name, serverId: mock.machine, user: nil, owned: true, servers: []), [])
        return (env, app)
    }

    @Test func readsWithoutDownloads() throws {
        let mock = MockPMS()
        defer { mock.stop() }
        let (_, app) = try connected(mock)
        #expect(Commands.downloads(app).isEmpty)
        let overview = Commands.smartDownloads(app)
        #expect(overview.policies.isEmpty && overview.budget == nil && overview.used == 0)
    }

    @Test func smartDownloadPoliciesRoundTrip() async throws {
        let mock = MockPMS()
        mock.serve(MediaFile(Data(repeating: 1, count: 1024)))
        defer { mock.stop() }
        let (env, app) = try connected(mock)
        let policy = try await Commands.setSmartDownload(app, ratingKey: "500", keep: 2, deleteWatched: true)
        #expect(policy.target == .show && policy.keep == 2)
        await #expect(throws: CoreError.self) {
            try await Commands.setSmartDownload(app, ratingKey: "500", keep: 0, deleteWatched: true)
        }

        let report = try await Commands.runSmartDownloads(app)
        #expect(report.added == 2 && report.errors.isEmpty)
        #expect(Commands.downloads(app).filter { $0.policy == policy.id }.count == 2)
        try await until("the policy's downloads to finish") {
            Commands.downloads(app).allSatisfy { $0.state == .done }
        }
        var overview = Commands.smartDownloads(app)
        #expect(overview.policies.map(\.id) == [policy.id])
        #expect(overview.policies[0].active && overview.policies[0].downloads == 2 && overview.used == 2048)

        try Commands.setDownloadBudget(app, bytes: 4096)
        try Commands.updateSmartDownload(app, id: policy.id, patch: Smart.Patch(enabled: false))
        overview = Commands.smartDownloads(app)
        #expect(overview.budget == 4096 && overview.policies[0].enabled == false)
        // smart-downloads.json keeps the TypeScript format.
        let saved = try JSONValue.parse(Data(contentsOf: env.paths.config.appendingPathComponent("smart-downloads.json")))
        #expect(saved["budget"] == 4096)
        #expect(saved["policies"]?[0]?["deleteWatched"] == true)

        try Commands.removeSmartDownload(app, id: policy.id, deleteDownloads: true)
        #expect(Commands.smartDownloads(app).policies.isEmpty)
        #expect(Commands.downloads(app).isEmpty)
    }
}
