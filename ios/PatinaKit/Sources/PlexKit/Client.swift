// Port of desktop/plex/client.ts.
// Shared HTTP plumbing for plex.tv and server clients (port of crates/plex/src/client.rs)
// on top of `URLSession`.

import Foundation
import Synchronization

/// Identifies this application to Plex. Every request carries these values as `X-Plex-*` headers.
public struct ClientInfo: Sendable, Hashable {
    /// Must be stable per installation.
    public var identifier: String
    public var product: String
    public var version: String
    public var platform: String
    public var platformVersion: String
    public var device: String
    public var deviceName: String

    /// The platform this build runs on, as Plex names it.
    public static var currentPlatform: String {
        #if os(iOS)
            "iOS"
        #elseif os(macOS)
            "macOS"
        #elseif os(Linux)
            "Linux"
        #elseif os(Windows)
            "Windows"
        #else
            "Unknown"
        #endif
    }

    /// `device` defaults to the platform and `deviceName` to the host name (as on the desktop);
    /// the iOS app passes "iOS", the device model and the device name.
    public init(
        identifier: String, product: String, version: String, platform: String = ClientInfo.currentPlatform,
        device: String? = nil, deviceName: String? = nil, platformVersion: String = ""
    ) {
        self.identifier = identifier
        self.product = product
        self.version = version
        self.platform = platform
        self.platformVersion = platformVersion
        self.device = device ?? platform
        self.deviceName = deviceName ?? hostname() ?? platform
    }

    public func headers() -> [(String, String)] {
        [
            ("X-Plex-Client-Identifier", identifier),
            ("X-Plex-Product", product),
            ("X-Plex-Version", version),
            ("X-Plex-Platform", platform),
            ("X-Plex-Platform-Version", platformVersion),
            ("X-Plex-Device", device),
            ("X-Plex-Device-Name", deviceName),
            ("X-Plex-Model", "desktop"),
            ("X-Plex-Provides", "player"),
        ]
    }

    /// The default header map: `Accept` plus the `X-Plex-*` values reqwest could encode.
    public func headerMap() -> [String: String] {
        var map = ["Accept": "application/json"]
        for (k, v) in headers() {
            // HeaderValue::from_str only accepts visible ASCII (and tab); other values are skipped.
            if v.unicodeScalars.allSatisfy({ $0 == "\t" || (0x20...0x7e).contains($0.value) }) { map[k] = v }
        }
        return map
    }
}

private func hostname() -> String? {
    var raw = ProcessInfo.processInfo.environment["HOSTNAME"]
    if raw == nil {
        var buf = [CChar](repeating: 0, count: 256)
        if gethostname(&buf, buf.count) == 0 {
            raw = String(decoding: buf.prefix(while: { $0 != 0 }).map { UInt8(bitPattern: $0) }, as: UTF8.self)
        }
    }
    guard var h = raw?.trimmingCharacters(in: .whitespacesAndNewlines) else { return nil }
    while h.hasSuffix(".local") { h.removeLast(".local".count) }
    return h.isEmpty ? nil : h
}

/// A query parameter value: written like JavaScript's `String(value)`.
public protocol ParamValue: Sendable {
    var paramString: String { get }
}

extension String: ParamValue { public var paramString: String { self } }
extension Substring: ParamValue { public var paramString: String { String(self) } }
extension Int: ParamValue { public var paramString: String { String(self) } }
extension Int64: ParamValue { public var paramString: String { String(self) } }
extension Int32: ParamValue { public var paramString: String { String(self) } }
extension Bool: ParamValue { public var paramString: String { String(self) } }
extension Double: ParamValue { public var paramString: String { jsNumberString(self) } }

/// Ordered query parameters. Plex endpoints take almost everything as query
/// parameters, including on PUT/POST. The builders return a modified copy (the Rust
/// ones consume `self`); `push` appends in place.
public struct Params: Sendable, Equatable, ExpressibleByDictionaryLiteral {
    public var entries: [(key: String, value: String)]

    public init() {
        entries = []
    }

