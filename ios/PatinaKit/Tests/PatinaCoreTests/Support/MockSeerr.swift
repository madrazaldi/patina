// Port of tools/mock_seerr.py: a fake Seerr answering inside the process, on a `MockPMS`
// address (so `MockPMS.session` routes to it). Sign in with API key `mock-key` (an admin),
// or with any Plex token (Alice, who may request without auto-approval). `events()` is the
// log of what changed (what the script prints), with the request bodies' fields.
//
//     let seerr = MockSeerr(pending: true)
//     try await Seerr.connect(app, seerr.url, "mock-key")

import Foundation
import PlexKit
import Synchronization

nonisolated final class MockSeerr: Sendable {
    static let API_KEY = "mock-key"
    static let ADMIN = 2
    static let REQUEST = 32
    static let USERS: [Int: Obj] = [
        1: ["id": 1, "email": "admin@example.com", "displayName": "Mock Admin", "permissions": 2, "userType": 1],
        2: [
            "id": 2, "email": "alice@example.com", "displayName": "Alice", "plexUsername": "alice", "permissions": 32,
            "userType": 1,
        ],
    ]
    static let OVERVIEW = "A mock-server title used to exercise Patina's Seerr pages."

    struct Movie {
        var title: String
        var releaseDate: String
        var poster: Int
        var status: Int?
        var ratingKey: String?
    }

    struct Show {
        var name: String
        var firstAirDate: String
        var poster: Int
        /// Season number and episode count, in order.
        var seasons: [(Int, Int)]
        var available: [Int]
        var ratingKey: String?
        var tvdbId: Int
    }

    /// tmdb id -> movie; "The Verdigris Line" is mock_pms.py's movie 100.
    static let MOVIES: [(Int, Movie)] = [
        (9001, Movie(title: "The Verdigris Line", releaseDate: "2014-05-01", poster: 0, status: 5, ratingKey: "100")),
        (9002, Movie(title: "Lighthouse Keepers", releaseDate: "2021-09-17", poster: 1)),
        (9003, Movie(title: "Northern Static", releaseDate: "2023-02-03", poster: 2)),
        (9004, Movie(title: "Paper Satellites", releaseDate: "2024-11-08", poster: 3)),
        (9005, Movie(title: "Long Way Down River", releaseDate: "2019-04-12", poster: 4)),
        (9006, Movie(title: "Kid's Own (Director's Cut)!", releaseDate: "2012-06-30", poster: 5)),
    ]
    static let SHOWS: [(Int, Show)] = [
        (
            8001,
            Show(
                name: "Station Eleven Below", firstAirDate: "2019-01-10", poster: 6, seasons: [(1, 3), (2, 3), (3, 8)],
                available: [1, 2], ratingKey: "500", tvdbId: 7001)
        ),
        (
            8002,
            Show(
                name: "Ferry Season", firstAirDate: "2022-03-04", poster: 7, seasons: [(1, 10), (2, 10), (3, 6)], available: [],
                tvdbId: 7002)
        ),
        (8003, Show(name: "Glass Weather", firstAirDate: "2025-08-21", poster: 2, seasons: [(1, 8)], available: [], tvdbId: 7003)),
    ]
    static let SERVICES: [String: [Obj]] = [
        "radarr": [
            ["id": 0, "name": "Radarr", "is4k": false, "isDefault": true, "activeProfileId": 1, "activeDirectory": "/movies"],
            ["id": 1, "name": "Radarr 4K", "is4k": true, "isDefault": true, "activeProfileId": 2, "activeDirectory": "/movies-4k"],
        ],
        "sonarr": [["id": 0, "name": "Sonarr", "is4k": false, "isDefault": true, "activeProfileId": 1, "activeDirectory": "/tv"]],
    ]

    struct State {
        var requests: [Obj] = []
        var nextId = 1
        /// connect.sid value -> user id.
        var sessions: [String: Int] = [:]
        var log: [Obj] = []
    }

    let pms: MockPMS
    let state = Mutex(State())
    var url: String { pms.url }

    /// `pending`: start with a pending request by Alice for Northern Static (`--pending`).
    init(pending: Bool = false) {
        pms = MockPMS(name: "Seerr", machine: "mock-seerr")
        if pending {
            state.withLock { s in
                s.requests.append([
                    "id": 1, "status": 1, "type": "movie", "is4k": false, "createdAt": .string(Self.now()),
                    "updatedAt": .string(Self.now()), "requestedBy": .object(Self.USERS[2]!), "modifiedBy": nil,
                    "media": ["tmdbId": 9003], "seasons": [],
                ])
                s.nextId = 2
            }
        }
        pms.on { [self] _, req in route(req) }
    }

    func stop() {
        pms.stop()
    }

    func events() -> [Obj] {
        state.withLock { $0.log }
    }

    static func now() -> String {
        ISO8601DateFormatter().string(from: Date())
    }

    static func movie(_ id: Int) -> Movie? { MOVIES.first { $0.0 == id }?.1 }
    static func show(_ id: Int) -> Show? { SHOWS.first { $0.0 == id }?.1 }

    // MARK: Media

    private static func requestsFor(_ s: State, _ kind: String, _ tmdb: Int) -> [Obj] {
        s.requests.filter { $0["type"]?.stringValue == kind && $0["media"]?["tmdbId"]?.intValue == tmdb }
    }

    /// MediaStatus of a title: available on the server, else from its open requests.
    private static func mediaStatus(_ s: State, _ kind: String, _ tmdb: Int, _ is4k: Bool = false) -> Int? {
        if !is4k {
            if kind == "movie", let status = movie(tmdb)?.status { return status }
            if kind == "tv", let sh = show(tmdb), !sh.available.isEmpty {
                return sh.available.count == sh.seasons.count ? 5 : 4
            }
        }
        let open = requestsFor(s, kind, tmdb).filter {
            $0["is4k"]?.boolValue == is4k && [1, 2].contains($0["status"]?.intValue ?? 0)
        }
        if open.contains(where: { $0["status"]?.intValue == 2 }) { return 3 }
        return open.isEmpty ? nil : 2
    }

    private static func mediaInfo(_ s: State, _ kind: String, _ tmdb: Int, _ withRequests: Bool = true) -> Obj? {
        let (status, status4k) = (mediaStatus(s, kind, tmdb), mediaStatus(s, kind, tmdb, true))
        let reqs = requestsFor(s, kind, tmdb)
        if status == nil && status4k == nil && reqs.isEmpty { return nil }
        var info: Obj = [
            "id": JSONValue(tmdb), "tmdbId": JSONValue(tmdb), "mediaType": .string(kind), "status": JSONValue(status ?? 1),
            "status4k": JSONValue(status4k ?? 1),
        ]
        let ratingKey = kind == "movie" ? movie(tmdb)?.ratingKey : show(tmdb)?.ratingKey
        if let ratingKey, status == 4 || status == 5 { info["ratingKey"] = .string(ratingKey) }
        if kind == "tv", let sh = show(tmdb) {
            info["tvdbId"] = JSONValue(sh.tvdbId)
            info["seasons"] = .array(
                sh.seasons.map { n, _ in
                    ["seasonNumber": JSONValue(n), "status": sh.available.contains(n) ? 5 : 1, "status4k": 1]
                })
        }
        if withRequests {
            info["requests"] = .array(
                reqs.map {
                    var r = $0
                    r["media"] = ["id": JSONValue(tmdb), "tmdbId": JSONValue(tmdb)]
                    return .object(r)
                })
        }
        return info
    }

    private static func poster(_ n: Int) -> JSONValue { .string("/mock\(n).jpg") }

    private static func movieResult(_ s: State, _ tmdb: Int) -> Obj {
        let m = movie(tmdb)!
        var out: Obj = [
            "id": JSONValue(tmdb), "mediaType": "movie", "title": .string(m.title), "releaseDate": .string(m.releaseDate),
            "overview": .string(OVERVIEW), "posterPath": poster(m.poster), "backdropPath": poster(m.poster), "voteAverage": 7.2,
        ]
        if let info = mediaInfo(s, "movie", tmdb, false) { out["mediaInfo"] = .object(info) }
        return out
    }

    private static func tvResult(_ s: State, _ tmdb: Int) -> Obj {
        let sh = show(tmdb)!
        var out: Obj = [
            "id": JSONValue(tmdb), "mediaType": "tv", "name": .string(sh.name), "firstAirDate": .string(sh.firstAirDate),
            "overview": .string(OVERVIEW), "posterPath": poster(sh.poster), "backdropPath": poster(sh.poster), "voteAverage": 8.1,
        ]
        if let info = mediaInfo(s, "tv", tmdb, false) { out["mediaInfo"] = .object(info) }
        return out
    }

    private static func allResults(_ s: State) -> [Obj] {
        MOVIES.map { movieResult(s, $0.0) } + SHOWS.map { tvResult(s, $0.0) } + [
            ["id": 1, "mediaType": "person", "name": "Ada Lindqvist", "knownFor": []]
        ]
    }

    private static func pageOf(_ results: [Obj], _ req: MockRequest) -> JSONValue {
        let page = max(1, Int(req.q("page") ?? "1") ?? 1)
        let size = 20
        return [
            "page": JSONValue(page), "totalPages": JSONValue(max(1, (results.count + size - 1) / size)),
            "totalResults": JSONValue(results.count),
            "results": .array(results.dropFirst((page - 1) * size).prefix(size).map(JSONValue.object)),
        ]
    }

    // MARK: Routes

    private static func message(_ text: String, _ status: Int) -> MockResponse {
        .json(["message": .string(text)], status: status)
    }

    private func record(_ s: inout State, _ what: String, _ fields: Obj = [:]) {
        var entry = fields
        entry["what"] = .string(what)
        s.log.append(entry)
    }

    private func route(_ req: MockRequest) -> MockResponse {
        guard req.path.hasPrefix("/api/v1/") else { return Self.message("Not found", 404) }
        let p = String(req.path.dropFirst("/api/v1".count))
        // Seerr's query validation rejects these characters unless percent-encoded.
        if req.rawQuery.contains(where: { " !'()*".contains($0) }) {
            return Self.message("request/query/query must be url encoded", 400)
        }
        if p == "/status" { return .json(["version": "2.7.3", "commitTag": "mock"]) }
        let body = (try? JSONValue.parse(req.body)) ?? [:]
        return state.withLock { s in
            if p == "/auth/plex" && req.method == "POST" {
                guard let token = body["authToken"]?.stringValue, !token.isEmpty, token != "bad" else {
                    return Self.message("Unable to authenticate.", 403)
                }
                let sid = "mock-\(s.sessions.count + 1)"
                s.sessions[sid] = 2
                record(&s, "plex-sign-in")
                var r = MockResponse.json(.object(Self.USERS[2]!))
                r.headers["Set-Cookie"] = "connect.sid=\(sid); Path=/; HttpOnly"
                return r
            }
            guard let user = user(s, req) else {
                return Self.message("You do not have permission to access this endpoint.", 403)
            }
            let manage = (user["permissions"]?.intValue ?? 0) & Self.ADMIN != 0
            return authed(&s, req, p, body, user, manage)
        }
    }

    private func user(_ s: State, _ req: MockRequest) -> Obj? {
        func header(_ name: String) -> String? {
            req.headers.first { $0.key.caseInsensitiveCompare(name) == .orderedSame }?.value
        }
        if header("X-Api-Key") == Self.API_KEY { return Self.USERS[1] }
        if let cookie = header("Cookie"), let m = cookie.firstMatch(of: #/connect\.sid=([^;]+)/#),
            let id = s.sessions[String(m.1)]
        {
            return Self.USERS[id]
        }
        return nil
    }

    private func authed(_ s: inout State, _ req: MockRequest, _ p: String, _ body: JSONValue, _ user: Obj, _ manage: Bool)
        -> MockResponse
    {
        let method = req.method
        if p == "/auth/me" { return .json(.object(user)) }
        if p == "/search" {
            let text = (req.q("query") ?? "").lowercased()
            let found = Self.allResults(s).filter {
                ($0["title"]?.stringValue ?? $0["name"]?.stringValue ?? "").lowercased().contains(text)
            }
            return .json(Self.pageOf(found, req))
        }
        if p == "/discover/trending" { return .json(Self.pageOf(Self.allResults(s), req)) }
        if p == "/discover/movies" || p == "/discover/movies/upcoming" {
            return .json(Self.pageOf(Self.MOVIES.map { Self.movieResult(s, $0.0) }, req))
        }
        if p == "/discover/tv" || p == "/discover/tv/upcoming" {
            return .json(Self.pageOf(Self.SHOWS.map { Self.tvResult(s, $0.0) }, req))
        }
        if let m = p.wholeMatch(of: #/\/(movie|tv)\/(\d+)/#) {
            let (kind, tmdb) = (String(m.1), Int(m.2)!)
            var out: Obj
            if kind == "movie" {
                guard Self.movie(tmdb) != nil else { return Self.message("Unable to retrieve details.", 404) }
                out = Self.movieResult(s, tmdb)
                out["runtime"] = 112
                out["tagline"] = "Every signal leaves a trace."
                out["genres"] = [["id": 18, "name": "Drama"]]
            } else {
                guard let sh = Self.show(tmdb) else { return Self.message("Unable to retrieve details.", 404) }
                out = Self.tvResult(s, tmdb)
                out["episodeRunTime"] = [45]
                out["genres"] = [["id": 9648, "name": "Mystery"]]
                out["externalIds"] = ["tvdbId": JSONValue(sh.tvdbId)]
                out["numberOfSeasons"] = JSONValue(sh.seasons.count)
                out["seasons"] = .array(
                    [["seasonNumber": 0, "name": "Specials", "episodeCount": 2]]
                        + sh.seasons.map { n, c in
                            [
                                "seasonNumber": JSONValue(n), "name": .string("Season \(n)"), "episodeCount": JSONValue(c),
                                "airDate": .string("\(2018 + n)-01-10"),
                            ]
                        })
            }
            if let info = Self.mediaInfo(s, kind, tmdb) { out["mediaInfo"] = .object(info) }
            return .json(.object(out))
        }
        if p == "/request" && method == "GET" { return listRequests(s, req, user, manage) }
        if p == "/request" && method == "POST" { return createRequest(&s, body, user, manage) }
        if let m = p.wholeMatch(of: #/\/request\/(\d+)(?:\/(approve|decline|retry))?/#) {
            guard let at = s.requests.firstIndex(where: { $0["id"]?.intValue == Int(m.1) }) else {
                return Self.message("Request not found.", 404)
            }
            let r = s.requests[at]
            let id = r["id"]!
            let action = m.2.map(String.init)
            if method == "DELETE" && action == nil {
                if !manage && (r["requestedBy"]?["id"] != user["id"] || r["status"]?.intValue != 1) {
                    return Self.message("You do not have permission to delete this request.", 401)
                }
                s.requests.remove(at: at)
                record(&s, "request-delete", ["id": id])
                return MockResponse(status: 204, headers: [:])
            }
            if method == "POST", let action {
                if !manage { return Self.message("You do not have permission to access this endpoint.", 403) }
                let status = r["status"]?.intValue
                if (action == "approve" || action == "decline") && status != 1 {
                    return Self.message("Only pending requests can be approved or declined.", 409)
                }
                if action == "retry" && status != 4 { return Self.message("Only failed requests can be retried.", 409) }
                s.requests[at]["status"] = JSONValue(["approve": 2, "decline": 3, "retry": 2][action]!)
                s.requests[at]["modifiedBy"] = .object(user)
                s.requests[at]["updatedAt"] = .string(Self.now())
                record(&s, "request-\(action)", ["id": id])
                return .json(.object(s.requests[at]))
            }
        }
        if method == "GET", let m = p.wholeMatch(of: #/\/service\/(radarr|sonarr)(?:\/(\d+))?/#) {
            let services = Self.SERVICES[String(m.1)]!
            guard let n = m.2 else { return .json(.array(services.map(JSONValue.object))) }
            guard let server = services.first(where: { $0["id"]?.intValue == Int(n) }) else {
                return Self.message("Not found", 404)
            }
            let root = server["activeDirectory"]!.stringValue!
            return .json([
                "server": .object(server), "profiles": [["id": 1, "name": "HD-1080p"], ["id": 2, "name": "Ultra-HD"]],
                "rootFolders": [
                    ["id": 1, "path": .string(root), "freeSpace": 4e12], ["id": 2, "path": .string("\(root)-archive"), "freeSpace": 1e12],
                ],
            ])
        }
        return Self.message("Not found", 404)
    }

    private func listRequests(_ s: State, _ req: MockRequest, _ user: Obj, _ manage: Bool) -> MockResponse {
        let take = Int(req.q("take") ?? "20") ?? 20
        let skip = Int(req.q("skip") ?? "0") ?? 0
        let want = req.q("filter") ?? "all"
        let by = req.q("requestedBy").flatMap { Int($0) }
        var out: [JSONValue] = []
        for r in s.requests.sorted(by: { ($0["id"]?.intValue ?? 0) > ($1["id"]?.intValue ?? 0) }) {
            let by_ = r["requestedBy"]?["id"]?.intValue
            if !manage && by_ != user["id"]?.intValue { continue }
            if let by, by_ != by { continue }
            let (kind, tmdb) = (r["type"]!.stringValue!, r["media"]!["tmdbId"]!.intValue!)
            let status = Self.mediaStatus(s, kind, tmdb, r["is4k"]?.boolValue ?? false)
            let rs = r["status"]?.intValue
            let available = status == 4 || status == 5
            if want == "pending" && rs != 1 { continue }
            if want == "approved" && rs != 2 { continue }
            if want == "failed" && rs != 4 { continue }
            if want == "available" && !available { continue }
            if (want == "processing" || want == "unavailable") && (rs != 2 || available) { continue }
            var media = Self.mediaInfo(s, kind, tmdb, false) ?? [:]
            media["tmdbId"] = JSONValue(tmdb)
            var entry = r
            entry["media"] = .object(media)
            out.append(.object(entry))
        }
        let size = max(take, 1)
        return .json([
            "pageInfo": [
                "page": JSONValue(skip / size + 1), "pages": JSONValue(max(1, (out.count + size - 1) / size)),
                "results": JSONValue(out.count), "pageSize": JSONValue(take),
            ],
            "results": .array(Array(out.dropFirst(skip).prefix(take))),
        ])
    }

    private func createRequest(_ s: inout State, _ b: JSONValue, _ user: Obj, _ manage: Bool) -> MockResponse {
        let kind = b["mediaType"]?.stringValue ?? ""
        guard let tmdb = b["mediaId"]?.intValue,
            kind == "movie" ? Self.movie(tmdb) != nil : kind == "tv" && Self.show(tmdb) != nil
        else { return Self.message("Media not found.", 404) }
        let is4k = b["is4k"]?.boolValue ?? false
        var seasons: [Int] = []
        if kind == "tv" {
            let sh = Self.show(tmdb)!
            seasons = b["seasons"] == "all" ? sh.seasons.map(\.0) : (b["seasons"]?.arrayValue ?? []).compactMap(\.intValue)
            if seasons.isEmpty { return Self.message("No seasons requested.", 400) }
            var taken = Set(is4k ? [] : sh.available)
            for r in Self.requestsFor(s, kind, tmdb) where r["is4k"]?.boolValue == is4k && ![3, 5].contains(r["status"]?.intValue ?? 0) {
                for season in r["seasons"]?.arrayValue ?? [] {
                    if let n = season["seasonNumber"]?.intValue { taken.insert(n) }
                }
            }
            // Like Seerr: seasons already requested or available are dropped; none left is a 202.
            seasons = seasons.filter { !taken.contains($0) }
            if seasons.isEmpty { return Self.message("No seasons available to request", 202) }
        } else if [2, 3, 4, 5].contains(Self.mediaStatus(s, kind, tmdb, is4k) ?? 0) {
            return Self.message("Request for this media already exists.", 409)
        }
        let req: Obj = [
            "id": JSONValue(s.nextId), "status": manage ? 2 : 1, "type": .string(kind), "is4k": .bool(is4k),
            "createdAt": .string(Self.now()), "updatedAt": .string(Self.now()), "requestedBy": .object(user),
            "modifiedBy": manage ? .object(user) : nil, "media": ["tmdbId": JSONValue(tmdb)],
            "seasons": .array(seasons.map { ["id": JSONValue($0), "seasonNumber": JSONValue($0), "status": 1] }),
            "serverId": b["serverId"] ?? nil, "profileId": b["profileId"] ?? nil, "rootFolder": b["rootFolder"] ?? nil,
        ]
        s.nextId += 1
        s.requests.append(req)
        record(
            &s, "request",
            [
                "mediaType": .string(kind), "mediaId": JSONValue(tmdb), "seasons": .array(seasons.map { JSONValue($0) }),
                "is4k": .bool(is4k), "by": user["id"]!, "serverId": b["serverId"] ?? nil, "profileId": b["profileId"] ?? nil,
                "rootFolder": b["rootFolder"] ?? nil, "tvdbId": b["tvdbId"] ?? nil,
            ])
        var out = req
        out["media"] = Self.mediaInfo(s, kind, tmdb, false).map(JSONValue.object) ?? nil
        return .json(.object(out), status: 201)
    }
}
