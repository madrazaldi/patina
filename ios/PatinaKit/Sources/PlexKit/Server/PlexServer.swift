// Port of desktop/plex/server/index.ts (and the `ServerBase` class of desktop/plex/server/mod.ts).
// The Plex Media Server client (port of crates/plex/src/server/mod.rs and its submodules).
// The endpoints are extensions in the other files of this folder.

import Foundation

/// Client bound to one Plex Media Server connection. Immutable; equal when the base URL and
/// token are (`PartialEq` in Rust).
public final class PlexServer: Sendable, Hashable, CustomStringConvertible {
    /// Shared HTTP plumbing (internal in Rust).
    public let http: Http
    /// The base URL, always ending in `/`.
    public let baseUrl: URL
    public let token: String
    public let machineIdentifier: String?

    /// `PlexServer::new(info, base_url, token)`.
    public convenience init(info: ClientInfo, baseUrl: String, token: String, session: URLSession? = nil) throws {
        try self.init(http: Http(info: info, session: session), baseUrl: baseUrl, token: token)
    }

    /// `PlexServer::with_http`: a client sharing the HTTP plumbing (and default headers) of another.
    public init(http: Http, baseUrl: String, token: String) throws {
        let parsed = try parseUrl(baseUrl)
        var c = URLComponents(url: parsed, resolvingAgainstBaseURL: true)
        if let path = c?.percentEncodedPath, !path.hasSuffix("/") { c?.percentEncodedPath = path + "/" }
        self.http = http
        self.baseUrl = c?.url ?? parsed
        self.token = token
        self.machineIdentifier = nil
    }

    private init(http: Http, baseUrl: URL, token: String, machineIdentifier: String?) {
        self.http = http
        self.baseUrl = baseUrl
        self.token = token
        self.machineIdentifier = machineIdentifier
    }

    public static func withHttp(_ http: Http, _ baseUrl: String, _ token: String) throws -> PlexServer {
        try PlexServer(http: http, baseUrl: baseUrl, token: token)
    }

    /// Returns a copy bound to the server's machine identifier.
    public func withMachineIdentifier(_ id: String?) -> PlexServer {
        PlexServer(http: http, baseUrl: baseUrl, token: token, machineIdentifier: id)
    }

    public var clientInfo: ClientInfo { http.info }

    public static func == (a: PlexServer, b: PlexServer) -> Bool {
        a.baseUrl.absoluteString == b.baseUrl.absoluteString && a.token == b.token
    }

    public func hash(into hasher: inout Hasher) {
        hasher.combine(baseUrl.absoluteString)
        hasher.combine(token)
    }

    /// Like the Rust `Debug` impl: the token is never included.
    public var description: String {
        "PlexServer(base: \(baseUrl.absoluteString), machineIdentifier: \(machineIdentifier ?? "nil"))"
    }
}
