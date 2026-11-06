// Port of desktop/plex/server/mod.ts.
// Client bound to one Plex Media Server connection (port of crates/plex/src/server/mod.rs).
//
// The Rust `impl PlexServer` is spread over several files; here each file adds its endpoints
// in an extension of `PlexServer` (`PlexServer.swift`). This file holds the connection, URL
// building and request helpers.

import Foundation

/// 24 hours, the timeout of media downloads.
let downloadTimeout: TimeInterval = 60 * 60 * 24

extension PlexServer {
    /// Resolve a server path (`/library/...`) or absolute URL.
    public func url(_ path: String) throws -> URL {
        if path.hasPrefix("http://") || path.hasPrefix("https://") { return try parseUrl(path) }
        // "./" keeps a colon in the first segment (`/tv.plex.providers.epg.cloud:2/...`) from reading as a scheme.
        let relative = path.drop(while: { $0 == "/" })
        return try parseUrl("./\(relative)", base: baseUrl)
    }

    /// A URL with the token embedded, for handing to external programs.
    public func authedUrl(_ path: String, _ params: Params) throws -> URL {
        let extra = params.set("X-Plex-Token", token)
        return appendQuery(try url(path), extra.toQueryString())
    }

    /// `server://<machine>/com.plexapp.plugins.library<path>` URIs used by
    /// playlists, collections and play queues.
    public func libraryUri(_ path: String) -> String {
        "server://\(machineIdentifier ?? "")/com.plexapp.plugins.library\(path)"
    }

    /// Library URI for one or more items (`/library/metadata/1,2,3`).
    public func itemUri(_ ratingKeys: [String]) -> String {
        libraryUri("/library/metadata/\(ratingKeys.joined(separator: ","))")
    }

    // ----- request helpers ----------------------------------------------

    func requestBuilder(_ method: String, _ path: String, _ params: Params) throws -> RequestBuilder {
        http.request(method, try url(path), token: token, params)
    }

    func request<T>(_ method: String, _ path: String, _ params: Params, _ normalize: (JSONValue) throws -> T)
        async throws -> T
    {
        try await http.json(try requestBuilder(method, path, params), normalize)
    }

    func container(_ method: String, _ path: String, _ params: Params) async throws -> MediaContainer {
        try await request(method, path, params, Envelope.init(json:)).mediaContainer
    }

    func get(_ path: String, _ params: Params = Params()) async throws -> MediaContainer {
        try await container("GET", path, params)
    }

    func empty(_ method: String, _ path: String, _ params: Params = Params()) async throws {
        try await http.empty(try requestBuilder(method, path, params))
    }

    func put(_ path: String, _ params: Params = Params()) async throws {
        try await empty("PUT", path, params)
    }

    func delete(_ path: String, _ params: Params = Params()) async throws {
        try await empty("DELETE", path, params)
    }

    func post(_ path: String, _ params: Params = Params()) async throws {
        try await empty("POST", path, params)
    }

    /// Escape hatch: any endpoint, raw JSON back.
    public func raw(_ method: String, _ path: String, _ params: Params = Params()) async throws -> JSONValue {
        try await request(method, path, params) { $0 }
    }

    /// Escape hatch returning the typed container.
    public func fetch(_ path: String, _ params: Params = Params()) async throws -> MediaContainer {
        try await get(path, params)
    }

    /// GET raw bytes (images, subtitles, downloads) and the response content type.
    public func bytes(_ path: String, _ params: Params = Params()) async throws -> (Data, String?) {
        let (data, resp) = try await http.send(try requestBuilder("GET", path, params))
        return (data, resp.value(forHTTPHeaderField: "Content-Type"))
    }

    /// Streaming GET, for large responses (the 60s timeout bounds the wait for the headers).
    public func stream(_ path: String, _ params: Params = Params()) async throws -> PlexResponse {
        try await http.open(try requestBuilder("GET", path, params))
    }

    /// Streaming GET without the usual request timeout, for whole media files.
    public func download(_ path: String, _ params: Params = Params()) async throws -> PlexResponse {
        try await http.open(try requestBuilder("GET", path, params).timeout(downloadTimeout))
    }

    /// The request `downloadRange` sends, for handing to a background `URLSession`.
    public func downloadRequest(_ path: String, offset: Int, validator: String? = nil) throws -> URLRequest {
        var rb = try requestBuilder("GET", path, Params().set("download", 1))
            .header("Accept-Encoding", "identity")
            .timeout(downloadTimeout)
        if offset > 0 {
            rb = rb.header("Range", "bytes=\(offset)-")
            if let validator { rb = rb.header("If-Range", validator) }
        }
        var req = http.urlRequest(rb)
        req.timeoutInterval = downloadTimeout
        return req
    }

    /// Resume a media transfer. Identity encoding keeps byte offsets meaningful.
    public func downloadRange(_ path: String, offset: Int, validator: String? = nil) async throws -> PlexResponse {
        var rb = try requestBuilder("GET", path, Params().set("download", 1))
            .header("Accept-Encoding", "identity")
            .timeout(downloadTimeout)
        if offset > 0 {
            rb = rb.header("Range", "bytes=\(offset)-")
            if let validator { rb = rb.header("If-Range", validator) }
        }
        return try await http.open(rb)
    }

    // ----- server info ---------------------------------------------------

    /// Server root: capabilities, version, flags (in `extra`) and feature directories.
    public func capabilities() async throws -> MediaContainer {
        try await get("/", Params())
    }

    /// Unauthenticated identity check; used to probe connections.
    public func identity() async throws -> Identity {
        try await request("GET", "/identity", Params()) { v in
            guard case .object(let o) = v else { throw DecodeFailure("", "invalid type: expected struct Wrap") }
            guard let mc = o["MediaContainer"] else { throw DecodeFailure("", "missing field `MediaContainer`") }
            do {
                return try Identity(json: mc)
            } catch let e as DecodeFailure {
                throw e.under("MediaContainer")
            }
        }
    }

    private func imageParams(_ source: String, _ width: Int, _ height: Int) -> Params {
        Params()
            .set("url", source)
            .set("width", width)
            .set("height", height)
            .set("minSize", 1)
            .set("upscale", 1)
    }

    /// Resized image URL via the photo transcoder. `source` is a server path or absolute URL.
    public func imageUrl(_ source: String, width: Int, height: Int) throws -> URL {
        try authedUrl("/photo/:/transcode", imageParams(source, width, height))
    }

    /// Fetch a resized image through the photo transcoder.
    public func image(_ source: String, width: Int, height: Int) async throws -> (Data, String?) {
        try await bytes("/photo/:/transcode", imageParams(source, width, height))
    }
}
