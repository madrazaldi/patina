// Port of desktop/core/ops/pick.ts.
// "Pick something to watch": a few random candidates that fit what you feel like.
//
// The server does the heavy filtering through library query parameters (unwatched, genre,
// content rating, year, rating, length, random order), so even a large library only sends one
// page per section. Everything is checked again here (`matches`), which keeps the result right
// when a server ignores a filter.

import Foundation
import PlexKit

nonisolated private let log = logger("pick")

extension Ops {
    public enum Pick {
        /// Candidates fetched per library; the random order makes each fetch a fresh sample.
        static let PER_SECTION = 120
        /// Used when the server refuses the numeric filters and everything is checked here.
        static let FALLBACK_PER_SECTION = 400

        public nonisolated enum Kind: String, Sendable, Equatable {
            case movie, show, any
        }

        /// The media type a section of `sectionKind` is searched as for this kind of pick.
        private nonisolated static func mediaTypeOf(_ kind: Kind, _ sectionKind: String) -> Kind? {
            if (kind == .movie || kind == .any) && sectionKind == "movie" { return .movie }
            if (kind == .show || kind == .any) && sectionKind == "show" { return .show }
            return nil
        }

        /// `PickKind::parse`.
        public nonisolated static func parseKind(_ s: String) -> Kind? {
            switch asciiLower(s) {
            case "movie", "movies", "film", "films": .movie
            case "show", "shows", "series", "tv": .show
            case "any", "all": .any
            default: nil
            }
        }

        /// What to watch. The defaults are an unwatched film, four candidates.
        public nonisolated struct Query: Sendable, Equatable {
            public var kind: Kind = .movie
            /// Only things with something left to watch.
            public var unwatched = true
            /// Longest running time in minutes (a film, or one episode of a show).
            public var maxMinutes: Double?
            public var minMinutes: Double?
            /// Any of these genres (by name).
            public var genres: [String] = []
            /// Any of these content ratings ("PG-13", "TV-MA", ...).
            public var contentRatings: [String] = []
            /// Library section ids; empty means every movie and show library.
            public var sections: [String] = []
            public var yearMin: Int?
            public var yearMax: Int?
            /// Lowest audience rating, 0 to 10.
            public var minRating: Double?
            /// How many candidates to return.
            public var count = 4
            /// Rating keys shown before, so a reroll offers something new.
            public var exclude: [String] = []

            public init(
                kind: Kind = .movie, unwatched: Bool = true, maxMinutes: Double? = nil, minMinutes: Double? = nil,
                genres: [String] = [], contentRatings: [String] = [], sections: [String] = [], yearMin: Int? = nil,
                yearMax: Int? = nil, minRating: Double? = nil, count: Int = 4, exclude: [String] = []
            ) {
                self.kind = kind
                self.unwatched = unwatched
                self.maxMinutes = maxMinutes
                self.minMinutes = minMinutes
                self.genres = genres
                self.contentRatings = contentRatings
                self.sections = sections
                self.yearMin = yearMin
                self.yearMax = yearMax
                self.minRating = minRating
                self.count = count
                self.exclude = exclude
            }
        }

        private nonisolated static func queryCount(_ q: Query) -> Int {
            min(max(q.count, 1), 12)
        }

        public nonisolated struct Result: Sendable, Equatable {
            public var items: [Metadata]
            /// How many titles fit the query in the libraries searched (a lower bound when a
            /// library has more than one sample page of them).
            public var matches: Int
            /// How many of those have not been offered yet.
            public var remaining: Int
            /// Everything that fits was already offered, so the choice started over.
            public var wrapped: Bool
            /// Names of the libraries that were searched.
            public var libraries: [String]
        }

        public nonisolated struct Section: Sendable, Equatable {
            public var id: String
            public var title: String
            /// `movie` or `show`.
            public var kind: String
        }