    /// `Params::from_iter`.
    public init(_ entries: [(String, any ParamValue)]) {
        self.entries = entries.map { ($0.0, $0.1.paramString) }
    }

    /// `["type": 1, "sort": "titleSort"]`, in the order written.
    public init(dictionaryLiteral elements: (String, any ParamValue)...) {
        self.init(elements)
    }

    public static func == (a: Params, b: Params) -> Bool {
        a.entries.count == b.entries.count && zip(a.entries, b.entries).allSatisfy { $0 == $1 }
    }

    public func set(_ key: String, _ value: some ParamValue) -> Params {
        var p = self
        p.push(key, value)
        return p
    }

    public func opt(_ key: String, _ value: (some ParamValue)?) -> Params {
        guard let value else { return self }
        return set(key, value)
    }

    /// Adds `key=1` when `flag` is true.
    public func flag(_ key: String, _ flag: Bool) -> Params {
        flag ? set(key, 1) : self
    }

    public mutating func push(_ key: String, _ value: some ParamValue) {
        entries.append((key, value.paramString))
    }

    public func extend(_ other: Params) -> Params {
        var p = self
        p.entries.append(contentsOf: other.entries)
        return p
    }

    public var isEmpty: Bool { entries.isEmpty }

    /// `application/x-www-form-urlencoded`, as `url::form_urlencoded::Serializer` writes it.
    public func toQueryString() -> String {
        formEncode(entries)
    }
}

/// The form-urlencoded byte serializer (`URLSearchParams`): `*-._` and alphanumerics stay,
/// space becomes `+`, everything else is percent-encoded.
func formEncodeComponent(_ s: String) -> String {
    var out = ""
    for b in s.utf8 {
        switch b {
        case UInt8(ascii: "a")...UInt8(ascii: "z"), UInt8(ascii: "A")...UInt8(ascii: "Z"),
            UInt8(ascii: "0")...UInt8(ascii: "9"), UInt8(ascii: "*"), UInt8(ascii: "-"), UInt8(ascii: "."),
            UInt8(ascii: "_"):
            out.unicodeScalars.append(Unicode.Scalar(b))
        case UInt8(ascii: " "):
            out += "+"
        default:
            out += String(format: "%%%02X", b)
        }
    }
    return out
}

func formEncode(_ pairs: [(key: String, value: String)]) -> String {
    pairs.map { "\(formEncodeComponent($0.key))=\(formEncodeComponent($0.value))" }.joined(separator: "&")
}

/// Parses a form-urlencoded query (`URLSearchParams`): `+` is a space, `%XX` a byte.
func formDecode(_ query: String) -> [(key: String, value: String)] {
    func decode(_ s: Substring) -> String {
        var bytes: [UInt8] = []
        var it = Array(s.utf8)[...]
        while let b = it.popFirst() {
            if b == UInt8(ascii: "+") {
                bytes.append(UInt8(ascii: " "))
            } else if b == UInt8(ascii: "%"), it.count >= 2,
                let v = UInt8(String(decoding: it.prefix(2), as: UTF8.self), radix: 16)
            {
                bytes.append(v)
                it = it.dropFirst(2)
            } else {
                bytes.append(b)
            }
        }
        return String(decoding: bytes, as: UTF8.self)
    }
    return query.split(separator: "&").map { pair in
        if let eq = pair.firstIndex(of: "=") {
            return (decode(pair[..<eq]), decode(pair[pair.index(after: eq)...]))
        }
        return (decode(pair), "")
    }
}

