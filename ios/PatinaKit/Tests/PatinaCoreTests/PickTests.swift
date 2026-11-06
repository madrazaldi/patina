// Port of desktop/core/ops/pick.test.ts.

import PlexKit
import Testing

@testable import PatinaCore

private func movie(_ rk: String, _ minutes: Int, _ year: Int, _ genre: String, _ rating: String, _ audience: Double, _ views: Int)
    -> Metadata
{
    try! Ops.Util.metadataFromJson([
        "ratingKey": .string(rk), "type": "movie", "title": .string("Movie \(rk)"), "year": JSONValue(year),
        "duration": JSONValue(minutes * 60_000), "contentRating": .string(rating), "audienceRating": JSONValue(audience),
        "viewCount": JSONValue(views), "Genre": [["tag": .string(genre)]],
    ])
}

private func show(_ rk: String, _ leaves: Int, _ viewed: Int) -> Metadata {
    try! Ops.Util.metadataFromJson([
        "ratingKey": .string(rk), "type": "show", "title": "S", "leafCount": JSONValue(leaves),
        "viewedLeafCount": JSONValue(viewed), "duration": JSONValue(45 * 60_000),
    ])
}

@Suite struct PickTests {
    typealias Query = Ops.Pick.Query

    @Test func unwatchedShortFilmWithGenreAndRating() {
        let q = Query(maxMinutes: 100, genres: ["comedy"], contentRatings: ["PG", "PG-13"], yearMin: 2000, yearMax: 2020, minRating: 7.0)
        #expect(Ops.Pick.matches(movie("1", 95, 2010, "Comedy", "PG", 7.5, 0), q))
        #expect(!Ops.Pick.matches(movie("2", 101, 2010, "Comedy", "PG", 7.5, 0), q))  // too long
        #expect(!Ops.Pick.matches(movie("3", 95, 2010, "Drama", "PG", 7.5, 0), q))  // wrong genre
        #expect(!Ops.Pick.matches(movie("4", 95, 2010, "Comedy", "R", 7.5, 0), q))  // wrong rating
        #expect(!Ops.Pick.matches(movie("5", 95, 1999, "Comedy", "PG", 7.5, 0), q))  // too old
        #expect(!Ops.Pick.matches(movie("6", 95, 2021, "Comedy", "PG", 7.5, 0), q))  // too new
        #expect(!Ops.Pick.matches(movie("7", 95, 2010, "Comedy", "PG", 6.9, 0), q))  // rated too low
        #expect(!Ops.Pick.matches(movie("8", 95, 2010, "Comedy", "PG", 7.5, 2), q))  // already watched
    }

    @Test func filtersOffAcceptEverythingOfTheKind() {
        var q = Query(unwatched: false)
        #expect(Ops.Pick.matches(movie("1", 300, 1950, "Drama", "R", 1.0, 5), q))
        #expect(!Ops.Pick.matches(show("2", 10, 0), q))  // shows are not movies
        q.kind = .any
        #expect(Ops.Pick.matches(show("2", 10, 0), q))
    }

    @Test func aShowWithEpisodesLeftCountsAsUnwatched() {
        let q = Query(kind: .show)
        #expect(Ops.Pick.matches(show("1", 10, 0), q))
        #expect(Ops.Pick.matches(show("2", 10, 4), q))
        #expect(!Ops.Pick.matches(show("3", 10, 10), q))
        let short = Query(kind: .show, maxMinutes: 30)
        #expect(!Ops.Pick.matches(show("4", 10, 0), short))  // 45 minute episodes
    }

    @Test func itemsWithoutALengthFailALengthFilter() {
        var m = movie("1", 90, 2010, "Drama", "PG", 7.0, 0)
        m.duration = nil
        #expect(Ops.Pick.matches(m, Query()))
        #expect(!Ops.Pick.matches(m, Query(maxMinutes: 120)))
    }

    @Test func chooseReturnsTheRequestedCountWithoutRepeats() {
        let fits = (0..<20).map { movie(String($0), 90, 2010, "Drama", "PG", 7.0, 0) }
        var rng = Ops.Pick.Rng(7)
        let (items, matchCount, remaining, wrapped) = Ops.Pick.choose(fits, Query(count: 4), &rng)
        #expect(items.count == 4 && matchCount == 20 && remaining == 20 && !wrapped)
        #expect(Set(items.map(\.ratingKey)).count == 4)
    }

    @Test func rerollSkipsWhatWasShownThenStartsOver() {
        let fits = { (0..<5).map { movie(String($0), 90, 2010, "Drama", "PG", 7.0, 0) } }
        var rng = Ops.Pick.Rng(1)
        var q = Query(count: 3)
        let first = Ops.Pick.choose(fits(), q, &rng).items
        q.exclude = first.map { $0.ratingKey ?? "" }
        let (second, matchCount, remaining, wrapped) = Ops.Pick.choose(fits(), q, &rng)
        #expect(second.count == 2 && matchCount == 5 && remaining == 2 && !wrapped)
        #expect(second.allSatisfy { !q.exclude.contains($0.ratingKey ?? "") })
        q.exclude += second.map { $0.ratingKey ?? "" }
        let third = Ops.Pick.choose(fits(), q, &rng)
        #expect(third.wrapped)  // everything was shown, so it starts over
        #expect(third.items.count == 3)
    }

    @Test func chooseHandlesNothingAndDuplicates() {
        var rng = Ops.Pick.Rng()
        let (items, matchCount, remaining, wrapped) = Ops.Pick.choose([], Query(), &rng)
        #expect(items.isEmpty && matchCount == 0 && remaining == 0 && !wrapped)
        let same = movie("1", 90, 2010, "Drama", "PG", 7.0, 0)
        let again = Ops.Pick.choose([same, same], Query(), &rng)
        #expect(again.items.count == 1 && again.matches == 1)
    }

    @Test func serverFiltersCoverEveryCriterion() {
        let q = Query(maxMinutes: 100, contentRatings: ["PG", "R"], yearMin: 1990, yearMax: 1999, minRating: 6.5)
        let p = Ops.Pick.serverFilters(q, ["3", "9"], true).toQueryString()
        for part in [
            "genre=3%2C9", "contentRating=PG%2CR", "year%3E%3E=1990", "year%3C%3C=1999", "audienceRating%3E%3E=6.5",
            "duration%3C%3C=6000000",
        ] {
            #expect(p.contains(part))
        }
        #expect(Ops.Pick.serverFilters(q, [], false).toQueryString() == "contentRating=PG%2CR")
    }

    @Test func ratingsSortFromGentleToStrict() {
        let r = ["R", "Unrated", "G", "PG-13", "PG"].sorted { a, b in
            let (ra, sa) = Ops.Pick.ratingOrder(a)
            let (rb, sb) = Ops.Pick.ratingOrder(b)
            return ra != rb ? ra < rb : sa < sb
        }
        #expect(r == ["G", "PG", "PG-13", "R", "Unrated"])
    }

    @Test func genreFilterValueIds() {
        #expect(Ops.Pick.filterId(Directory(key: "12")) == "12")
        #expect(Ops.Pick.filterId(Directory(key: "/library/sections/1/all?genre=12")) == "12")
    }

    @Test func kindNamesParse() {
        #expect(Ops.Pick.parseKind("Film") == .movie)
        #expect(Ops.Pick.parseKind("tv") == .show)
        #expect(Ops.Pick.parseKind("any") == .any)
        #expect(Ops.Pick.parseKind("music") == nil)
    }
}