        /// The choices the form offers.
        public nonisolated struct Options: Sendable, Equatable {
            public var sections: [Section]
            public var genres: [String]
            public var contentRatings: [String]
        }

        private nonisolated static func isPickable(_ d: Directory) -> Bool {
            d.type == "movie" || d.type == "show"
        }

        /// Libraries, genres and content ratings to choose from.
        public static func options(_ app: AppState) async throws -> Options {
            let server = try app.server()
            var sections: [Section] = []
            for d in app.sections().filter(isPickable) {
                if let key = d.key, let type = d.type { sections.append(Section(id: key, title: Directory.title(d), kind: type)) }
            }
            let lists = try await Util.mapLimit(sections, max(sections.count, 1)) { s, _ in
                let kind = s.kind == "show" ? 2 : 1
                async let genres = (try? server.sectionFilterValues(s.id, filter: "genre", kind: kind)) ?? []
                async let ratings = (try? server.sectionFilterValues(s.id, filter: "contentRating", kind: kind)) ?? []
                return (await genres, await ratings)
            }
            var genres: [String] = []
            var ratings: [String] = []
            for (g, r) in lists {
                for d in g { pushUnique(&genres, d.title ?? d.tag) }
                for d in r { pushUnique(&ratings, d.title ?? d.tag ?? d.key) }
            }
            genres.sort { $0.lowercased() < $1.lowercased() }
            ratings.sort { a, b in
                let (ra, sa) = ratingOrder(a)
                let (rb, sb) = ratingOrder(b)
                return ra != rb ? ra < rb : sa < sb
            }
            return Options(sections: sections, genres: genres, contentRatings: ratings)
        }

        /// `to_ascii_lowercase`: only ASCII letters fold.
        nonisolated static func asciiLower(_ s: String) -> String {
            String(String.UnicodeScalarView(s.unicodeScalars.map { ("A"..."Z").contains($0) ? Unicode.Scalar($0.value + 32)! : $0 }))
        }

        private nonisolated static func asciiUpper(_ s: String) -> String {
            String(String.UnicodeScalarView(s.unicodeScalars.map { ("a"..."z").contains($0) ? Unicode.Scalar($0.value - 32)! : $0 }))
        }

        private nonisolated static func pushUnique(_ list: inout [String], _ value: String?) {
            guard let value, !value.isEmpty, !list.contains(where: { asciiLower($0) == asciiLower(value) }) else { return }
            list.append(value)
        }

        /// Orders ratings roughly from gentlest to strictest (G, PG, PG-13, R, TV-*, others).
        public nonisolated static func ratingOrder(_ r: String) -> (Int, String) {
            let rank =
                switch asciiUpper(r) {
                case "G", "TV-Y", "TV-G": 0
                case "PG", "TV-PG", "TV-Y7": 1
                case "PG-13", "TV-14": 2
                case "R", "TV-MA": 3
                case "NC-17": 4
                default: 5
                }
            return (rank, r.lowercased())
        }

        /// Request parameters narrowing a section's listing on the server.
        public nonisolated static func serverFilters(_ q: Query, _ genreIds: [String], _ numeric: Bool) -> Params {
            var p = Params()
            if !genreIds.isEmpty { p.push("genre", genreIds.joined(separator: ",")) }
            if !q.contentRatings.isEmpty { p.push("contentRating", q.contentRatings.joined(separator: ",")) }
            if numeric {
                // `key>>=value` means at least, `key<<=value` at most.
                if let v = q.yearMin { p.push("year>>", v) }
                if let v = q.yearMax { p.push("year<<", v) }
                if let v = q.minRating { p.push("audienceRating>>", v) }
                if let v = q.maxMinutes { p.push("duration<<", v * 60_000) }
                if let v = q.minMinutes, v > 0 { p.push("duration>>", v * 60_000) }
            }
            return p
        }

