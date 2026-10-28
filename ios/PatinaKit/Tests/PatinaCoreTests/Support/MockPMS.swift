// Port of desktop/test/mock-pms.ts, and of the parts of tools/mock_pms.py the core tests use.
//
// The TypeScript tests start tools/mock_pms.py on a free port. Here the fake server answers
// inside the process: `MockPMS.session` routes every request through `MockProtocol`, which
// finds the `MockPMS` registered for the URL's host and port (`http://127.0.0.1:<port>`,
// a port no socket ever opens) and asks it for the answer. Unknown hosts (plex.tv,
// `127.0.0.1:9`) fail as unreachable.
//
// Usage:
//
//     let mock = MockPMS(name: "Alpha", machine: "alpha-x", seed: 0)
//     let app = AppState(sink: NullSink(), environment: TestEnv().environment)   // uses MockPMS.session
//     try await Session.signInManual(app, mock.url, "x")
//     mock.events()                                // the request log (`record` in mock_pms.py)
//
// Extending: a route is a function from the request to a response, or `nil` when it does not
// apply. `MockPMS.routes` lists the built-in ones in the order mock_pms.py tries them; add a
// group of your own (`static let downloadRoutes: [Route] = [...]` in an extension) to that
// list, or give one mock extra routes with `mock.on { mock, req in ... }`, which are tried
// first. The library data (`movie(i)`, `episode(...)`, `find(rk)`, ...) is that of
// mock_pms.py, so the TypeScript tests' rating keys and expectations carry over.

import Foundation
import PlexKit
import Synchronization

typealias Obj = [String: JSONValue]

/// A request as the fake server sees it.
nonisolated struct MockRequest: Sendable {
    let method: String
    let path: String
    /// `parse_qs`: every value of each key.
    let query: [String: [String]]
    let headers: [String: String]

    func q(_ key: String) -> String? {
        query[key]?.first
    }

    func has(_ key: String) -> Bool {
        query[key] != nil
    }

    /// The query without the `X-Plex-*` headers-as-parameters, as the mock logs it.
    var logged: [String: String] {
        query.filter { !$0.key.hasPrefix("X-Plex") }.compactMapValues(\.first)
    }
}

/// What the fake server answers.
nonisolated struct MockResponse: Sendable {
    var status = 200
    var headers: [String: String] = ["Content-Type": "application/json"]
    var body = Data()
    /// Keep the connection open after the body (an event stream); counted in `openStreams`.
    var hang = false
    /// Seconds to wait before answering (a slow server).
    var delay: Double = 0
    /// Send the body in pieces of this many bytes, `chunkDelay` seconds apart (a media transfer).
    var chunkSize = 0
    var chunkDelay: Double = 0
    /// Drop the connection once this many bytes of the body are sent.
    var dropAfter: Int?

    static func json(_ v: JSONValue, status: Int = 200, delay: Double = 0) -> MockResponse {
        MockResponse(status: status, body: Data(v.serialized().utf8), delay: delay)
    }

    static func text(_ text: String, contentType: String = "text/plain; charset=utf-8") -> MockResponse {
        MockResponse(headers: ["Content-Type": contentType], body: Data(text.utf8))
    }
}

/// `mc(**kw)`: a `MediaContainer` with `size` defaulting to the length of its list.
nonisolated func mc(_ kw: Obj = [:]) -> JSONValue {
    var kw = kw
    if kw["size"] == nil {
        let list = kw["Metadata"] ?? kw["Directory"] ?? kw["Hub"]
        kw["size"] = JSONValue(list?.arrayValue?.count ?? 0)
    }
    return ["MediaContainer": .object(kw)]
}

/// `page(items, q)`: the slice the container start and size ask for.
nonisolated func page(_ items: [Obj], _ req: MockRequest) -> JSONValue {
    let start = Int(req.q("X-Plex-Container-Start") ?? "0") ?? 0
    let size = Int(req.q("X-Plex-Container-Size") ?? "1000") ?? 1000
    let slice = Array(items.dropFirst(start).prefix(size))
    return mc([
        "Metadata": .array(slice.map(JSONValue.object)), "offset": JSONValue(start), "totalSize": JSONValue(items.count),
        "size": JSONValue(slice.count),
    ])
}

/// The mutable state of one mock server (the module globals of mock_pms.py).
nonisolated struct MockState: Sendable {
    /// Watch state per rating key: `viewCount`, `viewOffset`, `userRating`.
    var items: [String: Obj] = [:]
    /// Rating keys removed from Continue Watching.
    var hidden = Set<String>()
    /// part id -> ["audio": stream id, "subtitle": stream id or 0].
    var selected: [Int: [String: Int]] = [:]
    var playlist = (items: ["104", "105", "1012"], title: "Evening Mix", summary: "", gone: false)
    var collection = (items: ["100", "101", "102", "103"], title: "Heist Nights", gone: false)
}

