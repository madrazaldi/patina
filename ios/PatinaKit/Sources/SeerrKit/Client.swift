// Port of desktop/seerr/client.ts.

// Client of the Seerr API (`/api/v1`, https://docs.seerr.dev/api/seerr-api/),
// which Overseerr and Jellyseerr share. It signs in with an API key
// (`X-Api-Key`), or with the Plex account token (`POST /auth/plex`), whose
// session cookie it keeps and renews when Seerr forgets it.

import Foundation
import Synchronization

/// Requests give up after this long.
let timeout: TimeInterval = 30

public enum SeerrAuth: Sendable, Hashable {
    case apiKey(String)
    case plexToken(String)
}

public struct SeerrError: Error, LocalizedError, CustomStringConvertible, Sendable, Hashable {
    public let message: String
    /// HTTP status, when Seerr answered.
    public let status: Int?

    public init(_ message: String, status: Int? = nil) {
        self.message = message
        self.status = status
    }

    public var errorDescription: String? { message }
    public var description: String { message }
}

/// The base URL as typed by the user, without a trailing slash or `/api/v1`.
/// A missing scheme means http, as Seerr usually runs on a LAN port.
public func normalizeUrl(_ input: String) throws(SeerrError) -> String {
    var url = input.trimmingCharacters(in: .whitespacesAndNewlines)
    if url.isEmpty { throw SeerrError("Enter the address of your Seerr server.") }
    if url.range(of: "^[a-z][a-z0-9+.-]*://", options: [.regularExpression, .caseInsensitive]) == nil { url = "http://\(url)" }
    let invalid = SeerrError("“\(input.trimmingCharacters(in: .whitespacesAndNewlines))” is not a valid address.")
    guard var parsed = URLComponents(string: url), let scheme = parsed.scheme?.lowercased() else { throw invalid }
    if scheme != "http" && scheme != "https" { throw SeerrError("The address must start with http:// or https://.") }
    guard let host = parsed.percentEncodedHost, !host.isEmpty else { throw invalid }
    // As the WHATWG URL serializes it: lowercase scheme and host, no default port.
    parsed.scheme = scheme
    parsed.percentEncodedHost = host.lowercased()
    if parsed.port == (scheme == "http" ? 80 : 443) { parsed.port = nil }
    parsed.query = nil
    parsed.fragment = nil
    guard var out = parsed.string else { throw invalid }
    while out.hasSuffix("/") { out.removeLast() }
    if out.hasSuffix("/api/v1") { out.removeLast("/api/v1".count) }
    return out
}

/// Seerr validates queries against its OpenAPI schema and rejects some characters
/// unless they are percent-encoded, so encode everything but `A-Z a-z 0-9 - _ . ~`.
public func encodeQuery(_ params: KeyValuePairs<String, (any CustomStringConvertible)?>) -> String {
    params.compactMap { k, v in v.map { "\(enc(k))=\(enc($0.description))" } }.joined(separator: "&")
}

private let unreserved = CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.~")

private func enc(_ v: String) -> String {
    v.addingPercentEncoding(withAllowedCharacters: unreserved) ?? v
}

/// Version of the server (`GET /status`).
public struct RawStatus: Codable, Sendable, Hashable {
    public var version: String?
    public var commitTag: String?

    public init(version: String? = nil, commitTag: String? = nil) {
        self.version = version
        self.commitTag = commitTag
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        version = c.optString(.version)
        commitTag = c.optString(.commitTag)
    }
}

public final class SeerrClient: Sendable {
    /// How requests are signed in.
    public enum AuthKind: String, Codable, Sendable, Hashable {
        case apiKey
        case plex
    }

    /// Radarr (movies) or Sonarr (shows).
    public enum ServiceKind: String, Codable, Sendable, Hashable {
        case radarr
        case sonarr
    }

    /// What `setRequestStatus` does to a pending request.
    public enum RequestStatus: String, Codable, Sendable, Hashable {
        case approve
        case decline
    }

    /// The body of `POST /request`; nil fields are left out.
    public struct RequestBody: Codable, Sendable, Hashable {
        public var mediaType: SeerrMediaType
        public var mediaId: Int
        public var tvdbId: Int?
        public var seasons: [Int]?
        public var is4k: Bool
        public var serverId: Int?
        public var profileId: Int?
        public var rootFolder: String?

        public init(
            mediaType: SeerrMediaType, mediaId: Int, tvdbId: Int? = nil, seasons: [Int]? = nil, is4k: Bool,
            serverId: Int? = nil, profileId: Int? = nil, rootFolder: String? = nil
        ) {
            self.mediaType = mediaType
            self.mediaId = mediaId
            self.tvdbId = tvdbId
            self.seasons = seasons
            self.is4k = is4k
            self.serverId = serverId
            self.profileId = profileId
            self.rootFolder = rootFolder
        }
    }

    private struct State {
        /// `connect.sid=...` after a Plex sign-in.
        var cookie: String?
        var signingIn: Task<Void, any Error>?
    }

