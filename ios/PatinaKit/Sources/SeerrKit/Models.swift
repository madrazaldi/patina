// Port of desktop/seerr/models.ts.

// Seerr's JSON, as far as Patina reads it. Every field is optional: Seerr and
// TMDB leave many out, and Overseerr, Jellyseerr and Seerr differ slightly.
// The few the TypeScript requires default to 0, "", false or [] when missing, and
// a field of an unexpected type reads as missing (`De.swift`), so one odd field
// doesn't fail a whole response.

/// A movie or a show; what `type` and `mediaType` hold on requests and media.
public enum SeerrMediaType: String, Codable, Sendable, Hashable {
    case movie
    case tv
}

public struct RawUser: Codable, Sendable, Hashable {
    public var id: Int
    public var email: String?
    public var username: String?
    public var plexUsername: String?
    public var jellyfinUsername: String?
    public var displayName: String?
    public var permissions: Int?
    public var avatar: String?

    public init(
        id: Int = 0, email: String? = nil, username: String? = nil, plexUsername: String? = nil,
        jellyfinUsername: String? = nil, displayName: String? = nil, permissions: Int? = nil, avatar: String? = nil
    ) {
        self.id = id
        self.email = email
        self.username = username
        self.plexUsername = plexUsername
        self.jellyfinUsername = jellyfinUsername
        self.displayName = displayName
        self.permissions = permissions
        self.avatar = avatar
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = c.optI64(.id) ?? 0
        email = c.optString(.email)
        username = c.optString(.username)
        plexUsername = c.optString(.plexUsername)
        jellyfinUsername = c.optString(.jellyfinUsername)
        displayName = c.optString(.displayName)
        permissions = c.optI64(.permissions)
        avatar = c.optString(.avatar)
    }
}

public struct RawMediaInfo: Codable, Sendable, Hashable {
    public struct Season: Codable, Sendable, Hashable {
        public var seasonNumber: Int
        public var status: Int?
        public var status4k: Int?

        public init(seasonNumber: Int, status: Int? = nil, status4k: Int? = nil) {
            self.seasonNumber = seasonNumber
            self.status = status
            self.status4k = status4k
        }

        public init(from decoder: any Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            seasonNumber = c.optI64(.seasonNumber) ?? 0
            status = c.optI64(.status)
            status4k = c.optI64(.status4k)
        }
    }

    public var id: Int?
    public var tmdbId: Int?
    public var tvdbId: Int?
    public var status: Int?
    public var status4k: Int?
    /// Plex rating key of the copy on the server, once available.
    public var ratingKey: String?
    public var ratingKey4k: String?
    public var requests: [RawMediaRequest]?
    public var seasons: [Season]?
    /// Set on a request's `media` (the TypeScript's `RawMediaInfo & { mediaType }`).
    public var mediaType: SeerrMediaType?

    public init(
        id: Int? = nil, tmdbId: Int? = nil, tvdbId: Int? = nil, status: Int? = nil, status4k: Int? = nil,
        ratingKey: String? = nil, ratingKey4k: String? = nil, requests: [RawMediaRequest]? = nil,
        seasons: [Season]? = nil, mediaType: SeerrMediaType? = nil
    ) {
        self.id = id
        self.tmdbId = tmdbId
        self.tvdbId = tvdbId
        self.status = status
        self.status4k = status4k
        self.ratingKey = ratingKey
        self.ratingKey4k = ratingKey4k
        self.requests = requests
        self.seasons = seasons
        self.mediaType = mediaType
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = c.optI64(.id)
        tmdbId = c.optI64(.tmdbId)
        tvdbId = c.optI64(.tvdbId)
        status = c.optI64(.status)
        status4k = c.optI64(.status4k)
        ratingKey = c.optString(.ratingKey)
        ratingKey4k = c.optString(.ratingKey4k)
        requests = c.oneOrMany(.requests)
        seasons = c.oneOrMany(.seasons)
        mediaType = c.lenient(.mediaType)
    }
}

public struct RawMediaRequest: Codable, Sendable, Hashable {
    public struct Season: Codable, Sendable, Hashable {
        public var seasonNumber: Int
        public var status: Int?

        public init(seasonNumber: Int, status: Int? = nil) {
            self.seasonNumber = seasonNumber
            self.status = status
        }

