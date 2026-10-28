// Port of desktop/core/seerr.ts.
// Seerr: request movies and shows the server doesn't have yet, and follow or manage requests.
// The instance is set in `seerr` of config.json, its API key in the Keychain; with no API key
// Patina signs in with the Plex account. The front end gets the shapes below, never the key
// or the session.

import Foundation
import PlexKit
import SeerrKit

/// Where a title stands in Seerr (its `MediaStatus`); `nil` when nobody has requested it.
public nonisolated enum Availability: String, Sendable, Equatable {
    case unknown, pending, processing, partial, available, blocklisted, deleted
}

/// A request's own state (`MediaRequestStatus`).
public nonisolated enum RequestState: String, Sendable, Equatable {
    case pending, approved, declined, failed, completed
}

public nonisolated struct SeerrTitle: Sendable, Equatable {
    public var mediaType: SeerrMediaType
    public var tmdbId: Int
    public var title: String
    public var year: Int?
    public var overview: String
    public var poster: String?
    public var backdrop: String?
    /// TMDB vote average, 0–10.
    public var rating: Double?
    public var status: Availability?
    public var status4k: Availability?
    /// Rating key of the copy on the Plex server Seerr syncs with, once available (checked
    /// against this server in `details`).
    public var ratingKey: String?
}

public nonisolated struct SeerrSeason: Sendable, Equatable {
    public var number: Int
    public var name: String
    public var episodes: Int?
    public var airDate: String?
    public var status: Availability?
    public var status4k: Availability?
    /// State of the newest open request covering it, if any.
    public var requested: RequestState?
    public var requested4k: RequestState?
}

public nonisolated struct SeerrRequest: Sendable, Equatable {
    public var id: Int
    public var state: RequestState
    public var mediaType: SeerrMediaType
    public var tmdbId: Int
    public var title: String
    public var year: Int?
    public var poster: String?
    public var is4k: Bool
    public var seasons: [Int]
    public var requestedBy: String
    /// Requested by the signed-in Seerr user.
    public var mine: Bool
    public var createdAt: String?
    /// Availability of the title itself.
    public var status: Availability?
}

/// A title's page: the `SeerrTitle` fields (read through, `details.title`) and more.
@dynamicMemberLookup
public nonisolated struct SeerrDetails: Sendable, Equatable {
    public var base: SeerrTitle
    public var tagline: String
    public var genres: [String]
    /// Minutes (an episode's, for shows).
    public var runtime: Int?
    public var tvdbId: Int?
    public var seasons: [SeerrSeason]
    public var requests: [SeerrRequest]

    public subscript<T>(dynamicMember key: KeyPath<SeerrTitle, T>) -> T {
        base[keyPath: key]
    }
}

public nonisolated struct SeerrPage<T: Sendable & Equatable>: Sendable, Equatable {
    public var page: Int
    public var pages: Int
    public var results: [T]
}

/// What the signed-in Seerr user may do.
public nonisolated struct SeerrCan: Sendable, Equatable {
    public var requestMovie: Bool
    public var requestTv: Bool
    public var request4kMovie: Bool
    public var request4kTv: Bool
    /// Choose the Radarr/Sonarr server, quality profile and folder.
    public var advanced: Bool
    /// Approve, decline, retry and delete anyone's requests.
    public var manage: Bool
}

public nonisolated struct SeerrStatus: Sendable, Equatable {
    public struct User: Sendable, Equatable {
        public var id: Int
        public var name: String
        public var avatar: String?
    }

    public var url: String
    /// How Patina signs in.
    public var auth: SeerrClient.AuthKind
    public var version: String?
    public var user: User
    public var can: SeerrCan
}

/// The instance as configured, without talking to it.
public nonisolated struct SeerrConfig: Sendable, Equatable {
    public var url: String
    public var auth: SeerrClient.AuthKind
}