    public let baseUrl: String
    private let auth: SeerrAuth
    private let session: URLSession
    private let state = Mutex(State())

    /// `configuration` is for tests (a stub `URLProtocol`); the client turns off its
    /// cookie handling and caching either way, and carries the session cookie itself.
    public init(_ url: String, auth: SeerrAuth, configuration: URLSessionConfiguration = .ephemeral) throws(SeerrError) {
        baseUrl = try normalizeUrl(url)
        self.auth = auth
        let config = configuration.copy() as! URLSessionConfiguration
        config.httpCookieStorage = nil
        config.httpShouldSetCookies = false
        config.httpCookieAcceptPolicy = .never
        config.urlCache = nil
        config.requestCachePolicy = .reloadIgnoringLocalCacheData
        config.timeoutIntervalForRequest = timeout
        config.timeoutIntervalForResource = timeout
        session = URLSession(configuration: config, delegate: NoRedirects(), delegateQueue: nil)
    }

    deinit {
        session.finishTasksAndInvalidate()
    }

    /// How requests are signed in.
    public var authKind: AuthKind {
        if case .apiKey = auth { .apiKey } else { .plex }
    }

    /// Version of the server; needs no sign-in.
    public func status() async throws -> RawStatus {
        try await request("GET", "/status", authed: false)
    }

    /// The signed-in user.
    public func me() async throws -> RawUser {
        try await request("GET", "/auth/me")
    }

    public func search(_ query: String, page: Int = 1) async throws -> RawPage<RawResult> {
        try await request("GET", "/search", query: encodeQuery(["query": query, "page": page]))
    }

    /// `path` is a discover list such as `/discover/trending` or `/discover/movies/upcoming`.
    public func discover(_ path: String, page: Int = 1) async throws -> RawPage<RawResult> {
        try await request("GET", path, query: encodeQuery(["page": page]))
    }

    public func movie(_ tmdbId: Int) async throws -> RawMovieDetails {
        try await request("GET", "/movie/\(tmdbId)")
    }

    public func tv(_ tmdbId: Int) async throws -> RawTvDetails {
        try await request("GET", "/tv/\(tmdbId)")
    }

    public func requests(take: Int, skip: Int, filter: String, sort: String, requestedBy: Int? = nil) async throws -> RawRequestPage {
        let query = encodeQuery(["take": take, "skip": skip, "filter": filter, "sort": sort, "requestedBy": requestedBy])
        return try await request("GET", "/request", query: query)
    }

    /// nil when Seerr answers with an empty body. When nothing was left to request it
    /// answers 202 with only a `message`, which reads as a request without a `status`.
    public func createRequest(_ body: RequestBody) async throws -> RawMediaRequest? {
        try await requestOrNothing("POST", "/request", body: json(body))
    }

    /// `approve` or `decline` a pending request.
    @discardableResult
    public func setRequestStatus(_ id: Int, _ status: RequestStatus) async throws -> RawMediaRequest {
        try await request("POST", "/request/\(id)/\(status.rawValue)")
    }

    @discardableResult
    public func retryRequest(_ id: Int) async throws -> RawMediaRequest {
        try await request("POST", "/request/\(id)/retry")
    }

    public func deleteRequest(_ id: Int) async throws {
        let _: Ignored? = try await requestOrNothing("DELETE", "/request/\(id)")
    }

    /// Radarr (`radarr`) or Sonarr (`sonarr`) servers set up in Seerr.
    public func services(_ kind: ServiceKind) async throws -> [RawService] {
        let list: [Lenient<RawService>] = try await request("GET", "/service/\(kind.rawValue)")
        return list.compactMap(\.value)
    }

    /// Quality profiles and root folders of one Radarr or Sonarr server.
    public func service(_ kind: ServiceKind, _ id: Int) async throws -> RawServiceDetails {
        try await request("GET", "/service/\(kind.rawValue)/\(id)")
    }

    /// The session cookie of a Plex sign-in; nil with an API key.
    private func signIn() async throws -> String? {
        guard case .plexToken(let token) = auth else { return nil }
        let (data, resp) = try await send("POST", "/auth/plex", "", json(["authToken": token]), nil)
        if !(200..<300).contains(resp.statusCode) { throw failure(data, resp, "POST", "/auth/plex") }
        let fields = resp.allHeaderFields.reduce(into: [String: String]()) { out, field in
            if let k = field.key as? String, let v = field.value as? String { out[k] = v }
        }
        let cookies = HTTPCookie.cookies(withResponseHeaderFields: fields, for: resp.url ?? URL(string: baseUrl)!)
        guard let sid = cookies.first(where: { $0.name == "connect.sid" }) else {
            throw SeerrError("Seerr accepted the Plex sign-in but sent no session.")
        }
        return "connect.sid=\(sid.value)"
    }

