// Port of desktop/test/seerr.test.ts: the Seerr commands against `MockSeerr` (tools/mock_seerr.py):
// connecting with an API key or the Plex account, discovering and requesting titles, and
// managing requests. The TypeScript tests share one mock and run in order, so here they are
// the steps of one test.

import Foundation
import PlexKit
import SeerrKit
import Testing

@testable import PatinaCore

final class OpenedUrls: PatinaCore.Host {
    var urls: [String] = []
    func openUrl(_ url: String) async throws { urls.append(url) }
    func openPath(_ path: String) async throws {}
}

/// Expect `op` to fail with a message containing `text`.
private func rejects(_ text: String, sourceLocation: SourceLocation = #_sourceLocation, _ op: () async throws -> some Any)
    async
{
    do {
        _ = try await op()
        Issue.record("expected a failure with “\(text)”", sourceLocation: sourceLocation)
    } catch {
        #expect(errorText(error).contains(text), "\(errorText(error))", sourceLocation: sourceLocation)
    }
}

@MainActor
@Suite struct SeerrIntegrationTests {
    let env = TestEnv()
    let seerr = MockSeerr(pending: true)
    let pms = MockPMS()
    let host = OpenedUrls()

    @Test func theSeerrCommandsInOrder() async throws {
        let app = env.app()
        defer {
            app.dispose()
            seerr.stop()
            pms.stop()
        }
        try await Session.signInManual(app, pms.url, "x")

        try await nothingWorksUntilSeerrIsSetUpAndABadKeyIsNotSaved(app)
        try await connectingWithAnApiKeySavesTheInstanceAndReportsAnAdmin(app)
        try await searchAndDiscoverListMoviesAndShowsWithTheirAvailability(app)
        try await requestingAMovieAndSeasonsOfAShow(app)
        try await advancedOptionsListTheServersOfTheRightKindWithProfilesAndFolders(app)
        try await requestsListWithTitlesFilterAndCanBeApprovedDeclinedAndDeleted(app)
        try await openTheSeerrPageOfATitle(app)
        try await withoutAnApiKeyPatinaSignsInWithThePlexAccount(app)
    }

    func nothingWorksUntilSeerrIsSetUpAndABadKeyIsNotSaved(_ app: AppState) async throws {
        #expect(Commands.seerrConfig(app) == nil)
        await rejects("Seerr is not set up") { try await Commands.seerrSearch(app, query: "line") }
        await rejects("did not accept the sign-in") { try await Commands.seerrConnect(app, url: seerr.url, apiKey: "wrong") }
        #expect(Commands.seerrConfig(app) == nil)
    }

    func connectingWithAnApiKeySavesTheInstanceAndReportsAnAdmin(_ app: AppState) async throws {
        let status = try await Commands.seerrConnect(app, url: "\(seerr.url)/api/v1/", apiKey: "mock-key")
        #expect(status.url == seerr.url)
        #expect(status.auth == .apiKey)
        #expect(status.version == "2.7.3")
        #expect(status.user.name == "Mock Admin")
        #expect(
            status.can
                == SeerrCan(
                    requestMovie: true, requestTv: true, request4kMovie: true, request4kTv: false, advanced: true, manage: true))
        #expect(Commands.seerrConfig(app) == SeerrConfig(url: seerr.url, auth: .apiKey))
        // The key is kept in the secret store, not the config file.
        #expect(env.secrets.get("seerr.api_key") == "mock-key")
        let file = try String(contentsOf: Config.configFile(env.paths), encoding: .utf8)
        #expect(file.contains(seerr.url))
        #expect(!file.contains("mock-key"))
        // Reconnecting without a key keeps the saved one.
        #expect(try await Commands.seerrConnect(app, url: seerr.url, apiKey: nil).auth == .apiKey)
        // The key never reaches the UI.
        #expect(!"\(Commands.settings(app))".contains("mock-key"))
    }

    func searchAndDiscoverListMoviesAndShowsWithTheirAvailability(_ app: AppState) async throws {
        let found = try await Commands.seerrSearch(app, query: "Kid's Own (Director's Cut)!")
        #expect(found.results.map(\.title) == ["Kid's Own (Director's Cut)!"])
        let trending = try await Commands.seerrDiscover(app, list: "trending", page: 1)
        #expect(trending.results.allSatisfy { $0.mediaType == .movie || $0.mediaType == .tv })
        let line = try #require(trending.results.first { $0.tmdbId == 9001 })
        #expect(line.title == "The Verdigris Line")
        #expect(line.year == 2014)
        #expect(line.status == .available)
        #expect(line.ratingKey == "100")
        await rejects("Unknown list nope.") { try await Commands.seerrDiscover(app, list: "nope") }
    }

    func requestingAMovieAndSeasonsOfAShow(_ app: AppState) async throws {
        #expect(try await Commands.seerrRequest(app, request: .init(mediaType: .movie, tmdbId: 9002)) == .approved)
        let movie = try await Commands.seerrDetails(app, mediaType: .movie, tmdbId: 9002)
        #expect(movie.status == .processing)
        #expect(movie.requests.map { [$0.state.rawValue, "\($0.mine)", $0.title] } == [["approved", "true", "Lighthouse Keepers"]])

        let show = try await Commands.seerrDetails(app, mediaType: .tv, tmdbId: 8001)
        #expect(show.tvdbId == 7001)
        // The Plex copy is offered once this server's item has the same TMDB id.
        #expect(show.ratingKey == "500")
        #expect(show.seasons.map(\.number) == [1, 2, 3])
        #expect(show.seasons.map(\.status) == [.available, .available, .unknown])
        await rejects("Choose at least one season.") {
            try await Commands.seerrRequest(app, request: .init(mediaType: .tv, tmdbId: 8001, seasons: []))
        }
        _ = try await Commands.seerrRequest(
            app,
            request: .init(
                mediaType: .tv, tmdbId: 8001, tvdbId: 7001, seasons: [3], serverId: 0, profileId: 2, rootFolder: "/tv-archive"))
        let after = try await Commands.seerrDetails(app, mediaType: .tv, tmdbId: 8001)
        #expect(after.seasons.first { $0.number == 3 }?.requested == .approved)
        let logged = try #require(seerr.events().first { $0["what"] == "request" && $0["mediaType"] == "tv" })
        #expect(logged["mediaId"] == 8001)
        #expect(logged["seasons"] == [3])
        #expect(logged["tvdbId"] == 7001)
        #expect(logged["serverId"] == 0)
        #expect(logged["profileId"] == 2)
        #expect(logged["rootFolder"] == "/tv-archive")
        // Nothing left to request is Seerr's 202, not a success.
        await rejects("No seasons available to request") {
            try await Commands.seerrRequest(app, request: .init(mediaType: .tv, tmdbId: 8001, seasons: [3]))
        }
        // Seerr's own message comes through.
        await rejects("Request for this media already exists.") {
            try await Commands.seerrRequest(app, request: .init(mediaType: .movie, tmdbId: 9002))
        }
    }

    func advancedOptionsListTheServersOfTheRightKindWithProfilesAndFolders(_ app: AppState) async throws {
        let movies4k = try await Commands.seerrRequestOptions(app, mediaType: .movie, is4k: true)
        #expect(movies4k.count == 1)
        let s = try #require(movies4k.first)
        #expect([s.id, s.activeProfileId] == [1, 2])
        #expect(s.name == "Radarr 4K")
        #expect(s.isDefault)
        #expect(s.activeDirectory == "/movies-4k")
        #expect(s.profiles.map(\.name) == ["HD-1080p", "Ultra-HD"])
        #expect(
            s.rootFolders
                == [.init(path: "/movies-4k", freeSpace: 4_000_000_000_000), .init(path: "/movies-4k-archive", freeSpace: 1_000_000_000_000)])
        #expect(try await Commands.seerrRequestOptions(app, mediaType: .tv, is4k: true).isEmpty)
    }

    func requestsListWithTitlesFilterAndCanBeApprovedDeclinedAndDeleted(_ app: AppState) async throws {
        let all = try await Commands.seerrRequests(app, filter: "all", page: 1)
        #expect(all.results.map { [$0.title, $0.state.rawValue, $0.requestedBy] } == [
            ["Station Eleven Below", "approved", "Mock Admin"],
            ["Lighthouse Keepers", "approved", "Mock Admin"],
            ["Northern Static", "pending", "Alice"],
        ])
        let pending = try await Commands.seerrRequests(app, filter: "pending", page: 1)
        #expect(pending.results.count == 1)
        let id = try #require(pending.results.first).id
        try await Commands.seerrRequestAction(app, id: id, action: "approve")
        await rejects("Only pending requests can be approved or declined.") {
            try await Commands.seerrRequestAction(app, id: id, action: "decline")
        }
        try await Commands.seerrRequestAction(app, id: id, action: "delete")
        #expect(try await Commands.seerrRequests(app, filter: "all", page: 1).results.count == 2)
        #expect(try await Commands.seerrRequests(app, filter: "all", page: 1, mine: true).results.count == 2)
        await rejects("Unknown request action frob.") { try await Commands.seerrRequestAction(app, id: id, action: "frob") }
        await rejects("Unknown filter frob.") { try await Commands.seerrRequests(app, filter: "frob") }
    }

    func openTheSeerrPageOfATitle(_ app: AppState) async throws {
        try await Commands.seerrOpen(app, host, mediaType: .tv, tmdbId: 8001)
        #expect(host.urls.last == "\(seerr.url)/tv/8001")
    }

    func withoutAnApiKeyPatinaSignsInWithThePlexAccount(_ app: AppState) async throws {
        try app.tryUpdateSettings { $0.auth.token = "plex-token" }
        let status = try await Commands.seerrConnect(app, url: seerr.url, apiKey: "")
        #expect(status.auth == .plex)
        #expect(status.user.name == "Alice")
        #expect(status.can.requestMovie && !status.can.request4kMovie && !status.can.manage)
        // Not auto-approved, and only Alice's own requests are listed.
        #expect(try await Commands.seerrRequest(app, request: .init(mediaType: .movie, tmdbId: 9004)) == .pending)
        let mine = try await Commands.seerrRequests(app, filter: "all", page: 1)
        #expect(mine.results.map(\.title) == ["Paper Satellites"])
        #expect(mine.results.map(\.mine) == [true])
        #expect(seerr.events().filter { $0["what"] == "plex-sign-in" }.count == 1)
        try Commands.seerrDisconnect(app)
        #expect(Commands.seerrConfig(app) == nil)
    }
}