nonisolated final class MockPMS: Sendable {
    typealias Route = @Sendable (MockPMS, MockRequest) -> MockResponse?

    let name: String
    let machine: String
    /// Library variant: servers with different seeds share some titles.
    let seed: Int
    /// Seconds every search takes.
    let searchDelay: Double
    /// Seconds to hold timeline, scrobble and stream-selection requests.
    let reportDelay: Double
    /// Answer those requests with this HTTP status.
    let reportStatus: Int
    /// Rating keys answered with 404.
    let missingMetadata: Set<String>
    let port: Int
    /// `http://127.0.0.1:<port>`.
    let url: String

    let state = Mutex(MockState())
    private let log = Mutex<[[String: String]]>([])
    private let extra = Mutex<[Route]>([])
    private let streams = Mutex(0)

    /// The session every Plex client in a test must use (`AppEnvironment.urlSession`).
    static let session: URLSession = {
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [MockProtocol.self]
        return URLSession(configuration: config)
    }()

    private static let registry = Mutex<[String: MockPMS]>([:])
    private static let nextPort = Atomic(41000)

    init(
        name: String = "Mock Server", machine: String = "mock-machine-0001", seed: Int = 0, searchDelay: Double = 0,
        reportDelay: Double = 0, reportStatus: Int = 200, missingMetadata: Set<String> = []
    ) {
        self.name = name
        self.machine = machine
        self.seed = seed
        self.searchDelay = searchDelay
        self.reportDelay = reportDelay
        self.reportStatus = reportStatus
        self.missingMetadata = missingMetadata
        port = MockPMS.nextPort.add(1, ordering: .relaxed).newValue
        url = "http://127.0.0.1:\(port)"
        MockPMS.registry.withLock { $0["127.0.0.1:\(port)"] = self }
    }

    /// Take the server down: requests fail as unreachable from now on.
    func stop() {
        MockPMS.registry.withLock { $0["127.0.0.1:\(port)"] = nil }
    }

    static func lookup(_ url: URL) -> MockPMS? {
        guard let host = url.host, let port = url.port else { return nil }
        return registry.withLock { $0["\(host):\(port)"] }
    }

    /// Extra routes for this server, tried before the built-in ones.
    func on(_ route: @escaping Route) {
        extra.withLock { $0.append(route) }
    }

    /// The request log so far (what mock_pms.py prints to stdout).
    func events() -> [[String: String]] {
        log.withLock { $0 }
    }

    /// The `what` of each logged request.
    func logged() -> [String] {
        events().compactMap { $0["what"] }
    }

    func record(_ what: String, _ req: MockRequest) {
        var entry = req.logged
        entry["what"] = what
        log.withLock { $0.append(entry) }
    }

    /// Event streams currently connected.
    var openStreams: Int { streams.withLock { $0 } }

    fileprivate func streamOpened() { streams.withLock { $0 += 1 } }
    fileprivate func streamClosed() { streams.withLock { $0 -= 1 } }

    func handle(_ req: MockRequest) -> MockResponse {
        for route in extra.withLock({ $0 }) + MockPMS.routes {
            if let r = route(self, req) { return r }
        }
        record("unhandled \(req.method) \(req.path)", req)
        return .json(mc())
    }

    /// The built-in routes, in the order mock_pms.py tries them.
    static let routes: [Route] = eventRoutes + playlistRoutes + reportRoutes + serverRoutes + libraryRoutes + searchRoutes
}

// MARK: - The library (port of mock_pms.py's data)