public enum Seerr {
    public nonisolated struct RequestOptionServer: Sendable, Equatable {
        public struct RootFolder: Sendable, Equatable {
            public var path: String
            public var freeSpace: Int?
        }

        public var id: Int
        public var name: String
        public var isDefault: Bool
        public var activeProfileId: Int?
        public var activeDirectory: String?
        public var profiles: [RawServiceDetails.Profile]
        public var rootFolders: [RootFolder]
    }

    public nonisolated struct NewRequest: Sendable, Equatable {
        public var mediaType: SeerrMediaType
        public var tmdbId: Int
        public var tvdbId: Int?
        public var seasons: [Int]?
        public var is4k: Bool
        public var serverId: Int?
        public var profileId: Int?
        public var rootFolder: String?

        public init(
            mediaType: SeerrMediaType, tmdbId: Int, tvdbId: Int? = nil, seasons: [Int]? = nil, is4k: Bool = false,
            serverId: Int? = nil, profileId: Int? = nil, rootFolder: String? = nil
        ) {
            self.mediaType = mediaType
            self.tmdbId = tmdbId
            self.tvdbId = tvdbId
            self.seasons = seasons
            self.is4k = is4k
            self.serverId = serverId
            self.profileId = profileId
            self.rootFolder = rootFolder
        }
    }

    // Seerr's permission bits (server/lib/permissions.ts). ADMIN implies all of them.
    nonisolated enum P {
        static let ADMIN = 2
        static let MANAGE_REQUESTS = 16
        static let REQUEST = 32
        static let REQUEST_4K = 1024
        static let REQUEST_4K_MOVIE = 2048
        static let REQUEST_4K_TV = 4096
        static let REQUEST_ADVANCED = 8192
        static let REQUEST_MOVIE = 262144
        static let REQUEST_TV = 524288
    }

    /// Whether `permissions` grant any of `wanted` (admins have them all).
    public nonisolated static func hasAny(_ permissions: Int, _ wanted: Int...) -> Bool {
        if permissions & P.ADMIN != 0 { return true }
        return wanted.contains { permissions & $0 != 0 }
    }

    /// What the user may do, given which kinds have a 4K server set up.
    public nonisolated static func capabilities(_ permissions: Int, movie4k: Bool, tv4k: Bool) -> SeerrCan {
        SeerrCan(
            requestMovie: hasAny(permissions, P.REQUEST, P.REQUEST_MOVIE),
            requestTv: hasAny(permissions, P.REQUEST, P.REQUEST_TV),
            request4kMovie: movie4k && hasAny(permissions, P.REQUEST_4K, P.REQUEST_4K_MOVIE),
            request4kTv: tv4k && hasAny(permissions, P.REQUEST_4K, P.REQUEST_4K_TV),
            advanced: hasAny(permissions, P.REQUEST_ADVANCED, P.MANAGE_REQUESTS),
            manage: hasAny(permissions, P.MANAGE_REQUESTS))
    }

    private nonisolated static let AVAILABILITY: [Int: Availability] = [
        1: .unknown, 2: .pending, 3: .processing, 4: .partial, 5: .available, 6: .blocklisted, 7: .deleted,
    ]
    private nonisolated static let REQUEST_STATES: [Int: RequestState] = [
        1: .pending, 2: .approved, 3: .declined, 4: .failed, 5: .completed,
    ]

    public nonisolated static func availability(_ n: Int?) -> Availability? {
        n.flatMap { AVAILABILITY[$0] }
    }

    public nonisolated static func requestState(_ n: Int?) -> RequestState {
        n.flatMap { REQUEST_STATES[$0] } ?? .pending
    }

    /// TMDB artwork URL for a Seerr image path (absolute URLs pass through). `size` is
    /// `w500` or `w1280`.
    public nonisolated static func tmdbImage(_ path: String?, _ size: String) -> String? {
        guard let path, !path.isEmpty else { return nil }
        if path.hasPrefix("http://") || path.hasPrefix("https://") { return path }
        return "https://image.tmdb.org/t/p/\(size)\(path.hasPrefix("/") ? "" : "/")\(path)"
    }

