// Port of desktop/core/seerr.test.ts.

import Foundation
import PlexKit
import SeerrKit
import Testing

@testable import PatinaCore

@Suite struct SeerrTests {
    @Test func permissionsFollowSeerrAdminsMayDoEverything() {
        #expect(Seerr.hasAny(2, 16))
        #expect(!Seerr.hasAny(32, 16))
        let user = Seerr.capabilities(32, movie4k: true, tv4k: false)
        #expect(
            user
                == SeerrCan(
                    requestMovie: true, requestTv: true, request4kMovie: false, request4kTv: false, advanced: false,
                    manage: false))
        let movies = Seerr.capabilities(262144 | 2048, movie4k: true, tv4k: true)
        #expect([movies.requestMovie, movies.requestTv, movies.request4kMovie, movies.request4kTv] == [true, false, true, false])
        // 4K needs a 4K server too.
        #expect(
            Seerr.capabilities(2, movie4k: false, tv4k: true)
                == SeerrCan(
                    requestMovie: true, requestTv: true, request4kMovie: false, request4kTv: true, advanced: true,
                    manage: true))
    }

    @Test func statusesMapToNames() {
        #expect(Seerr.availability(nil) == nil)
        #expect(
            [1, 2, 3, 4, 5, 6, 7].map(Seerr.availability)
                == [.unknown, .pending, .processing, .partial, .available, .blocklisted, .deleted])
        #expect([1, 2, 3, 4, 5].map(Seerr.requestState) == [.pending, .approved, .declined, .failed, .completed])
    }

    @Test func resultsKeepMoviesAndShowsWithTheirArtworkAndAvailability() {
        let titles = Seerr.titlesFrom([
            RawResult(
                id: 1, mediaType: "movie", title: "A", releaseDate: "2021-02-03", posterPath: "/a.jpg",
                mediaInfo: RawMediaInfo(status: 5, ratingKey: "100")),
            RawResult(id: 2, mediaType: "tv", name: "B", firstAirDate: "", posterPath: "http://127.0.0.1/b.jpg"),
            RawResult(id: 3, mediaType: "person", name: "C"),
            RawResult(id: 4, mediaType: "movie", title: "D", mediaInfo: RawMediaInfo(status: 1, status4k: 5, ratingKey: nil, ratingKey4k: "42")),
        ])
        #expect(titles.map(\.mediaType) == [.movie, .tv, .movie])
        #expect(titles.map(\.title) == ["A", "B", "D"])
        #expect(titles.map(\.year) == [2021, nil, nil])
        #expect(titles.map(\.poster) == ["https://image.tmdb.org/t/p/w500/a.jpg", "http://127.0.0.1/b.jpg", nil])
        #expect(titles.map(\.status) == [.available, nil, .unknown])
        #expect(titles.map(\.ratingKey) == ["100", nil, "42"])
        #expect(Seerr.tmdbImage(nil, "w500") == nil)
    }

    @Test func seasonsShowWhatIsAvailableAndWhatIsRequestedWithoutSpecials() {
        let seasons = Seerr.seasonsFrom(
            RawTvDetails(
                id: 8,
                mediaInfo: RawMediaInfo(
                    status: 4,
                    requests: [
                        RawMediaRequest(id: 1, status: 1, createdAt: "2024-01-01", seasons: [.init(seasonNumber: 2)]),
                        RawMediaRequest(id: 2, status: 2, createdAt: "2024-02-01", seasons: [.init(seasonNumber: 2)]),
                        RawMediaRequest(id: 3, status: 3, createdAt: "2024-03-01", seasons: [.init(seasonNumber: 3)]),
                        RawMediaRequest(id: 4, status: 1, is4k: true, seasons: [.init(seasonNumber: 3)]),
                    ],
                    seasons: [.init(seasonNumber: 1, status: 5)]),
                seasons: [.init(seasonNumber: 0), .init(seasonNumber: 1, episodeCount: 3), .init(seasonNumber: 2), .init(seasonNumber: 3)]))
        #expect(seasons.map(\.number) == [1, 2, 3])
        #expect(seasons.map(\.name) == ["Season 1", "Season 2", "Season 3"])
        #expect(seasons.map(\.episodes) == [3, nil, nil])
        #expect(seasons.map(\.status) == [.available, nil, nil])
        #expect(seasons.map(\.requested) == [nil, .approved, nil])
        #expect(seasons.map(\.requested4k) == [nil, nil, .pending])
    }

    @Test func aPlexItemIsTheSeerrTitleOnlyWhenItsTmdbIdOrWithoutIdsItsTitleAgrees() throws {
        func item(_ o: [String: JSONValue]) throws -> Metadata {
            var base: [String: JSONValue] = ["ratingKey": "100", "type": "movie", "title": "Heat"]
            base.merge(o) { $1 }
            return try Metadata(json: .object(base))
        }
        #expect(Seerr.isTmdbTitle(try item(["Guid": [["id": "imdb://tt1"], ["id": "tmdb://949"]]]), .movie, 949, "Heat"))
        // Another server's rating key can name an unrelated title.
        #expect(!Seerr.isTmdbTitle(try item(["title": "Up", "Guid": [["id": "tmdb://14160"]]]), .movie, 949, "Heat"))
        #expect(Seerr.isTmdbTitle(try item(["guid": "com.plexapp.agents.themoviedb://949?lang=en"]), .movie, 949, "Heat"))
        #expect(Seerr.isTmdbTitle(try item([:]), .movie, 949, "heat"))
        #expect(!Seerr.isTmdbTitle(try item([:]), .tv, 949, "Heat"))
    }

    @Test func thePlexCopyIsCheckedButAServerThatDoesntAnswerDoesntHoldUpThePage() async throws {
        let movie = Seerr.titlesFrom([
            RawResult(id: 949, mediaType: "movie", title: "Heat", mediaInfo: RawMediaInfo(status: 5, ratingKey: "100"))
        ])[0]
        let found = try Metadata(json: ["ratingKey": "100", "type": "movie", "title": "Heat", "Guid": [["id": "tmdb://949"]]])
        #expect(await Seerr.plexCopy({ _ in found }, movie) == "100")
        #expect(await Seerr.plexCopy({ _ in nil }, movie) == nil)
        #expect(await Seerr.plexCopy({ _ in throw CoreError("offline") }, movie) == nil)
        let started = ContinuousClock.now
        let never: @MainActor @Sendable (String) async throws -> Metadata? = { _ in
            try await Task.sleep(for: .seconds(3600))
            return nil
        }
        #expect(await Seerr.plexCopy(never, movie, wait: .milliseconds(30)) == nil)
        #expect(ContinuousClock.now - started < .seconds(1))
    }
}