nonisolated extension MockPMS {
    static let GENRES = ["Drama", "Science Fiction", "Thriller", "Comedy", "Animation", "Mystery"]
    static let MOVIE_TITLES = [
        "The Verdigris Line", "Copper Sky", "Afterglow", "Quiet Harbor", "Northbound",
        "Paper Moons", "The Long Exposure", "Salt & Static", "Glass Orchard", "Lanterns",
        "Midnight Transit", "A Field of Echoes", "Tidewater", "The Cartographer", "Low Orbit",
        "Signal Fires", "Hollow Crown", "Bright Minutes", "The Understudy", "Ember Season",
        "Wire Garden", "Night Ferry", "The Tin Orchard", "Slow Weather", "Cold Frequency",
        "Amber Static", "Dockside", "The Last Switchboard", "Pale Meridian", "Harbor Vow",
    ]
    static let SHOW_TITLES = ["Station Eleven Below", "The Weatherman", "Parallel Lines", "Harbor Lights"]
    static let TRACK_TITLES = ["Dial Tone", "Undersea Cable", "Relay Station", "Last Transmission"]
    static let TMDB: [String: Int] = ["100": 9001, "500": 8001]

    static func now() -> Int {
        Int(Date().timeIntervalSince1970)
    }

    func img(_ kind: String, _ n: Int) -> JSONValue {
        .string("/library/metadata/\(n)/\(kind)/\(1_700_000_000 + n)")
    }

    func streamList(_ partId: Int, _ s: inout MockState) -> JSONValue {
        var streams: [Obj] = [
            [
                "id": JSONValue(partId * 10 + 1), "streamType": 1, "codec": "h264", "index": 0, "height": 720,
                "width": 1280, "displayTitle": "720p (H.264)", "frameRate": 24.0,
            ],
            [
                "id": JSONValue(partId * 10 + 2), "streamType": 2, "codec": "aac", "index": 1, "channels": 2,
                "language": "English", "languageCode": "eng", "displayTitle": "English (AAC Stereo)",
                "extendedDisplayTitle": "English (AAC Stereo)",
            ],
            [
                "id": JSONValue(partId * 10 + 3), "streamType": 2, "codec": "aac", "index": 2, "channels": 2,
                "language": "Japanese", "languageCode": "jpn", "selected": true,
                "displayTitle": "Japanese (AAC Stereo)", "extendedDisplayTitle": "Japanese (AAC Stereo)",
            ],
            [
                "id": JSONValue(partId * 10 + 4), "streamType": 3, "codec": "srt", "index": 3, "language": "English",
                "languageCode": "eng", "selected": true, "displayTitle": "English (SRT)",
                "extendedDisplayTitle": "English (SRT Embedded)",
            ],
        ]
        // Every show's S02E03 lacks the Japanese dub, to exercise "no matching track".
        if (1000..<2000).contains(partId) && partId % 100 == 23 {
            streams = streams.filter { $0["languageCode"] != "jpn" || $0["streamType"] != 2 }
            streams[1]["selected"] = true
        }
        let sel = s.selected[partId] ?? [:]
        for i in streams.indices {
            let kind = streams[i]["streamType"] == 2 ? "audio" : streams[i]["streamType"] == 3 ? "subtitle" : nil
            if let kind, let chosen = sel[kind] { streams[i]["selected"] = .bool(streams[i]["id"] == JSONValue(chosen)) }
        }
        return .array(streams.map(JSONValue.object))
    }

    func mediaFor(_ rk: String, _ s: inout MockState) -> JSONValue {
        let partId = Int(rk) ?? 0
        let hd = seed > 0
        let part: Obj = [
            "id": JSONValue(partId), "key": .string("/library/parts/\(partId)/1700000000/file.mkv"), "duration": 180000,
            "file": .string("/media/movies/\(rk).mkv"), "size": JSONValue(1024 * (hd ? 4 : 1)), "container": "mkv",
            "Stream": streamList(partId, &s),
        ]
        let media: Obj = [
            "id": JSONValue(partId), "duration": 180000, "bitrate": JSONValue(hd ? 12400 : 6200),
            "width": JSONValue(hd ? 1920 : 1280), "height": JSONValue(hd ? 1080 : 720), "aspectRatio": 1.78,
            "audioChannels": JSONValue(hd ? 6 : 2), "audioCodec": "aac", "videoCodec": .string(hd ? "hevc" : "h264"),
            "videoResolution": .string(hd ? "1080" : "720"), "container": "mkv", "videoFrameRate": "24p",
            "Part": [.object(part)],
        ]
        return [.object(media)]
    }

    func titleIndex(_ i: Int) -> Int {
        (i + seed * 10) % MockPMS.MOVIE_TITLES.count
    }

    /// Fold the stored watch state in (only the truthy values, as `m.update(...)` does).
    private func merge(_ m: inout Obj, _ state: Obj) {
        for (k, v) in state where v != .null && v != 0 && v != false { m[k] = v }
    }

    func movie(_ i: Int, _ s: inout MockState) -> Obj {
        let rk = String(100 + i)
        let t = titleIndex(i)
        if s.items[rk] == nil {
            s.items[rk] = ["viewCount": JSONValue(i % 5 == 3 ? 1 : 0), "viewOffset": JSONValue(i == 0 ? 61000 : 0)]
        }
        let genre = 10 + t % MockPMS.GENRES.count
        var m: Obj = [
            "ratingKey": .string(rk), "key": .string("/library/metadata/\(rk)"), "guid": .string("plex://movie/5d77\(100 + t)"),
            "type": "movie", "title": .string(MockPMS.MOVIE_TITLES[t]), "year": JSONValue(2014 + t % 11),
            "contentRating": .string(["PG-13", "R", "PG"][t % 3]),
            "summary": "A quiet engineer discovers that the old copper lines beneath the city still carry voices.",
            "tagline": "Every signal leaves a trace.", "studio": "Mock Pictures",
            "rating": JSONValue(7.1 + Double(t % 3) / 2), "audienceRating": JSONValue(8.0 - Double(t % 4) / 3),
            "duration": JSONValue((70 + (t * 13) % 80) * 60000), "addedAt": JSONValue(MockPMS.now() - i * 86400),
            "originallyAvailableAt": .string("\(2014 + i % 11)-05-0\(1 + i % 9)"), "thumb": img("thumb", i % 8),
            "art": img("art", i % 8), "librarySectionID": 1,
            "Genre": [["id": JSONValue(genre), "tag": .string(MockPMS.GENRES[t % MockPMS.GENRES.count]), "filter": .string("genre=\(genre)")]],
            "Director": [["id": 200, "tag": "Ada Lindqvist", "filter": "director=200"]],
            "Media": mediaFor(rk, &s),
        ]
        merge(&m, s.items[rk] ?? [:])
        if let tmdb = MockPMS.TMDB[rk] { m["Guid"] = [["id": .string("tmdb://\(tmdb)")]] }
        return m
    }

    func show(_ i: Int) -> Obj {
        let rk = String(500 + i)
        var m: Obj = [
            "ratingKey": .string(rk), "key": .string("/library/metadata/\(rk)/children"), "guid": .string("plex://show/5e\(rk)"),
            "type": "show", "title": .string(MockPMS.SHOW_TITLES[i]), "year": JSONValue(2019 + i), "duration": 2_700_000,
            "audienceRating": JSONValue(7.5 + Double(i) / 4), "summary": "An anthology of small towns and strange weather.",
            "thumb": img("thumb", (i + 3) % 8), "art": img("art", (i + 3) % 8), "childCount": 2, "leafCount": 6,
            "viewedLeafCount": JSONValue(i == 0 ? 1 : 0), "librarySectionID": 2, "contentRating": "TV-14",
            "Genre": [["id": 12, "tag": "Mystery", "filter": "genre=12"]],
        ]
        if let tmdb = MockPMS.TMDB[rk] { m["Guid"] = [["id": .string("tmdb://\(tmdb)")]] }
        return m
    }

    func season(_ showI: Int, _ n: Int) -> Obj {
        let rk = String(600 + showI * 10 + n)
        return [
            "ratingKey": .string(rk), "key": .string("/library/metadata/\(rk)/children"), "type": "season",
            "title": .string("Season \(n)"), "index": JSONValue(n), "parentRatingKey": .string(String(500 + showI)),
            "parentTitle": .string(MockPMS.SHOW_TITLES[showI]), "thumb": img("thumb", (showI + n) % 8),
            "art": img("art", (showI + 3) % 8), "leafCount": 3,
            "viewedLeafCount": JSONValue(showI == 0 && n == 1 ? 1 : 0), "librarySectionID": 2,
        ]
    }

    func episode(_ showI: Int, _ n: Int, _ e: Int, _ s: inout MockState) -> Obj {
        let rk = String(1000 + showI * 100 + n * 10 + e)
        if s.items[rk] == nil {
            s.items[rk] = ["viewCount": JSONValue(showI == 0 && n == 1 && e == 1 ? 1 : 0), "viewOffset": 0]
        }
        let key = Int(rk)!
        var m: Obj = [
            "ratingKey": .string(rk), "key": .string("/library/metadata/\(rk)"), "type": "episode",
            "title": .string("Chapter \(e): \(["Static", "Undertow", "Relay"][e - 1])"), "index": JSONValue(e),
            "parentIndex": JSONValue(n), "parentRatingKey": .string(String(600 + showI * 10 + n)),
            "parentTitle": .string("Season \(n)"), "grandparentRatingKey": .string(String(500 + showI)),
            "grandparentTitle": .string(MockPMS.SHOW_TITLES[showI]), "grandparentThumb": img("thumb", (showI + 3) % 8),
            "grandparentArt": img("art", (showI + 3) % 8), "thumb": img("art", (e + n) % 8),
            "summary": "The storm arrives early and the radio tower starts talking back.", "duration": 180000,
            "originallyAvailableAt": .string("202\(n)-0\(e)-12"), "librarySectionID": 2, "Media": mediaFor(rk, &s),
            "Marker": [
                ["id": JSONValue(key * 10 + 1), "type": "intro", "startTimeOffset": 5000, "endTimeOffset": 25000],
                [
                    "id": JSONValue(key * 10 + 2), "type": "credits", "startTimeOffset": 165000, "endTimeOffset": 180000,
                    "final": true,
                ],
            ],
        ]
        merge(&m, s.items[rk] ?? [:])
        return m
    }

    func artist() -> Obj {
        [
            "ratingKey": "3000", "key": "/library/metadata/3000/children", "type": "artist", "title": "The Copper Lines",
            "summary": "A band that only exists on the mock server.", "thumb": img("thumb", 6), "art": img("art", 6),
            "librarySectionID": 3, "childCount": 1,
        ]
    }

    func album() -> Obj {
        [
            "ratingKey": "3100", "key": "/library/metadata/3100/children", "type": "album", "title": "Low Tide Radio",
            "parentRatingKey": "3000", "parentTitle": "The Copper Lines", "year": 2021, "thumb": img("thumb", 7),
            "librarySectionID": 3, "leafCount": JSONValue(MockPMS.TRACK_TITLES.count), "studio": "Mock Records",
        ]
    }

    func track(_ i: Int) -> Obj {
        let rk = String(3101 + i)
        let partId = 3101 + i
        let part: Obj = [
            "id": JSONValue(partId), "key": .string("/library/parts/\(partId)/1700000000/file.mkv"), "duration": 180000,
            "file": .string("/media/music/\(rk).mkv"), "container": "mkv",
            "Stream": [
                ["id": JSONValue(partId * 10 + 2), "streamType": 2, "codec": "aac", "channels": 2, "selected": true, "displayTitle": "AAC Stereo"],
                [
                    "id": JSONValue(partId * 10 + 5), "streamType": 4, "codec": "lrc", "format": "lrc",
                    "key": .string("/library/streams/\(partId * 10 + 5)"),
                ],
            ],
        ]
        return [
            "ratingKey": .string(rk), "key": .string("/library/metadata/\(rk)"), "type": "track",
            "title": .string(MockPMS.TRACK_TITLES[i]), "index": JSONValue(i + 1), "parentIndex": 1,
            "parentRatingKey": "3100", "parentTitle": "Low Tide Radio", "grandparentRatingKey": "3000",
            "grandparentTitle": "The Copper Lines", "thumb": img("thumb", 7), "librarySectionID": 3, "duration": 180000,
            "Media": [["id": JSONValue(partId), "duration": 180000, "container": "mkv", "audioCodec": "aac", "audioChannels": 2, "Part": [.object(part)]]],
        ]
    }

    func playlist(_ s: MockState) -> Obj {
        [
            "ratingKey": "950", "key": "/playlists/950/items", "type": "playlist", "playlistType": "video", "smart": false,
            "title": .string(s.playlist.title), "summary": .string(s.playlist.summary),
            "leafCount": JSONValue(s.playlist.items.count), "duration": JSONValue(180000 * s.playlist.items.count),
        ]
    }

    func collection(_ s: MockState) -> Obj {
        [
            "ratingKey": "900", "key": "/library/collections/900/children", "type": "collection", "subtype": "movie",
            "title": .string(s.collection.title), "librarySectionID": 1, "childCount": JSONValue(s.collection.items.count),
            "thumb": img("thumb", 5),
        ]
    }

    func allMovies(_ s: inout MockState) -> [Obj] {
        (0..<20).map { movie($0, &s) }
    }

    func allShows() -> [Obj] {
        MockPMS.SHOW_TITLES.indices.map(show)
    }

    func find(_ rk: String, _ s: inout MockState) -> Obj? {
        guard let n = Int(rk) else { return nil }
        switch n {
        case 100..<120: return movie(n - 100, &s)
        case 500..<(500 + MockPMS.SHOW_TITLES.count): return show(n - 500)
        case 600..<700: return season((n - 600) / 10, n % 10)
        case 1000..<2000: return episode((n - 1000) / 100, (n % 100) / 10, n % 10, &s)
        case 900 where !s.collection.gone: return collection(s)
        case 3000: return artist()
        case 3100: return album()
        case 3101..<(3101 + MockPMS.TRACK_TITLES.count): return track(n - 3101)
        default: return nil
        }
    }

    func children(_ rk: String, _ s: inout MockState) -> [Obj] {
        guard let n = Int(rk) else { return [] }
        switch n {
        case 500..<600: return [1, 2].map { season(n - 500, $0) }
        case 600..<700: return [1, 2, 3].map { episode((n - 600) / 10, n % 10, $0, &s) }
        case 3000: return [album()]
        case 3100: return MockPMS.TRACK_TITLES.indices.map(track)
        default: return []
        }
    }

    func leaves(_ rk: String, _ s: inout MockState) -> [Obj] {
        guard let item = find(rk, &s) else { return [] }
        switch item["type"]?.stringValue {
        case "show": return children(rk, &s).flatMap { children($0["ratingKey"]?.stringValue ?? "", &s) }
        case "season", "album": return children(rk, &s)
        case "artist": return children("3100", &s)
        default: return [item]
        }
    }
}