        /// The id Plex filters a genre by, from a `genre` filter value entry.
        public nonisolated static func filterId(_ d: Directory) -> String? {
            guard let key = d.key else { return nil }
            guard let at = key.range(of: "genre=", options: .backwards) else { return key }
            return String(key[at.upperBound...])
        }

        /// Genre filter ids for the genres named in the query, if this section has any of them.
        private static func genreIds(_ server: PlexServer, _ section: String, _ kind: Int, _ wanted: [String]) async -> [String] {
            let values = (try? await server.sectionFilterValues(section, filter: "genre", kind: kind)) ?? []
            var out: [String] = []
            for d in values {
                let name = d.title ?? d.tag ?? ""
                if !wanted.contains(where: { asciiLower($0) == asciiLower(name) }) { continue }
                if let id = filterId(d) { out.append(id) }
            }
            return out
        }

        /// One library's candidates and how many titles fit in all.
        private static func candidates(_ server: PlexServer, _ section: Directory, _ q: Query) async throws -> ([Metadata], Int) {
            guard let id = section.key else { throw CoreError("library without an id") }
            guard let type = mediaTypeOf(q.kind, section.type ?? "") else { return ([], 0) }
            let kind = type == .show ? 2 : 1
            let ids = q.genres.isEmpty ? [] : await genreIds(server, id, kind, q.genres)
            if !q.genres.isEmpty && ids.isEmpty {
                // No genre of this library is among the wanted ones.
                return ([], 0)
            }
            func fetch(_ numeric: Bool, _ size: Int) async throws -> MediaContainer {
                try await server.sectionItems(
                    id,
                    query: LibraryQuery(
                        kind: kind, sort: "random", unwatched: q.unwatched, filters: serverFilters(q, ids, numeric),
                        start: 0, size: size))
            }
            let mc: MediaContainer
            do {
                mc = try await fetch(true, PER_SECTION)
            } catch {
                // A server that rejects the numeric filters still gets a (larger) unfiltered sample.
                log.debug("pick filters refused by library \(id): \(errorText(error))")
                mc = try await fetch(false, FALLBACK_PER_SECTION)
            }
            let total = mc.totalSize.map { max($0, 0) } ?? mc.metadata.count
            return (mc.metadata, total)
        }

        private nonisolated static func hasUnwatched(_ item: Metadata) -> Bool {
            if item.type == "show" { return (item.leafCount ?? 1) > (item.viewedLeafCount ?? 0) }
            return (item.viewCount ?? 0) == 0
        }

        /// Whether an item fits the query. The same checks the server was asked to make.
        public nonisolated static func matches(_ item: Metadata, _ q: Query) -> Bool {
            let kind = item.type ?? ""
            let kindOk =
                (q.kind == .movie && kind == "movie") || (q.kind == .show && kind == "show")
                || (q.kind == .any && (kind == "movie" || kind == "show"))
            if !kindOk { return false }
            if q.unwatched && !hasUnwatched(item) { return false }
            if q.maxMinutes != nil || q.minMinutes != nil {
                guard let duration = item.duration, duration > 0 else { return false }
                let minutes = Double(duration) / 60_000
                if let max = q.maxMinutes, minutes > max { return false }
                if let min = q.minMinutes, minutes < min { return false }
            }
            if !q.genres.isEmpty
                && !item.genre.contains(where: { g in q.genres.contains { asciiLower($0) == asciiLower(g.tag ?? "") } })
            {
                return false
            }
            if !q.contentRatings.isEmpty {
                let rating = item.contentRating ?? ""
                if !q.contentRatings.contains(where: { asciiLower($0) == asciiLower(rating) }) { return false }
            }
            if q.yearMin != nil || q.yearMax != nil {
                guard let year = item.year else { return false }
                if let min = q.yearMin, year < min { return false }
                if let max = q.yearMax, year > max { return false }
            }
            if let min = q.minRating {
                guard let r = item.audienceRating ?? item.rating, r >= min else { return false }
            }
            return true
        }