    /// `parseInt(date.slice(0, 4))`.
    private nonisolated static func yearOf(_ date: String?) -> Int? {
        guard let date else { return nil }
        let head = date.prefix(4)
        let match = head.prefixMatch(of: #/\s*[+-]?[0-9]+/#).map { String($0.output).trimmingCharacters(in: .whitespaces) }
        return match.flatMap { Int($0) }
    }

    private nonisolated static func userName(_ u: RawUser?) -> String {
        guard let u else { return "Unknown" }
        for name in [u.displayName, u.username, u.plexUsername, u.jellyfinUsername, u.email] {
            if let name, !name.isEmpty { return name }
        }
        return "User \(u.id)"
    }

    /// What a search result and a movie's or show's details share (the TypeScript reads
    /// them through one loose type).
    private nonisolated struct Fields {
        var id: Int
        var title: String?
        var name: String?
        var releaseDate: String?
        var firstAirDate: String?
        var overview: String?
        var posterPath: String?
        var backdropPath: String?
        var voteAverage: Double?
        var mediaInfo: RawMediaInfo?

        init(_ r: RawResult) {
            self.init(
                id: r.id, title: r.title, name: r.name, releaseDate: r.releaseDate, firstAirDate: r.firstAirDate,
                overview: r.overview, posterPath: r.posterPath, backdropPath: r.backdropPath,
                voteAverage: r.voteAverage, mediaInfo: r.mediaInfo)
        }

        init(_ r: RawMovieDetails) {
            self.init(
                id: r.id, title: r.title, releaseDate: r.releaseDate, overview: r.overview, posterPath: r.posterPath,
                backdropPath: r.backdropPath, voteAverage: r.voteAverage, mediaInfo: r.mediaInfo)
        }

        init(_ r: RawTvDetails) {
            self.init(
                id: r.id, name: r.name, firstAirDate: r.firstAirDate, overview: r.overview, posterPath: r.posterPath,
                backdropPath: r.backdropPath, voteAverage: r.voteAverage, mediaInfo: r.mediaInfo)
        }

        init(
            id: Int, title: String? = nil, name: String? = nil, releaseDate: String? = nil, firstAirDate: String? = nil,
            overview: String?, posterPath: String?, backdropPath: String?, voteAverage: Double?,
            mediaInfo: RawMediaInfo?
        ) {
            self.id = id
            self.title = title
            self.name = name
            self.releaseDate = releaseDate
            self.firstAirDate = firstAirDate
            self.overview = overview
            self.posterPath = posterPath
            self.backdropPath = backdropPath
            self.voteAverage = voteAverage
            self.mediaInfo = mediaInfo
        }
    }

    private nonisolated static func titleFrom(_ mediaType: SeerrMediaType, _ r: Fields) -> SeerrTitle {
        let info = r.mediaInfo
        let rating = r.voteAverage.flatMap { $0 == 0 || $0.isNaN ? nil : $0 }
        // A title that arrived only in 4K is named by its 4K key.
        let ratingKey = [info?.ratingKey, info?.ratingKey4k].lazy.compactMap { $0 }.first { !$0.isEmpty }
        return SeerrTitle(
            mediaType: mediaType, tmdbId: r.id, title: (mediaType == .movie ? r.title : r.name) ?? r.title ?? r.name ?? "Untitled",
            year: yearOf(mediaType == .movie ? r.releaseDate : r.firstAirDate), overview: r.overview ?? "",
            poster: tmdbImage(r.posterPath, "w500"), backdrop: tmdbImage(r.backdropPath, "w1280"), rating: rating,
            status: availability(info?.status), status4k: availability(info?.status4k), ratingKey: ratingKey)
    }

    /// Movies and shows of a search or discover page (people and collections are left out).
    public nonisolated static func titlesFrom(_ results: [RawResult]) -> [SeerrTitle] {
        results.compactMap { r in SeerrMediaType(rawValue: r.mediaType).map { titleFrom($0, Fields(r)) } }
    }

    /// What a request shows of its title: only its TMDB id is in the request.
    nonisolated struct Known: Sendable, Equatable {
        var title: String
        var year: Int?
        var poster: String?

        init(title: String, year: Int?, poster: String?) {
            self.title = title
            self.year = year
            self.poster = poster
        }

        init(_ t: SeerrTitle) {
            self.init(title: t.title, year: t.year, poster: t.poster)
        }
    }

    private nonisolated static func requestFrom(_ r: RawMediaRequest, _ me: Int?, _ known: Known?) -> SeerrRequest {
        let is4k = r.is4k ?? false
        return SeerrRequest(
            id: r.id, state: requestState(r.status), mediaType: r.type ?? r.media?.mediaType ?? .movie,
            tmdbId: r.media?.tmdbId ?? 0, title: known?.title ?? "", year: known?.year, poster: known?.poster,
            is4k: is4k, seasons: (r.seasons ?? []).map(\.seasonNumber).sorted(),
            requestedBy: userName(r.requestedBy), mine: me != nil && r.requestedBy?.id == me, createdAt: r.createdAt,
            status: availability(is4k ? r.media?.status4k : r.media?.status))
    }

    /// Stable sort by a string key (`a.localeCompare(b)` on ISO dates).
    private nonisolated static func sortedBy<T>(_ list: [T], descending: Bool = false, _ key: (T) -> String) -> [T] {
        list.enumerated()
            .sorted { a, b in
                let (ka, kb) = (key(a.element), key(b.element))
                if ka != kb { return descending ? ka > kb : ka < kb }
                return a.offset < b.offset
            }
            .map(\.element)
    }

    /// Seasons of a show (without specials), with their availability and open requests.
    public nonisolated static func seasonsFrom(_ tv: RawTvDetails) -> [SeerrSeason] {
        let info = tv.mediaInfo
        let open = sortedBy((info?.requests ?? []).filter { $0.status != 3 }) { $0.createdAt ?? "" }
        func requested(_ n: Int, _ is4k: Bool) -> RequestState? {
            var state: RequestState?
            for r in open where (r.is4k ?? false) == is4k && (r.seasons ?? []).contains(where: { $0.seasonNumber == n }) {
                state = requestState(r.status)
            }
            return state
        }
        return (tv.seasons ?? []).filter { $0.seasonNumber > 0 }.map { s in
            let media = info?.seasons?.first { $0.seasonNumber == s.seasonNumber }
            return SeerrSeason(
                number: s.seasonNumber, name: s.name.flatMap { $0.isEmpty ? nil : $0 } ?? "Season \(s.seasonNumber)",
                episodes: s.episodeCount, airDate: s.airDate, status: availability(media?.status),
                status4k: availability(media?.status4k), requested: requested(s.seasonNumber, false),
                requested4k: requested(s.seasonNumber, true))
        }
    }

    /// A client and the user it signs in as.
    final class Entry {
        let key: String
        let client: SeerrClient
        var me: RawUser?

        init(key: String, client: SeerrClient) {
            self.key = key
            self.client = client
        }
    }

    /// The module state: the client, kept while the settings it was made from stay the same
    /// (it holds the Plex session), and the titles of requested media.
    final class State {
        var cached: Entry?
        var titles: [String: Known] = [:]
        /// Keys of `titles`, oldest first (a `Map`'s insertion order).
        var order: [String] = []
    }

    private static func state(_ app: AppState) -> State {
        app.local(State.self) { State() }
    }

    /// The configured instance; `nil` when Seerr is not set up.
    public static func config(_ app: AppState) -> SeerrConfig? {
        let s = app.settings().seerr
        let url = s.url.trimmingCharacters(in: .whitespacesAndNewlines)
        if url.isEmpty { return nil }
        return SeerrConfig(url: url, auth: s.apiKey.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? .plex : .apiKey)
    }

    private static func plexToken(_ app: AppState) throws -> String {
        guard let token = app.settings().auth.token, !token.isEmpty else {
            throw SeerrError("Not signed in to plex.tv: add a Seerr API key to connect.")
        }
        return token
    }

    private static func clientFor(_ url: String, _ apiKey: String, _ app: AppState) throws -> Entry {
        let key = apiKey.isEmpty ? "\(url)|plex|\(try plexToken(app))" : "\(url)|key|\(apiKey)"
        if let cached = state(app).cached, cached.key == key { return cached }
        let auth: SeerrAuth = apiKey.isEmpty ? .plexToken(try plexToken(app)) : .apiKey(apiKey)
        let client = try SeerrClient(
            url, auth: auth, configuration: app.environment.urlSession?.configuration ?? .ephemeral)
        return Entry(key: key, client: client)
    }

    private static func current(_ app: AppState) throws -> Entry {
        let s = app.settings().seerr
        let url = s.url.trimmingCharacters(in: .whitespacesAndNewlines)
        if url.isEmpty { throw SeerrError("Seerr is not set up. Add it in Settings.") }
        let entry = try clientFor(url, s.apiKey.trimmingCharacters(in: .whitespacesAndNewlines), app)
        state(app).cached = entry
        return entry
    }

    private static func me(_ entry: Entry) async throws -> RawUser {
        if let me = entry.me { return me }
        let me = try await entry.client.me()
        entry.me = me
        return me
    }

    /// Talk to the instance: version, the signed-in user and what they may do.
    public static func status(_ app: AppState) async throws -> SeerrStatus {
        try await statusOf(current(app))
    }

    private static func statusOf(_ entry: Entry) async throws -> SeerrStatus {
        let client = entry.client
        async let info = try? client.status()
        let user = try await client.me()
        entry.me = user
        // Without a 4K server there is nothing to request in 4K. Users may not be allowed to list them.
        @Sendable func has4k(_ kind: SeerrClient.ServiceKind) async -> Bool {
            ((try? await client.services(kind)) ?? []).contains { $0.is4k }
        }
        async let movie4k = has4k(.radarr)
        async let tv4k = has4k(.sonarr)
        return SeerrStatus(
            url: client.baseUrl, auth: client.authKind, version: await info?.version,
            user: SeerrStatus.User(
                id: user.id, name: userName(user), avatar: user.avatar.flatMap { $0.isEmpty ? nil : avatarUrl(client.baseUrl, $0) }),
            can: capabilities(user.permissions ?? 0, movie4k: await movie4k, tv4k: await tv4k))
    }

    /// Avatars are plex.tv or gravatar URLs, or paths on the Seerr server.
    private nonisolated static func avatarUrl(_ base: String, _ avatar: String) -> String {
        avatar.hasPrefix("http://") || avatar.hasPrefix("https://")
            ? avatar : "\(base)\(avatar.hasPrefix("/") ? "" : "/")\(avatar)"
    }

    /// Check the instance answers and accepts the sign-in, then save it.
    /// `apiKey` nil keeps the saved key; empty signs in with the Plex account.
    public static func connect(_ app: AppState, _ url: String, _ apiKey: String?) async throws -> SeerrStatus {
        let base = try normalizeUrl(url)
        let key = (apiKey ?? app.settings().seerr.apiKey).trimmingCharacters(in: .whitespacesAndNewlines)
        let entry = try clientFor(base, key, app)
        let result = try await statusOf(entry)
        try app.tryUpdateSettings { s in
            s.seerr.url = base
            s.seerr.apiKey = key
        }
        state(app).cached = entry
        return result
    }

    public static func disconnect(_ app: AppState) throws {
        state(app).cached = nil
        try app.tryUpdateSettings { s in
            s.seerr.url = ""
            s.seerr.apiKey = ""
        }
    }

    private nonisolated static let DISCOVER: [String: String] = [
        "trending": "/discover/trending",
        "movies": "/discover/movies",
        "tv": "/discover/tv",
        "upcoming-movies": "/discover/movies/upcoming",
        "upcoming-tv": "/discover/tv/upcoming",
    ]

    /// A discover list: `trending`, `movies`, `tv`, `upcoming-movies` or `upcoming-tv`.
    public static func discover(_ app: AppState, _ list: String, _ page: Int) async throws -> SeerrPage<SeerrTitle> {
        guard let path = DISCOVER[list] else { throw SeerrError("Unknown list \(list).") }
        let got = try await current(app).client.discover(path, page: max(1, page))
        return SeerrPage(page: got.page, pages: got.totalPages, results: titlesFrom(got.results))
    }

    public static func search(_ app: AppState, _ query: String, _ page: Int) async throws -> SeerrPage<SeerrTitle> {
        let q = query.trimmingCharacters(in: .whitespacesAndNewlines)
        if q.isEmpty { return SeerrPage(page: 1, pages: 0, results: []) }
        let got = try await current(app).client.search(q, page: max(1, page))
        return SeerrPage(page: got.page, pages: got.totalPages, results: titlesFrom(got.results))
    }

    /// How long the Seerr title page waits for the Plex server to confirm its copy.
    public static let PLEX_CHECK: Duration = .seconds(5)

    /// Whether a Plex item is this TMDB title: by its TMDB guid (new or legacy agent), or,
    /// when it has no external ids, by kind and title.
    public nonisolated static func isTmdbTitle(_ m: Metadata, _ mediaType: SeerrMediaType, _ tmdbId: Int, _ title: String)
        -> Bool
    {
        if m.type != (mediaType == .tv ? "show" : "movie") { return false }
        let ids = m.guids.map { $0.id ?? "" } + [m.guid ?? ""]
        let tmdb = ids.compactMap { id in
            id.prefixMatch(of: #/(?:tmdb|com\.plexapp\.agents\.themoviedb):\/\/([0-9]+)/#).map { String($0.output.1) }
        }
        if !tmdb.isEmpty { return tmdb.contains(String(tmdbId)) }
        return m.guids.isEmpty && (m.title ?? "").lowercased() == title.lowercased()
    }

    /// Seerr's rating key names an item on the Plex server Seerr syncs with, which may not be
    /// the one Patina is connected to: keep it only if this server has the same title there.
    public static func plexCopy(_ server: PlexServer, _ t: SeerrTitle, wait: Duration = PLEX_CHECK) async -> String? {
        await plexCopy({ try await server.metadata($0, opts: .full()) }, t, wait: wait)
    }

    /// `plexCopy` with any way of fetching the item (tests).
    static func plexCopy(
        _ metadata: @escaping @MainActor (String) async throws -> Metadata?, _ t: SeerrTitle, wait: Duration = PLEX_CHECK
    ) async -> String? {
        guard let ratingKey = t.ratingKey, !ratingKey.isEmpty else { return nil }
        // Optional: a slow or unreachable Plex server must not hold up the Seerr page.
        let found = await Ops.Util.within(wait) { try? await metadata(ratingKey) }
        guard case let m?? = found else { return nil }
        return isTmdbTitle(m, t.mediaType, t.tmdbId, t.title) ? ratingKey : nil
    }

    /// `{ ...base, ...top }`: the fields `top` has win.
    private nonisolated static func overlay(_ base: RawMediaInfo?, _ top: RawMediaInfo?) -> RawMediaInfo {
        var out = base ?? RawMediaInfo()
        guard let top else { return out }
        if let v = top.id { out.id = v }
        if let v = top.tmdbId { out.tmdbId = v }
        if let v = top.tvdbId { out.tvdbId = v }
        if let v = top.status { out.status = v }
        if let v = top.status4k { out.status4k = v }
        if let v = top.ratingKey { out.ratingKey = v }
        if let v = top.ratingKey4k { out.ratingKey4k = v }
        if let v = top.requests { out.requests = v }
        if let v = top.seasons { out.seasons = v }
        if let v = top.mediaType { out.mediaType = v }
        return out
    }

    public static func details(_ app: AppState, _ mediaType: SeerrMediaType, _ tmdbId: Int) async throws -> SeerrDetails {
        let entry = try current(app)
        let user = try await me(entry)
        var title: SeerrTitle
        let info: RawMediaInfo?
        let tagline: String?
        let genres: [RawGenre]?
        let runtime: Int?
        var tv: RawTvDetails?
        if mediaType == .movie {
            let raw = try await entry.client.movie(tmdbId)
            title = titleFrom(mediaType, Fields(raw))
            (info, tagline, genres, runtime) = (raw.mediaInfo, raw.tagline, raw.genres, raw.runtime)
        } else {
            let raw = try await entry.client.tv(tmdbId)
            title = titleFrom(mediaType, Fields(raw))
            (info, tagline, genres, runtime) = (raw.mediaInfo, raw.tagline, raw.genres, raw.episodeRunTime?.first)
            tv = raw
        }
        remember(app, mediaType, tmdbId, Known(title))
        let known = Known(title)
        let requests = (info?.requests ?? []).map { r in
            var r = r
            r.type = r.type ?? mediaType
            var media = overlay(info, r.media)
            media.tmdbId = tmdbId
            r.media = media
            return requestFrom(r, user.id, known)
        }
        title.ratingKey = app.session() != nil ? await plexCopy(try app.server(), title) : nil
        return SeerrDetails(
            base: title, tagline: tagline ?? "", genres: (genres ?? []).map(\.name),
            runtime: runtime.flatMap { $0 == 0 ? nil : $0 }, tvdbId: tv?.externalIds?.tvdbId ?? info?.tvdbId,
            seasons: tv.map(seasonsFrom) ?? [], requests: sortedBy(requests, descending: true) { $0.createdAt ?? "" })
    }

    /// Make a request. Returns its state: `approved` when the user's requests are approved automatically.
    public static func request(_ app: AppState, _ r: NewRequest) async throws -> RequestState {
        if r.mediaType == .tv && (r.seasons ?? []).isEmpty { throw SeerrError("Choose at least one season.") }
        let made = try await current(app).client.createRequest(
            SeerrClient.RequestBody(
                mediaType: r.mediaType, mediaId: r.tmdbId, tvdbId: r.mediaType == .tv ? r.tvdbId : nil,
                seasons: r.mediaType == .tv ? r.seasons : nil, is4k: r.is4k, serverId: r.serverId,
                profileId: r.profileId, rootFolder: r.rootFolder))
        // Seerr answers 202 with only a message when nothing was left to request (say, someone else just did).
        guard let status = made?.status else {
            let why = made?.message ?? ""
            throw SeerrError(why.isEmpty ? "Seerr did not create the request." : why)
        }
        return requestState(status)
    }

    /// Radarr (movies) or Sonarr (shows) servers to send a request to, with their profiles and folders.
    public static func requestOptions(_ app: AppState, _ mediaType: SeerrMediaType, _ is4k: Bool) async throws
        -> [RequestOptionServer]
    {
        let client = try current(app).client
        let kind: SeerrClient.ServiceKind = mediaType == .movie ? .radarr : .sonarr
        let servers = try await client.services(kind).filter { $0.is4k == is4k }
        return try await Ops.Util.mapLimit(servers, max(servers.count, 1)) { s, _ in
            let d = try await client.service(kind, s.id)
            return RequestOptionServer(
                id: s.id, name: s.name, isDefault: s.isDefault, activeProfileId: d.server.activeProfileId ?? s.activeProfileId,
                activeDirectory: d.server.activeDirectory ?? s.activeDirectory, profiles: d.profiles,
                rootFolders: (d.rootFolders ?? []).map { RequestOptionServer.RootFolder(path: $0.path, freeSpace: $0.freeSpace) })
        }
    }

    public nonisolated static let REQUEST_FILTERS = ["all", "pending", "approved", "processing", "available", "unavailable", "failed"]
    nonisolated static let PAGE_SIZE = 20

    private static func remember(_ app: AppState, _ mediaType: SeerrMediaType, _ tmdbId: Int, _ t: Known) {
        let st = state(app)
        let key = "\(mediaType.rawValue):\(tmdbId)"
        st.order.removeAll { $0 == key }
        st.titles[key] = t
        st.order.append(key)
        if st.order.count > 1000 { st.titles[st.order.removeFirst()] = nil }
    }

    /// Requests by `filter` (see `REQUEST_FILTERS`), 20 a page; `mine` limits them to the user's own.
    public static func requests(_ app: AppState, _ filter: String, _ page: Int, mine: Bool = false) async throws
        -> SeerrPage<SeerrRequest>
    {
        if !REQUEST_FILTERS.contains(filter) { throw SeerrError("Unknown filter \(filter).") }
        let entry = try current(app)
        let user = try await me(entry)
        let p = max(1, page)
        let got = try await entry.client.requests(
            take: PAGE_SIZE, skip: (p - 1) * PAGE_SIZE, filter: filter, sort: "added", requestedBy: mine ? user.id : nil)
        let client = entry.client
        let results = try await Ops.Util.mapLimit(got.results, max(got.results.count, 1)) { r, _ in
            let mediaType = r.type ?? r.media?.mediaType ?? .movie
            let tmdbId = r.media?.tmdbId
            var known = tmdbId.flatMap { state(app).titles["\(mediaType.rawValue):\($0)"] }
            if known == nil, let tmdbId {
                do {
                    let title =
                        mediaType == .movie
                        ? titleFrom(mediaType, Fields(try await client.movie(tmdbId)))
                        : titleFrom(mediaType, Fields(try await client.tv(tmdbId)))
                    known = Known(title)
                    remember(app, mediaType, tmdbId, Known(title))
                } catch {
                    // Shown by its TMDB id instead.
                }
            }
            var r = r
            r.type = mediaType
            var out = requestFrom(r, user.id, known)
            if out.title.isEmpty { out.title = "TMDB \(tmdbId.map(String.init) ?? "?")" }
            return out
        }
        return SeerrPage(page: got.pageInfo.page, pages: got.pageInfo.pages, results: results)
    }

    /// `approve`, `decline`, `retry` or `delete` a request.
    public static func requestAction(_ app: AppState, _ id: Int, _ action: String) async throws {
        let client = try current(app).client
        switch action {
        case "approve": try await client.setRequestStatus(id, .approve)
        case "decline": try await client.setRequestStatus(id, .decline)
        case "retry": try await client.retryRequest(id)
        case "delete": try await client.deleteRequest(id)
        default: throw SeerrError("Unknown request action \(action).")
        }
    }

    /// The Seerr web page of a title.
    public static func pageUrl(_ app: AppState, _ mediaType: SeerrMediaType, _ tmdbId: Int) throws -> String {
        "\(try current(app).client.baseUrl)/\(mediaType == .tv ? "tv" : "movie")/\(tmdbId)"
    }

    /// Forget the client (tests, and a sign-out that changes the Plex token).
    public static func reset(_ app: AppState) {
        let st = state(app)
        st.cached = nil
        st.titles = [:]
        st.order = []
    }
}