// MARK: - Routes

nonisolated extension MockPMS {
    /// Live notifications: the stream stays open (mock_pms.py sends a fake scan every 30 seconds).
    static let eventRoutes: [Route] = [
        { _, req in
            guard req.path == "/:/eventsource/notifications" else { return nil }
            // The comment line flushes the headers, as a parked real server does.
            return MockResponse(headers: ["Content-Type": "text/event-stream"], body: Data(":\n\n".utf8), hang: true)
        }
    ]

    /// Play queue edits, and the playlist (950) and its entries (`7000 + rating key`).
    static let playlistRoutes: [Route] = [
        { mock, req in
            guard req.method != "GET",
                req.path.wholeMatch(of: #/\/playQueues\/\d+(\/items(\/\d+(\/move)?)?|\/shuffle|\/unshuffle)?/#) != nil
            else { return nil }
            mock.record("playqueue \(req.method) \(req.path)", req)
            return .json(mc(["playQueueID": 77]))
        },
        { mock, req in
            guard req.path == "/playlists" else { return nil }
            if req.method == "GET" {
                let kind = req.q("playlistType") ?? "video"
                let lists = mock.state.withLock { s in s.playlist.gone || kind != "video" ? [] : [mock.playlist(s)] }
                return .json(mc(["Metadata": .array(lists.map(JSONValue.object))]))
            }
            mock.record("create_playlist", req)
            var created = mock.state.withLock { mock.playlist($0) }
            created["ratingKey"] = "951"
            created["title"] = .string(req.q("title") ?? "")
            return .json(mc(["Metadata": [.object(created)]]))
        },
        { mock, req in
            guard let m = req.path.wholeMatch(of: #/\/playlists\/950(\/items(?:\/(\d+)(\/move)?)?)?/#) else { return nil }
            let sub = m.1.map(String.init)
            let entry = m.2.map(String.init)
            let move = m.3 != nil
            if sub == nil && req.method == "GET" {
                return .json(mc(["Metadata": [.object(mock.state.withLock { mock.playlist($0) })]]))
            }
            if sub == "/items" && entry == nil && req.method == "GET" {
                let out = mock.state.withLock { s in
                    s.playlist.items.compactMap { k -> Obj? in
                        guard var item = mock.find(k, &s) else { return nil }
                        item["playlistItemID"] = JSONValue(7000 + Int(k)!)
                        return item
                    }
                }
                return .json(page(out, req))
            }
            mock.state.withLock { s in
                if sub == nil && req.method == "PUT" {
                    if let t = req.q("title") { s.playlist.title = t }
                    if let t = req.q("summary") { s.playlist.summary = t }
                } else if sub == nil && req.method == "DELETE" {
                    s.playlist.gone = true
                } else if sub == "/items" && entry == nil && req.method == "DELETE" {
                    s.playlist.items.removeAll()
                } else if sub == "/items" && entry == nil && req.method == "PUT" {
                    if let uri = req.q("uri"), let last = uri.split(separator: "/").last { s.playlist.items.append(String(last)) }
                } else if move, let entry {
                    let rk = String(Int(entry)! - 7000)
                    s.playlist.items.removeAll { $0 == rk }
                    let after = req.q("after").map { String(Int($0)! - 7000) }
                    let at = after.flatMap { a in s.playlist.items.firstIndex(of: a).map { $0 + 1 } } ?? 0
                    s.playlist.items.insert(rk, at: at)
                } else if let entry, req.method == "DELETE" {
                    s.playlist.items.removeAll { $0 == String(Int(entry)! - 7000) }
                }
            }
            mock.record("playlist \(req.method) \(req.path)", req)
            return .json(mc())
        },
    ]

    /// Playback reporting: timeline, scrobbles, progress, Continue Watching, ratings, stream choices.
    static let reportRoutes: [Route] = [
        { mock, req in
            let p = req.path
            let reporting =
                ["/:/timeline", "/:/scrobble", "/:/unscrobble"].contains(p)
                || p.wholeMatch(of: #/\/library\/parts\/\d+/#) != nil
            guard reporting || ["/:/progress", "/actions/removeFromContinueWatching", "/:/rate"].contains(p) else {
                return nil
            }
            // Simulate a slow or failing server for playback reports.
            let delay = reporting ? mock.reportDelay : 0
            if reporting && mock.reportStatus != 200 {
                mock.record("failed report \(p)", req)
                return .json(mc(), status: mock.reportStatus, delay: delay)
            }
            switch p {
            case "/:/timeline":
                mock.record("timeline", req)
                let time = Int(req.q("time") ?? "0") ?? 0
                let duration = Int(req.q("duration") ?? "0") ?? 0
                let finished = req.q("state") == "stopped" && Double(time) >= Double(duration) * 0.9
                mock.state.withLock { $0.items[req.q("ratingKey") ?? "", default: [:]]["viewOffset"] = JSONValue(finished ? 0 : time) }
            case "/:/scrobble", "/:/unscrobble":
                mock.record(String(p.dropFirst(3)), req)
                mock.state.withLock { s in
                    let key = req.q("key") ?? ""
                    s.items[key, default: [:]]["viewCount"] = JSONValue(p == "/:/scrobble" ? 1 : 0)
                    s.items[key, default: [:]]["viewOffset"] = 0
                }
            case "/:/progress":
                mock.record("progress", req)
                let time = Int(req.q("time") ?? "0") ?? 0
                mock.state.withLock { s in
                    let key = req.q("key") ?? ""
                    s.items[key, default: [:]]["viewOffset"] = JSONValue(time)
                    if time > 0 { s.hidden.remove(key) }
                }
            case "/actions/removeFromContinueWatching":
                mock.record("removeFromContinueWatching", req)
                mock.state.withLock { _ = $0.hidden.insert(req.q("ratingKey") ?? "") }
            case "/:/rate":
                mock.record("rate", req)
                mock.state.withLock { $0.items[req.q("key") ?? "", default: [:]]["userRating"] = JSONValue(Double(req.q("rating") ?? "")) }
            default:
                mock.record("select_streams", req)
                let part = Int(p.split(separator: "/").last ?? "") ?? 0
                mock.state.withLock { s in
                    if let a = req.q("audioStreamID").flatMap(Int.init) { s.selected[part, default: [:]]["audio"] = a }
                    if let t = req.q("subtitleStreamID").flatMap(Int.init) { s.selected[part, default: [:]]["subtitle"] = t }
                }
            }
            return .json(mc(), delay: delay)
        }
    ]

    /// The server itself: identity, capabilities, libraries, home hubs, activities.
    static let serverRoutes: [Route] = [
        { mock, req in
            switch req.path {
            case "/identity":
                return .json(mc(["machineIdentifier": .string(mock.machine), "version": "1.41.0.9999", "claimed": true]))
            case "/":
                return .json(
                    mc([
                        "friendlyName": .string(mock.name), "machineIdentifier": .string(mock.machine),
                        "version": "1.41.0.9999", "platform": "MacOSX", "platformVersion": "27.0",
                        "myPlexUsername": "tester", "myPlexSubscription": true, "transcoderVideo": true,
                    ]))
            case "/library/sections":
                let now = MockPMS.now()
                let sections: [(String, String, String, String)] = [
                    ("1", "Movies", "movie", "/media/movies"), ("2", "TV Shows", "show", "/media/tv"),
                    ("3", "Music", "artist", "/media/music"),
                ]
                return .json(
                    mc([
                        "Directory": .array(
                            sections.map { key, title, type, path in
                                [
                                    "key": .string(key), "title": .string(title), "type": .string(type),
                                    "Location": [["id": JSONValue(Int(key)), "path": .string(path)]],
                                    "scannedAt": JSONValue(now - 3600),
                                ]
                            })
                    ]))
            case "/activities":
                return .json(mc(["Activity": []]))
            case "/hubs":
                let (movies, cont) = mock.state.withLock { s in
                    let movies = mock.allMovies(&s)
                    let cont = (movies + [mock.episode(0, 1, 2, &s)]).filter {
                        ($0["viewOffset"] != nil || $0["type"] == "episode")
                            && !s.hidden.contains($0["ratingKey"]?.stringValue ?? "")
                    }
                    return (movies, cont)
                }
                return .json(
                    mc([
                        "Hub": [
                            [
                                "hubIdentifier": "home.continue", "title": "Continue Watching", "type": "mixed",
                                "size": JSONValue(cont.count), "Metadata": .array(cont.map(JSONValue.object)),
                            ],
                            [
                                "hubIdentifier": "movie.recentlyadded.1", "title": "Recently Added Movies", "type": "movie",
                                "more": true, "key": "/library/sections/1/all?sort=addedAt:desc", "size": 12,
                                "Metadata": .array(movies.prefix(12).map(JSONValue.object)),
                            ],
                            [
                                "hubIdentifier": "tv.recentlyadded.2", "title": "Recently Added TV", "type": "show",
                                "size": 4, "Metadata": .array(mock.allShows().map(JSONValue.object)),
                            ],
                        ]
                    ]))
            default:
                return nil
            }
        }
    ]

    /// Library listings and metadata.
    static let libraryRoutes: [Route] = [
        { mock, req in
            guard let m = req.path.wholeMatch(of: #/\/library\/sections\/(\d+)\/all/#) else { return nil }
            let sid = String(m.1)
            var items = mock.state.withLock { s in
                sid == "1" ? mock.allMovies(&s) : sid == "3" ? [mock.artist()] : mock.allShows()
            }
            if req.q("unwatched") == "1" { items = items.filter { ($0["viewCount"]?.intValue ?? 0) == 0 } }
            let sort = req.q("sort") ?? "titleSort"
            let key = sort.split(separator: ":").first.map(String.init) ?? "titleSort"
            let desc = sort.hasSuffix(":desc")
            items.sort { a, b in
                let ordered: Bool
                switch key {
                case "addedAt", "year", "rating":
                    ordered = (a[key]?.doubleValue ?? 0) < (b[key]?.doubleValue ?? 0)
                default:
                    ordered = (a["title"]?.stringValue ?? "") < (b["title"]?.stringValue ?? "")
                }
                return desc ? !ordered : ordered
            }
            return .json(page(items, req))
        },
        { mock, req in
            guard let m = req.path.wholeMatch(of: #/\/library\/metadata\/(\d+(?:,\d+)+)/#) else { return nil }
            let found = mock.state.withLock { s in m.1.split(separator: ",").compactMap { mock.find(String($0), &s) } }
            return .json(mc(["Metadata": .array(found.map(JSONValue.object))]))
        },
        { mock, req in
            guard let m = req.path.wholeMatch(of: #/\/library\/metadata\/(\d+)(\/children|\/related|\/allLeaves)?/#) else {
                return nil
            }
            let rk = String(m.1)
            if mock.missingMetadata.contains(rk) { return .json(mc(), status: 404) }
            switch m.2.map(String.init) {
            case "/allLeaves":
                return .json(mc(["Metadata": .array(mock.state.withLock { mock.leaves(rk, &$0) }.map(JSONValue.object))]))
            case "/children":
                return .json(mc(["Metadata": .array(mock.state.withLock { mock.children(rk, &$0) }.map(JSONValue.object))]))
            case "/related":
                return .json(mc(["Hub": []]))
            default:
                let found = mock.state.withLock { s -> Obj? in
                    guard var item = mock.find(rk, &s) else { return nil }
                    if item["type"] == "show" { item["OnDeck"] = ["Metadata": [.object(mock.episode(Int(rk)! - 500, 1, 2, &s))]] }
                    let similar = Array(mock.allMovies(&s)[3..<11])
                    item["Related"] = [
                        "Hub": [
                            [
                                "hubIdentifier": "similar", "title": "More Like This", "type": "movie",
                                "Metadata": .array(similar.map(JSONValue.object)),
                            ]
                        ]
                    ]
                    return item
                }
                return .json(mc(["Metadata": .array(found.map { [.object($0)] } ?? [])]))
            }
        },
        { mock, req in
            guard req.path == "/library/collections/900/children" else { return nil }
            let items = mock.state.withLock { s in s.collection.items.compactMap { mock.find($0, &s) } }
            return .json(mc(["Metadata": .array(items.map(JSONValue.object))]))
        },
    ]

    /// Search, held for `searchDelay` seconds.
    static let searchRoutes: [Route] = [
        { mock, req in
            guard req.path == "/hubs/search" else { return nil }
            let query = (req.q("query") ?? "").lowercased()
            let movies = mock.state.withLock { mock.allMovies(&$0) }.filter {
                ($0["title"]?.stringValue ?? "").lowercased().contains(query)
            }
            let shows = mock.allShows().filter { ($0["title"]?.stringValue ?? "").lowercased().contains(query) }
            var hubs: [JSONValue] = []
            if !movies.isEmpty {
                hubs.append(["hubIdentifier": "movie", "title": "Movies", "type": "movie", "Metadata": .array(movies.map(JSONValue.object))])
            }
            if !shows.isEmpty {
                hubs.append(["hubIdentifier": "show", "title": "Shows", "type": "show", "Metadata": .array(shows.map(JSONValue.object))])
            }
            return .json(mc(["Hub": .array(hubs)]), delay: mock.searchDelay)
        }
    ]
}

// MARK: - The transport

/// Answers every request of `MockPMS.session` from the registered `MockPMS`.
nonisolated final class MockProtocol: URLProtocol, @unchecked Sendable {
    private struct Flight {
        var stopped = false
        /// The server whose stream this request holds open.
        var holding: MockPMS?
    }

    private let flight = Mutex(Flight())

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        let req = request
        guard let url = req.url, let mock = MockPMS.lookup(url) else {
            client?.urlProtocol(self, didFailWithError: URLError(.cannotConnectToHost))
            return
        }
        let c = URLComponents(url: url, resolvingAgainstBaseURL: true)
        var query: [String: [String]] = [:]
        for (k, v) in formPairs(c?.percentEncodedQuery ?? "") { query[k, default: []].append(v) }
        let seen = MockRequest(
            method: req.httpMethod ?? "GET", path: c?.path ?? url.path, query: query, headers: req.allHTTPHeaderFields ?? [:])
        let r = mock.handle(seen)
        if r.delay > 0 {
            DispatchQueue.global().asyncAfter(deadline: .now() + r.delay) { self.deliver(r, mock, url) }
        } else {
            deliver(r, mock, url)
        }
    }

    private func deliver(_ r: MockResponse, _ mock: MockPMS, _ url: URL) {
        let go = flight.withLock { f -> Bool in
            if f.stopped { return false }
            if r.hang {
                f.holding = mock
                mock.streamOpened()
            }
            return true
        }
        guard go else { return }
        let resp = HTTPURLResponse(url: url, statusCode: r.status, httpVersion: "HTTP/1.1", headerFields: r.headers)!
        client?.urlProtocol(self, didReceive: resp, cacheStoragePolicy: .notAllowed)
        if r.chunkSize > 0 { return sendChunk(r, from: 0) }
        if !r.body.isEmpty { client?.urlProtocol(self, didLoad: r.body) }
        if !r.hang { client?.urlProtocolDidFinishLoading(self) }
    }

    private func sendChunk(_ r: MockResponse, from at: Int) {
        if flight.withLock({ $0.stopped }) { return }
        if let drop = r.dropAfter, at >= drop {
            client?.urlProtocol(self, didFailWithError: URLError(.networkConnectionLost))
            return
        }
        guard at < r.body.count else {
            client?.urlProtocolDidFinishLoading(self)
            return
        }
        let end = min(at + r.chunkSize, r.body.count)
        client?.urlProtocol(self, didLoad: r.body.subdata(in: at..<end))
        DispatchQueue.global().asyncAfter(deadline: .now() + r.chunkDelay) { self.sendChunk(r, from: end) }
    }

    override func stopLoading() {
        let held = flight.withLock { f -> MockPMS? in
            f.stopped = true
            defer { f.holding = nil }
            return f.holding
        }
        held?.streamClosed()
    }
}

/// `parse_qs` of a form-encoded query: `+` is a space, `%XX` a byte.
nonisolated func formPairs(_ query: String) -> [(String, String)] {
    func decode(_ s: Substring) -> String {
        s.replacingOccurrences(of: "+", with: " ").removingPercentEncoding ?? String(s)
    }
    return query.split(separator: "&").map { pair in
        guard let eq = pair.firstIndex(of: "=") else { return (decode(pair), "") }
        return (decode(pair[..<eq]), decode(pair[pair.index(after: eq)...]))
    }
}