        public init(from decoder: any Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            seasonNumber = c.optI64(.seasonNumber) ?? 0
            status = c.optI64(.status)
        }
    }

    public var id: Int
    /// Optional, unlike the TypeScript's: `POST /request` answers 202 with only a `message`
    /// when nothing was left to request, and the caller tells that answer by its missing status.
    public var status: Int?
    public var media: RawMediaInfo?
    public var type: SeerrMediaType?
    public var createdAt: String?
    public var updatedAt: String?
    public var requestedBy: RawUser?
    public var modifiedBy: RawUser?
    public var is4k: Bool?
    public var seasons: [Season]?
    /// Seerr's explanation, on the 202 above (the TypeScript reads it through a cast).
    public var message: String?

    public init(
        id: Int = 0, status: Int? = nil, media: RawMediaInfo? = nil, type: SeerrMediaType? = nil,
        createdAt: String? = nil, updatedAt: String? = nil, requestedBy: RawUser? = nil, modifiedBy: RawUser? = nil,
        is4k: Bool? = nil, seasons: [Season]? = nil, message: String? = nil
    ) {
        self.id = id
        self.status = status
        self.media = media
        self.type = type
        self.createdAt = createdAt
        self.updatedAt = updatedAt
        self.requestedBy = requestedBy
        self.modifiedBy = modifiedBy
        self.is4k = is4k
        self.seasons = seasons
        self.message = message
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = c.optI64(.id) ?? 0
        status = c.optI64(.status)
        media = c.lenient(.media)
        type = c.lenient(.type)
        createdAt = c.optString(.createdAt)
        updatedAt = c.optString(.updatedAt)
        requestedBy = c.lenient(.requestedBy)
        modifiedBy = c.lenient(.modifiedBy)
        is4k = c.boolean(.is4k)
        seasons = c.oneOrMany(.seasons)
        message = c.optString(.message)
    }
}

public struct RawResult: Codable, Sendable, Hashable {
    public var id: Int
    /// `movie`, `tv`, `person` or `collection`.
    public var mediaType: String
    public var title: String?
    public var name: String?
    public var releaseDate: String?
    public var firstAirDate: String?
    public var overview: String?
    public var posterPath: String?
    public var backdropPath: String?
    public var voteAverage: Double?
    public var mediaInfo: RawMediaInfo?

    public init(
        id: Int = 0, mediaType: String = "", title: String? = nil, name: String? = nil, releaseDate: String? = nil,
        firstAirDate: String? = nil, overview: String? = nil, posterPath: String? = nil, backdropPath: String? = nil,
        voteAverage: Double? = nil, mediaInfo: RawMediaInfo? = nil
    ) {
        self.id = id
        self.mediaType = mediaType
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

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = c.optI64(.id) ?? 0
        mediaType = c.optString(.mediaType) ?? ""
        title = c.optString(.title)
        name = c.optString(.name)
        releaseDate = c.optString(.releaseDate)
        firstAirDate = c.optString(.firstAirDate)
        overview = c.optString(.overview)
        posterPath = c.optString(.posterPath)
        backdropPath = c.optString(.backdropPath)
        voteAverage = c.optF64(.voteAverage)
        mediaInfo = c.lenient(.mediaInfo)
    }
}

public struct RawPage<T: Codable & Sendable & Hashable>: Codable, Sendable, Hashable {
    public var page: Int
    public var totalPages: Int
    public var totalResults: Int?
    public var results: [T]

    public init(page: Int = 1, totalPages: Int = 0, totalResults: Int? = nil, results: [T] = []) {
        self.page = page
        self.totalPages = totalPages
        self.totalResults = totalResults
        self.results = results
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        page = c.optI64(.page) ?? 0
        totalPages = c.optI64(.totalPages) ?? 0
        totalResults = c.optI64(.totalResults)
        results = c.oneOrMany(.results) ?? []
    }
}

public struct RawRequestPage: Codable, Sendable, Hashable {
    public struct PageInfo: Codable, Sendable, Hashable {
        public var page: Int
        public var pages: Int
        public var results: Int
        public var pageSize: Int?

        public init(page: Int = 1, pages: Int = 0, results: Int = 0, pageSize: Int? = nil) {
            self.page = page
            self.pages = pages
            self.results = results
            self.pageSize = pageSize
        }