/// Parse a URL like `Url::parse`, mapping failures to `PlexError` with `url::ParseError` wording.
public func parseUrl(_ input: String, base: URL? = nil) throws -> URL {
    if let url = URL(string: input, relativeTo: base)?.absoluteURL, url.scheme != nil,
        !["http", "https"].contains(url.scheme?.lowercased() ?? "") || !(url.host ?? "").isEmpty
    {
        return url
    }
    let text = input.trimmingCharacters(in: .whitespacesAndNewlines)
    if base == nil, text.wholeMatch(of: #/[A-Za-z][A-Za-z0-9+.-]*:.*/#.dotMatchesNewlines()) == nil {
        throw PlexError.url("relative URL without a base")
    }
    let authority = text.firstMatch(of: #/^[A-Za-z][A-Za-z0-9+.-]*:(?:\/\/)?([^\/?#]*)/#).map { String($0.1) } ?? ""
    let hostPort = authority.split(separator: "@", omittingEmptySubsequences: false).last.map(String.init) ?? ""
    if hostPort.isEmpty || hostPort.hasPrefix(":") { throw PlexError.url("empty host") }
    if let port = hostPort.firstMatch(of: #/:(\d+)$/#), (Int(port.1) ?? Int.max) > 65535 {
        throw PlexError.url("invalid port number")
    }
    if !hostPort.hasPrefix("["), hostPort.firstMatch(of: #/:[^\]]*[^\d:\]][^:]*$/#) != nil {
        throw PlexError.url("invalid port number")
    }
    throw PlexError.url("invalid domain character")
}

/// Appends `extra` (already form-encoded) to the URL's existing query, leaving that query as it is.
public func appendQuery(_ url: URL, _ extra: String) -> URL {
    if extra.isEmpty { return url }
    guard var c = URLComponents(url: url, resolvingAgainstBaseURL: true) else { return url }
    let existing = c.percentEncodedQuery ?? ""
    c.percentEncodedQuery = existing.isEmpty ? extra : "\(existing)&\(extra)"
    return c.url ?? url
}

/// The percent-encoded path of a URL, as JavaScript's `URL.pathname` reports it.
func pathname(_ url: URL) -> String {
    let p = URLComponents(url: url, resolvingAgainstBaseURL: true)?.percentEncodedPath ?? url.path
    return p.isEmpty ? "/" : p
}

/// A request being assembled; `Http.send` runs it.
public struct RequestBuilder: Sendable {
    public let method: String
    public let url: URL
    public var headers: [String: String] = [:]
    public var body: Data?
    /// Total timeout for this request, replacing the client default.
    public var timeout: TimeInterval?

    public init(_ method: String, _ url: URL) {
        self.method = method
        self.url = url
    }

    public func header(_ name: String, _ value: String) -> RequestBuilder {
        var rb = self
        rb.headers[name] = value
        return rb
    }

    /// URL-encoded form body (sets the content type).
    public func form(_ pairs: [(String, String)]) -> RequestBuilder {
        var rb = self
        rb.body = Data(formEncode(pairs.map { (key: $0.0, value: $0.1) }).utf8)
        rb.headers["Content-Type"] = "application/x-www-form-urlencoded"
        return rb
    }

    public func timeout(_ seconds: TimeInterval) -> RequestBuilder {
        var rb = self
        rb.timeout = seconds
        return rb
    }
}

/// A response whose body has not been read yet (streaming GETs, downloads, notifications).
public struct PlexResponse: @unchecked Sendable {
    public let response: HTTPURLResponse
    /// The body, read as it arrives.
    public let bytes: URLSession.AsyncBytes

    public var status: Int { response.statusCode }
    public var url: URL? { response.url }

    /// A response header, case-insensitively.
    public func header(_ name: String) -> String? {
        response.value(forHTTPHeaderField: name)
    }
}

let connectTimeout: TimeInterval = 8
let totalTimeout: TimeInterval = 60

struct TimedOut: Error {}

/// Runs `op`, failing with `TimedOut` after `seconds`. `op` is cancelled when the time is up.
func withDeadline<T: Sendable>(_ seconds: TimeInterval, _ op: @escaping @Sendable () async throws -> T) async throws
    -> T
{
    try await withThrowingTaskGroup(of: T.self) { group in
        group.addTask { try await op() }
        group.addTask {
            try await Task.sleep(for: .seconds(seconds))
            throw TimedOut()
        }
        defer { group.cancelAll() }
        return try await group.next()!
    }
}

/// Shared HTTP plumbing for plex.tv and server clients.
public final class Http: Sendable {
    public let info: ClientInfo
    let session: URLSession
    private let defaults: [String: String]
    /// Origins that already answered once; the connect timeout only applies to the first request.
    private let reached = Mutex<Set<String>>([])

    /// One session for every client that is not given its own; reqwest keeps no cache or cookies.
    public static let defaultSession: URLSession = {
        let config = URLSessionConfiguration.default
        config.urlCache = nil
        config.requestCachePolicy = .reloadIgnoringLocalCacheData
        config.httpCookieStorage = nil
        config.httpShouldSetCookies = false
        return URLSession(configuration: config)
    }()

    public init(info: ClientInfo, session: URLSession? = nil) {
        self.info = info
        self.session = session ?? Http.defaultSession
        defaults = info.headerMap()
    }

    public func request(_ method: String, _ url: URL, token: String?, _ params: Params) -> RequestBuilder {
        var rb = RequestBuilder(method, appendQuery(url, params.toQueryString()))
        if let token { rb = rb.header("X-Plex-Token", token) }
        return rb
    }

    private static func origin(_ url: URL) -> String {
        "\(url.scheme ?? "")://\(url.host ?? ""):\(url.port.map(String.init) ?? "")"
    }

    /// The `URLRequest` for `rb`, with the default headers. Also what a background download
    /// session is given.
    public func urlRequest(_ rb: RequestBuilder) -> URLRequest {
        var req = URLRequest(url: rb.url)
        req.httpMethod = rb.method
        for (k, v) in defaults { req.setValue(v, forHTTPHeaderField: k) }
        for (k, v) in rb.headers { req.setValue(v, forHTTPHeaderField: k) }
        req.httpBody = rb.body
        return req
    }

    /// reqwest has a real connect timeout of 8s; here the first request to an origin gets an
    /// 8s idle timeout, which covers connecting and waiting for the answer.
    private func prepare(_ rb: RequestBuilder) -> (URLRequest, TimeInterval) {
        let total = rb.timeout ?? totalTimeout
        var req = urlRequest(rb)
        let first = !reached.withLock { $0.contains(Http.origin(rb.url)) }
        req.timeoutInterval = first ? min(connectTimeout, total) : total
        return (req, total)
    }

    private func sendError(_ error: Error, _ url: URL) -> Error {
        if Task.isCancelled || error is CancellationError { return CancellationError() }
        if let u = error as? URLError, u.code == .cancelled { return CancellationError() }
        let detail: String
        if error is TimedOut || (error as? URLError)?.code == .timedOut {
            detail = "operation timed out"
        } else {
            detail = error.localizedDescription
        }
        return PlexError.http("error sending request for url (\(redact(url))): \(detail)")
    }

    private func check(_ rb: RequestBuilder, _ resp: URLResponse, _ body: () async -> Data) async throws
        -> HTTPURLResponse
    {
        guard let http = resp as? HTTPURLResponse else {
            throw PlexError.http("error sending request for url (\(redact(rb.url))): not an HTTP response")
        }
        reached.withLock { _ = $0.insert(Http.origin(rb.url)) }
        if (200..<300).contains(http.statusCode) { return http }
        if http.statusCode == 401 { throw PlexError.unauthorized() }
        let text = String(decoding: await body(), as: UTF8.self)
        throw PlexError.statusError(rb.method, pathname(rb.url), http.statusCode, text)
    }

    /// Sends the request and reads the whole body; non-2xx answers become errors (401 is
    /// `Unauthorized`). The total timeout (60s unless the builder says otherwise) covers the body.
    public func send(_ rb: RequestBuilder) async throws -> (Data, HTTPURLResponse) {
        Log.debug("plex request method=\(rb.method) url=\(redact(rb.url))")
        let (req, total) = prepare(rb)
        let session = self.session
        let result: (Data, URLResponse)
        do {
            result = try await withDeadline(total) { try await session.data(for: req) }
        } catch {
            throw sendError(error, rb.url)
        }
        let http = try await check(rb, result.1) { result.0 }
        return (result.0, http)
    }

    /// Sends the request and returns once the headers are in; the body is read from the
    /// result. Only the wait for the headers is bounded by the timeout.
    public func open(_ rb: RequestBuilder) async throws -> PlexResponse {
        Log.debug("plex request method=\(rb.method) url=\(redact(rb.url))")
        let (req, total) = prepare(rb)
        let session = self.session
        let opened: Opened
        do {
            opened = try await withDeadline(total) {
                let (bytes, resp) = try await session.bytes(for: req)
                return Opened(bytes: bytes, response: resp)
            }
        } catch {
            throw sendError(error, rb.url)
        }
        let http = try await check(rb, opened.response) {
            var data = Data()
            do {
                for try await b in opened.bytes {
                    data.append(b)
                    if data.count > 64 * 1024 { break }
                }
            } catch {}
            return data
        }
        return PlexResponse(response: http, bytes: opened.bytes)
    }

    private struct Opened: @unchecked Sendable {
        let bytes: URLSession.AsyncBytes
        let response: URLResponse
    }

    /// Sends and returns the raw JSON answer (`serde_json::Value`).
    public func json(_ rb: RequestBuilder) async throws -> JSONValue {
        try await json(rb) { $0 }
    }

    /// Sends and decodes the JSON answer with `normalize`.
    public func json<T>(_ rb: RequestBuilder, _ normalize: (JSONValue) throws -> T) async throws -> T {
        let (data, resp) = try await send(rb)
        let path = resp.url.map(pathname) ?? pathname(rb.url)
        return try decode(path, data, normalize)
    }

    public func empty(_ rb: RequestBuilder) async throws {
        _ = try await send(rb)
    }
}

/// Decode a JSON body with `normalize`. An empty body counts as `{}`.
public func decode<T>(_ path: String, _ bytes: Data, _ normalize: (JSONValue) throws -> T) throws -> T {
    // Some endpoints answer with an empty body where a container is expected.
    let blank = bytes.allSatisfy { [0x20, 0x09, 0x0A, 0x0C, 0x0D].contains($0) }
    let value: JSONValue
    do {
        value = try JSONValue.parse(blank ? Data("{}".utf8) : bytes)
    } catch {
        throw PlexError.decode(path, parseMessage(error))
    }
    return try decodeValue(path, value, normalize)
}

/// The parser's own wording, without the `DecodingError` wrapping.
private func parseMessage(_ error: Error) -> String {
    if case DecodingError.dataCorrupted(let ctx) = error {
        if let underlying = ctx.underlyingError as NSError?,
            let text = underlying.userInfo[NSDebugDescriptionErrorKey] as? String
        {
            return text
        }
        return ctx.debugDescription
    }
    return error.localizedDescription
}

/// Run `normalize` over an already parsed value, mapping decode failures to `PlexError.decode`.
public func decodeValue<T>(_ path: String, _ value: JSONValue, _ normalize: (JSONValue) throws -> T) throws -> T {
    do {
        return try normalize(value)
    } catch let e as DecodeFailure {
        throw PlexError.decode(path, e)
    }
}

/// The URL for logs: the token query parameter is replaced by `***`.
public func redact(_ url: URL) -> String {
    guard var c = URLComponents(url: url, resolvingAgainstBaseURL: true) else { return url.absoluteString }
    let pairs = formDecode(c.percentEncodedQuery ?? "").map { pair in
        (key: pair.key, value: pair.key.lowercased() == "x-plex-token" ? "***" : pair.value)
    }
    c.percentEncodedQuery = pairs.isEmpty ? nil : formEncode(pairs)
    return c.url?.absoluteString ?? url.absoluteString
}

/// Debug logging (`PATINA_LOG=debug`), as `logger("plex")` does on the desktop.
enum Log {
    static let enabled = ProcessInfo.processInfo.environment["PATINA_LOG"]?.lowercased() == "debug"

    static func debug(_ message: @autoclosure () -> String) {
        if enabled { FileHandle.standardError.write(Data("[plex] \(message())\n".utf8)) }
    }
}