        /// A small seeded generator (the `fastrand::Rng` of the Rust code); unseeded ones start
        /// from the system's generator.
        public nonisolated struct Rng: Sendable {
            private var state: UInt32

            public init(_ seed: UInt32? = nil) {
                state = seed ?? UInt32.random(in: .min ... .max)
            }

            /// mulberry32: a float in `0..1`.
            public mutating func next() -> Double {
                state = state &+ 0x6d2b_79f5
                var t = state
                t = (t ^ (t >> 15)) &* (t | 1)
                t ^= t &+ ((t ^ (t >> 7)) &* (t | 61))
                return Double(t ^ (t >> 14)) / 4_294_967_296
            }

            /// Uniform in `0..n`.
            public mutating func below(_ n: Int) -> Int {
                Int(next() * Double(n))
            }

            /// Fisher-Yates, in place.
            public mutating func shuffle<T>(_ list: inout [T]) {
                guard list.count > 1 else { return }
                for i in stride(from: list.count - 1, to: 0, by: -1) {
                    list.swapAt(i, below(i + 1))
                }
            }
        }

        /// Choose from the candidates that fit: not offered before, in random order. When
        /// everything that fits was offered already, the choice starts over (`wrapped`).
        /// Returns the chosen items, how many fit, how many of those are still unseen, and
        /// whether it wrapped.
        public nonisolated static func choose(_ fits: [Metadata], _ q: Query, _ rng: inout Rng)
            -> (items: [Metadata], matches: Int, remaining: Int, wrapped: Bool)
        {
            var seen = Set<String>()
            let fits = fits.filter { seen.insert($0.ratingKey ?? "").inserted }
            let matchCount = fits.count
            let excluded = Set(q.exclude)
            var fresh: [Metadata] = []
            var stale: [Metadata] = []
            for m in fits {
                if excluded.contains(m.ratingKey ?? "") { stale.append(m) } else { fresh.append(m) }
            }
            let wrapped = fresh.isEmpty && !stale.isEmpty
            let remaining = fresh.count
            var pool = wrapped ? stale : fresh
            rng.shuffle(&pool)
            return (Array(pool.prefix(queryCount(q))), matchCount, remaining, wrapped)
        }

        /// Pick a few things to watch.
        public static func pick(_ app: AppState, _ q: Query) async throws -> Result {
            let server = try app.server()
            let sections = app.sections()
                .filter(isPickable)
                .filter { d in q.sections.isEmpty || d.key.map(q.sections.contains) == true }
                .filter { mediaTypeOf(q.kind, $0.type ?? "") != nil }
            if sections.isEmpty { throw CoreError("There is no movie or show library to pick from.") }
            let fetched = try await Util.mapLimit(sections, sections.count) { s, _ in
                do {
                    return Swift.Result<([Metadata], Int), any Error>.success(try await candidates(server, s, q))
                } catch {
                    return .failure(error)
                }
            }
            var fits: [Metadata] = []
            var total = 0
            var libraries: [String] = []
            var failure: (any Error)?
            for (section, result) in zip(sections, fetched) {
                switch result {
                case .success(let (items, count)):
                    libraries.append(Directory.title(section))
                    // The server's count covers the whole library; the sample is checked here.
                    let kept = items.filter { matches($0, q) }
                    total += kept.isEmpty ? 0 : max(count, kept.count)
                    fits += kept
                case .failure(let e):
                    failure = e
                }
            }
            if libraries.isEmpty { throw failure ?? CoreError("Nothing could be searched.") }
            var rng = Rng()
            let (items, fit, fresh, wrapped) = choose(fits, q, &rng)
            // A library with more matches than one sample page reports its real count.
            let matchCount = max(total, fit)
            let remaining = wrapped ? matchCount : max(matchCount - (fit - fresh), 0)
            return Result(items: items, matches: matchCount, remaining: remaining, wrapped: wrapped, libraries: libraries)
        }
    }
}