        public init(from decoder: any Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            page = c.optI64(.page) ?? 0
            pages = c.optI64(.pages) ?? 0
            results = c.optI64(.results) ?? 0
            pageSize = c.optI64(.pageSize)
        }
    }

    public var pageInfo: PageInfo
    public var results: [RawMediaRequest]

    public init(pageInfo: PageInfo = PageInfo(), results: [RawMediaRequest] = []) {
        self.pageInfo = pageInfo
        self.results = results
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        pageInfo = c.lenient(.pageInfo) ?? PageInfo(page: 0)
        results = c.oneOrMany(.results) ?? []
    }
}

public struct RawGenre: Codable, Sendable, Hashable {
    public var id: Int
    public var name: String

    public init(id: Int = 0, name: String = "") {
        self.id = id
        self.name = name
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = c.optI64(.id) ?? 0
        name = c.optString(.name) ?? ""
    }
}

/// What movie and show details share (the TypeScript's `RawDetails`, which they extend).
public protocol RawDetails: Codable, Sendable, Hashable {
    var id: Int { get }
    var overview: String? { get }
    var posterPath: String? { get }
    var backdropPath: String? { get }
    var voteAverage: Double? { get }
    var genres: [RawGenre]? { get }
    var tagline: String? { get }
    var status: String? { get }
    var mediaInfo: RawMediaInfo? { get }
}

public struct RawMovieDetails: RawDetails {
    public var id: Int
    public var overview: String?
    public var posterPath: String?
    public var backdropPath: String?
    public var voteAverage: Double?
    public var genres: [RawGenre]?
    public var tagline: String?
    public var status: String?
    public var mediaInfo: RawMediaInfo?
    public var title: String?
    public var releaseDate: String?
    public var runtime: Int?

    public init(
        id: Int = 0, overview: String? = nil, posterPath: String? = nil, backdropPath: String? = nil,
        voteAverage: Double? = nil, genres: [RawGenre]? = nil, tagline: String? = nil, status: String? = nil,
        mediaInfo: RawMediaInfo? = nil, title: String? = nil, releaseDate: String? = nil, runtime: Int? = nil
    ) {
        self.id = id
        self.overview = overview
        self.posterPath = posterPath
        self.backdropPath = backdropPath
        self.voteAverage = voteAverage
        self.genres = genres
        self.tagline = tagline
        self.status = status
        self.mediaInfo = mediaInfo
        self.title = title
        self.releaseDate = releaseDate
        self.runtime = runtime
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = c.optI64(.id) ?? 0
        overview = c.optString(.overview)
        posterPath = c.optString(.posterPath)
        backdropPath = c.optString(.backdropPath)
        voteAverage = c.optF64(.voteAverage)
        genres = c.oneOrMany(.genres)
        tagline = c.optString(.tagline)
        status = c.optString(.status)
        mediaInfo = c.lenient(.mediaInfo)
        title = c.optString(.title)
        releaseDate = c.optString(.releaseDate)
        runtime = c.optI64(.runtime)
    }
}

public struct RawTvDetails: RawDetails {
    public struct Season: Codable, Sendable, Hashable {
        public var seasonNumber: Int
        public var name: String?
        public var episodeCount: Int?
        public var airDate: String?

        public init(seasonNumber: Int, name: String? = nil, episodeCount: Int? = nil, airDate: String? = nil) {
            self.seasonNumber = seasonNumber
            self.name = name
            self.episodeCount = episodeCount
            self.airDate = airDate
        }

        public init(from decoder: any Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            seasonNumber = c.optI64(.seasonNumber) ?? 0
            name = c.optString(.name)
            episodeCount = c.optI64(.episodeCount)
            airDate = c.optString(.airDate)
        }
    }

    public struct ExternalIds: Codable, Sendable, Hashable {
        public var tvdbId: Int?

        public init(tvdbId: Int? = nil) {
            self.tvdbId = tvdbId
        }

        public init(from decoder: any Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            tvdbId = c.optI64(.tvdbId)
        }
    }

