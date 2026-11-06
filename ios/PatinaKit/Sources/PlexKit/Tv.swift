// Port of desktop/plex/tv.ts.
// plex.tv account client (port of crates/plex/src/tv.rs).

import Foundation

private let plexTvBase = "https://plex.tv"
private let clientsBase = "https://clients.plex.tv"
private let discoverBase = "https://discover.provider.plex.tv"
private let metadataBase = "https://metadata.provider.plex.tv"

/// Debug builds: `PATINA_PLEX_TV_URL` points the account and resource endpoints at
/// a mock server (tools/mock_pms.py), so multi-server features can be tried offline.
/// Only honoured when `PATINA_DEBUG_BUILD=1`, as on the desktop.
private func accountBase(_ def: String) -> String {
    let env = ProcessInfo.processInfo.environment
    if env["PATINA_DEBUG_BUILD"] == "1", let url = env["PATINA_PLEX_TV_URL"] { return url }
    return def
}

/// `Url::parse(base)?.join(path)?`
private func joinUrl(_ base: String, _ path: String) throws -> URL {
    try parseUrl(path, base: try parseUrl(base))
}

/// Client for plex.tv account services.
public final class PlexTv: Sendable {
    /// Shared HTTP plumbing (used by `connectResource` to bind servers to the same client).
    public let http: Http
    public let token: String?

    public init(info: ClientInfo, token: String? = nil, http: Http? = nil) {
        self.http = http ?? Http(info: info)
        self.token = token
    }

    public func withToken(_ token: String) -> PlexTv {
        PlexTv(info: http.info, token: token, http: http)
    }

    public var clientInfo: ClientInfo { http.info }

    private func get<T>(_ base: String, _ path: String, _ params: Params, _ normalize: (JSONValue) throws -> T)
        async throws -> T
    {
        try await http.json(http.request("GET", try joinUrl(base, path), token: token, params), normalize)
    }

    /// Escape hatch: call any plex.tv endpoint and get raw JSON back.
    public func raw(_ method: String, _ url: String, _ params: Params = Params()) async throws -> JSONValue {
        try await http.json(http.request(method, try parseUrl(url), token: token, params))
    }

    // ----- Sign-in -------------------------------------------------------

    /// Create a PIN for the link flow. `strong` PINs are long and meant to be
    /// embedded in the app.plex.tv auth URL rather than typed.
    public func createPin(_ strong: Bool) async throws -> Pin {
        let rb = http.request("POST", try joinUrl(plexTvBase, "/api/v2/pins"), token: nil, Params().flag("strong", strong))
        return try await http.json(rb, Pin.init(json:))
    }

    /// Poll a PIN; `authToken` is set once the user approved it.
    public func checkPin(_ id: Int) async throws -> Pin {
        let rb = http.request("GET", try joinUrl(plexTvBase, "/api/v2/pins/\(id)"), token: nil, Params())
        return try await http.json(rb, Pin.init(json:))
    }

    /// URL the user opens in a browser to approve a strong PIN.
    public func authUrl(_ code: String) -> String {
        let info = http.info
        let query = Params()
            .set("clientID", info.identifier)
            .set("code", code)
            .set("context[device][product]", info.product)
            .set("context[device][platform]", info.platform)
            .set("context[device][device]", info.device)
            .set("context[device][deviceName]", info.deviceName)
            .set("context[device][version]", info.version)
            .toQueryString()
        return "https://app.plex.tv/auth#?\(query)"
    }

    /// Username/password sign-in. `code` is the 2FA verification code, if enabled.
    public func signIn(_ login: String, password: String, code: String? = nil) async throws -> User {
        var form = [("login", login), ("password", password), ("rememberMe", "true")]
        if let code { form.append(("verificationCode", code)) }
        let rb = http.request("POST", try joinUrl(plexTvBase, "/api/v2/users/signin"), token: nil, Params()).form(form)
        return try await http.json(rb, User.init(json:))
    }

    /// Validates the current token and returns the account.
    public func user() async throws -> User {
        try await get(accountBase(plexTvBase), "/api/v2/user", Params(), User.init(json:))
    }

    // ----- Resources -----------------------------------------------------