    /// Sign in once, however many requests are waiting for it. `stale` is a session
    /// Seerr refused: it is forgotten first, unless another sign-in already replaced it.
    private func ensureSignedIn(forgetting stale: String? = nil) async throws {
        let task: Task<Void, any Error>? = state.withLock { s in
            if let stale, s.cookie == stale { s.cookie = nil }
            if s.cookie != nil { return nil }
            if let running = s.signingIn { return running }
            let task = Task {
                do {
                    let cookie = try await self.signIn()
                    self.state.withLock { $0.cookie = cookie; $0.signingIn = nil }
                } catch {
                    self.state.withLock { $0.signingIn = nil }
                    throw error
                }
            }
            s.signingIn = task
            return task
        }
        try await task?.value
    }

    private func request<T: Decodable>(_ method: String, _ path: String, query: String = "", body: Data? = nil, authed: Bool = true) async throws -> T {
        guard let value: T = try await requestOrNothing(method, path, query: query, body: body, authed: authed) else {
            throw SeerrError("\(baseUrl) did not answer like a Seerr server.")
        }
        return value
    }

    /// nil for an empty body.
    private func requestOrNothing<T: Decodable>(_ method: String, _ path: String, query: String = "", body: Data? = nil, authed: Bool = true) async throws -> T? {
        var plex = false
        if authed, case .plexToken = auth { plex = true }
        if plex && state.withLock({ $0.cookie }) == nil { try await ensureSignedIn() }
        let sent = authed ? state.withLock { $0.cookie } : nil
        var (data, resp) = try await send(method, path, query, body, sent)
        // Seerr restarted or the session expired: sign in again (once for all requests that noticed), and retry once.
        if plex, let sent, resp.statusCode == 401 || resp.statusCode == 403 {
            try await ensureSignedIn(forgetting: sent)
            (data, resp) = try await send(method, path, query, body, state.withLock { $0.cookie })
        }
        if !(200..<300).contains(resp.statusCode) { throw failure(data, resp, method, path) }
        if String(decoding: data, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { return nil }
        do {
            return try JSONDecoder().decode(T.self, from: data)
        } catch {
            throw SeerrError("\(baseUrl) did not answer like a Seerr server.")
        }
    }

    private func send(_ method: String, _ path: String, _ query: String, _ body: Data?, _ cookie: String?) async throws -> (Data, HTTPURLResponse) {
        let address = "\(baseUrl)/api/v1\(path)\(query.isEmpty ? "" : "?\(query)")"
        guard let url = URL(string: address) else { throw SeerrError("“\(address)” is not a valid address.") }
        var req = URLRequest(url: url, timeoutInterval: timeout)
        req.httpMethod = method
        req.httpShouldHandleCookies = false
        req.setValue("application/json", forHTTPHeaderField: "Accept")
        if case .apiKey(let key) = auth { req.setValue(key, forHTTPHeaderField: "X-Api-Key") }
        if let cookie { req.setValue(cookie, forHTTPHeaderField: "Cookie") }
        if let body {
            req.setValue("application/json", forHTTPHeaderField: "Content-Type")
            req.httpBody = body
        }
        do {
            let (data, resp) = try await session.data(for: req)
            guard let http = resp as? HTTPURLResponse else {
                throw SeerrError("Could not reach Seerr at \(baseUrl): not an HTTP response")
            }
            return (data, http)
        } catch let e as SeerrError {
            throw e
        } catch {
            if Task.isCancelled { throw CancellationError() }
            let why = (error as? URLError)?.code == .timedOut ? "timed out" : error.localizedDescription
            throw SeerrError("Could not reach Seerr at \(baseUrl): \(why)")
        }
    }

    private func json(_ value: some Encodable) throws -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = .withoutEscapingSlashes
        return try encoder.encode(value)
    }
}

/// The error of a failed response, with Seerr's own message when it sent one.
private func failure(_ data: Data, _ resp: HTTPURLResponse, _ method: String, _ path: String) -> SeerrError {
    let status = resp.statusCode
    // Not JSON: the status says enough.
    let detail = ((try? JSONSerialization.jsonObject(with: data)) as? [String: Any])?["message"] as? String ?? ""
    // Seerr answers a bad key or session with 403 too, so a refused sign-in check is a sign-in problem.
    if status == 401 || (status == 403 && path.hasPrefix("/auth/")) {
        return SeerrError("Seerr did not accept the sign-in. Check the API key, or that your Plex account has access.", status: status)
    }
    if status == 403 { return SeerrError(detail.isEmpty ? "Seerr refused \(method) \(path)." : "Seerr refused: \(detail)", status: status) }
    if (300..<400).contains(status) { return SeerrError("Seerr redirected \(method) \(path); check the address.", status: status) }
    return SeerrError(detail.isEmpty ? "\(method) \(path) failed with HTTP \(status)" : detail, status: status)
}

/// Hands redirects back as responses, as `fetch` does with `redirect: "manual"`.
private final class NoRedirects: NSObject, URLSessionTaskDelegate, Sendable {
    func urlSession(
        _ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
        newRequest request: URLRequest
    ) async -> URLRequest? {
        nil
    }
}

/// A body that is read (so it must be JSON) but not kept.
private struct Ignored: Decodable {
    init(from decoder: any Decoder) {}
}