    public var id: Int
    public var overview: String?
    public var posterPath: String?
    public var backdropPath: String?
    public var voteAverage: Double?
    public var genres: [RawGenre]?
    public var tagline: String?
    public var status: String?
    public var mediaInfo: RawMediaInfo?
    public var name: String?
    public var firstAirDate: String?
    public var episodeRunTime: [Int]?
    public var numberOfSeasons: Int?
    public var seasons: [Season]?
    public var externalIds: ExternalIds?

    public init(
        id: Int = 0, overview: String? = nil, posterPath: String? = nil, backdropPath: String? = nil,
        voteAverage: Double? = nil, genres: [RawGenre]? = nil, tagline: String? = nil, status: String? = nil,
        mediaInfo: RawMediaInfo? = nil, name: String? = nil, firstAirDate: String? = nil,
        episodeRunTime: [Int]? = nil, numberOfSeasons: Int? = nil, seasons: [Season]? = nil,
        externalIds: ExternalIds? = nil
    ) {
        self.id = id
        self.overview = overview
        self.posterPath = posterPath
        self.backdropPath = backdropPath
        self.voteAverage = voteAverage
        self.genres = genres
        self.tagline = tagline
        self.status = status
        self.mediaInfo = mediaInfo
        self.name = name
        self.firstAirDate = firstAirDate
        self.episodeRunTime = episodeRunTime
        self.numberOfSeasons = numberOfSeasons
        self.seasons = seasons
        self.externalIds = externalIds
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = c.optI64(.id) ?? 0
        overview = c.optString(.overview)
        posterPath = c.optString(.posterPath)
        backdropPath = c.optString(.backdropPath)
        voteAverage = c.optF64(.voteAverage)
        genres = c.oneOrMany(.genres)
        tagline = c.optString(.tagline)
        status = c.optString(.status)
        mediaInfo = c.lenient(.mediaInfo)
        name = c.optString(.name)
        firstAirDate = c.optString(.firstAirDate)
        episodeRunTime = c.oneOrMany(.episodeRunTime)
        numberOfSeasons = c.optI64(.numberOfSeasons)
        seasons = c.oneOrMany(.seasons)
        externalIds = c.lenient(.externalIds)
    }
}

public struct RawService: Codable, Sendable, Hashable {
    public var id: Int
    public var name: String
    public var is4k: Bool
    public var isDefault: Bool
    public var activeProfileId: Int?
    public var activeDirectory: String?

    public init(
        id: Int = 0, name: String = "", is4k: Bool = false, isDefault: Bool = false,
        activeProfileId: Int? = nil, activeDirectory: String? = nil
    ) {
        self.id = id
        self.name = name
        self.is4k = is4k
        self.isDefault = isDefault
        self.activeProfileId = activeProfileId
        self.activeDirectory = activeDirectory
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = c.optI64(.id) ?? 0
        name = c.optString(.name) ?? ""
        is4k = c.boolean(.is4k) ?? false
        isDefault = c.boolean(.isDefault) ?? false
        activeProfileId = c.optI64(.activeProfileId)
        activeDirectory = c.optString(.activeDirectory)
    }
}

public struct RawServiceDetails: Codable, Sendable, Hashable {
    public struct Profile: Codable, Sendable, Hashable {
        public var id: Int
        public var name: String

        public init(id: Int = 0, name: String = "") {
            self.id = id
            self.name = name
        }

        public init(from decoder: any Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            id = c.optI64(.id) ?? 0
            name = c.optString(.name) ?? ""
        }
    }

    public struct RootFolder: Codable, Sendable, Hashable {
        public var id: Int
        public var path: String
        public var freeSpace: Int?

        public init(id: Int = 0, path: String = "", freeSpace: Int? = nil) {
            self.id = id
            self.path = path
            self.freeSpace = freeSpace
        }

        public init(from decoder: any Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            id = c.optI64(.id) ?? 0
            path = c.optString(.path) ?? ""
            freeSpace = c.optI64(.freeSpace)
        }
    }

    public var server: RawService
    public var profiles: [Profile]
    public var rootFolders: [RootFolder]?

    public init(server: RawService = RawService(), profiles: [Profile] = [], rootFolders: [RootFolder]? = nil) {
        self.server = server
        self.profiles = profiles
        self.rootFolders = rootFolders
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        server = c.lenient(.server) ?? RawService()
        profiles = c.oneOrMany(.profiles) ?? []
        rootFolders = c.oneOrMany(.rootFolders)
    }
}