    /// All devices the account can access, including servers with their connections.
    public func resources() async throws -> [Resource] {
        let params = Params().set("includeHttps", 1).set("includeRelay", 1).set("includeIPv6", 1)
        return try await get(accountBase(clientsBase), "/api/v2/resources", params) {
            try De.list($0, Resource.init(json:))
        }
    }

    public func servers() async throws -> [Resource] {
        try await resources().filter(Resource.isServer)
    }

    // ----- Plex Home -----------------------------------------------------

    public func homeUsers() async throws -> [HomeUser] {
        let v = try await get(clientsBase, "/api/v2/home/users", Params()) { $0 }
        let items: JSONValue
        switch v {
        case .array: items = v
        case .object(let o): items = o["users"] ?? .array([])
        default: items = .array([])
        }
        do {
            return try De.list(items, HomeUser.init(json:))
        } catch let e as DecodeFailure {
            throw PlexError.other("home users: \(e.detail)")
        }
    }

    /// Switch to another Plex Home user. Returns that user with a fresh `authToken`.
    public func switchHomeUser(_ uuid: String, pin: String? = nil) async throws -> User {
        let rb = http.request(
            "POST", try joinUrl(clientsBase, "/api/v2/home/users/\(uuid)/switch"), token: token, Params().opt("pin", pin))
        return try await http.json(rb, User.init(json:))
    }

    public func friends() async throws -> [Friend] {
        try await get(plexTvBase, "/api/v2/friends", Params()) { try De.list($0, Friend.init(json:)) }
    }

    // ----- Discover & watchlist -----------------------------------------

    /// The account watchlist. Items carry discover ids in `ratingKey`.
    public func watchlist(_ start: Int, size: Int) async throws -> MediaContainer {
        let params = Params()
            .set("includeCollections", 1)
            .set("includeExternalMedia", 1)
            .set("sort", "watchlistedAt:desc")
            .set("X-Plex-Container-Start", start)
            .set("X-Plex-Container-Size", size)
        return try await get(discoverBase, "/library/sections/watchlist/all", params, Envelope.init(json:))
            .mediaContainer
    }

    public func addToWatchlist(_ discoverId: String) async throws {
        try await watchlistAction("addToWatchlist", discoverId)
    }

    public func removeFromWatchlist(_ discoverId: String) async throws {
        try await watchlistAction("removeFromWatchlist", discoverId)
    }

    private func watchlistAction(_ action: String, _ discoverId: String) async throws {
        let rb = http.request(
            "PUT", try joinUrl(discoverBase, "/actions/\(action)"), token: token, Params().set("ratingKey", discoverId))
        try await http.empty(rb)
    }

    /// Watchlist/view state for a discover item.
    public func userState(_ discoverId: String) async throws -> UserState? {
        let v = try await get(discoverBase, "/library/metadata/\(discoverId)/userState", Params()) { $0 }
        guard var state = De.pointer(v, "/MediaContainer/UserState") else { return nil }
        if case .array(let a) = state { state = a.last ?? .null }
        return try? UserState(json: state)
    }

    /// Full discover metadata (cast, reviews, ratings) for a discover id.
    public func discoverMetadata(_ discoverId: String) async throws -> Metadata? {
        try await get(metadataBase, "/library/metadata/\(discoverId)", Params(), Envelope.init(json:))
            .mediaContainer.metadata.first
    }

    /// Search Plex's global catalogue (movies & shows not necessarily on any server).
    public func discoverSearch(_ query: String, limit: Int) async throws -> [Metadata] {
        let params = Params()
            .set("query", query)
            .set("limit", limit)
            .set("searchTypes", "movies,tv")
            .set("searchProviders", "discover")
            .set("includeMetadata", 1)
        let v = try await get(discoverBase, "/library/search", params) { $0 }
        var out: [Metadata] = []
        guard case .array(let groups)? = De.pointer(v, "/MediaContainer/SearchResults") else { return out }
        for group in groups {
            guard case .array(let results)? = group["SearchResult"] else { continue }
            for result in results {
                // `if let Ok(m) = from_value`: entries that do not decode are skipped
                if let m = result["Metadata"], let item = try? Metadata(json: m) { out.append(item) }
            }
        }
        return out
    }
}
