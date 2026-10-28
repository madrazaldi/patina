// HTTP stubbing for the PlexKit tests (no TypeScript twin; the TS tests start a Bun server).
// Each `Fake` registers a handler under a unique id that it puts in `X-Plex-Device-Name`,
// so tests running in parallel do not see each other's requests.

import Foundation
import Synchronization

@testable import PlexKit

/// What a fake server answers.
struct StubResponse: Sendable {
    var status = 200
    var headers: [String: String] = [:]
    /// Sent one after the other.
    var chunks: [Data] = []
    /// Keep the connection open after the chunks (an endless stream).
    var hang = false
    /// Fail the request as if the host could not be reached.
    var unreachable = false

    static func json(_ text: String, status: Int = 200) -> StubResponse {
        StubResponse(status: status, headers: ["Content-Type": "application/json"], chunks: [Data(text.utf8)])
    }

    static func text(_ text: String, status: Int = 200, headers: [String: String] = [:]) -> StubResponse {
        StubResponse(status: status, headers: headers, chunks: [Data(text.utf8)])
    }

    static let down = StubResponse(unreachable: true)
}

/// A request as the fake server saw it.
struct Seen: Sendable {
    let method: String
    let url: URL
    let path: String
    let query: [(key: String, value: String)]
    let headers: [String: String]

    func header(_ name: String) -> String? {
        headers.first(where: { $0.key.lowercased() == name.lowercased() })?.value
    }

    var pairs: [String] { query.map { "\($0.key)=\($0.value)" } }
}

final class StubProtocol: URLProtocol, @unchecked Sendable {
    typealias Handler = @Sendable (Seen) -> StubResponse

    static let handlers = Mutex<[String: Handler]>([:])
    static let seen = Mutex<[String: [Seen]]>([:])

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        let req = request
        let key = req.value(forHTTPHeaderField: "X-Plex-Device-Name") ?? ""
        let url = req.url!
        let c = URLComponents(url: url, resolvingAgainstBaseURL: true)!
        let s = Seen(
            method: req.httpMethod ?? "GET", url: url, path: c.percentEncodedPath,
            query: formDecode(c.percentEncodedQuery ?? ""), headers: req.allHTTPHeaderFields ?? [:])
        StubProtocol.seen.withLock { $0[key, default: []].append(s) }
        guard let handler = StubProtocol.handlers.withLock({ $0[key] }) else {
            client?.urlProtocol(self, didFailWithError: URLError(.cannotConnectToHost))
            return
        }
        let r = handler(s)
        if r.unreachable {
            client?.urlProtocol(self, didFailWithError: URLError(.cannotConnectToHost))
            return
        }
        let resp = HTTPURLResponse(url: url, statusCode: r.status, httpVersion: "HTTP/1.1", headerFields: r.headers)!
        client?.urlProtocol(self, didReceive: resp, cacheStoragePolicy: .notAllowed)
        for chunk in r.chunks { client?.urlProtocol(self, didLoad: chunk) }
        if !r.hang { client?.urlProtocolDidFinishLoading(self) }
    }

    override func stopLoading() {}
}

/// One fake backend: a session routed to `handler`, and the requests it saw. Handlers stay
/// registered for the whole run: ARC may release a `Fake` before its last request is made.
final class Fake: Sendable {
    let id = "stub-\(UUID().uuidString)"
    let session: URLSession
    let info: ClientInfo

    init(identifier: String = "test-client", product: String = "Patina", version: String = "1.2.3", _ handler: @escaping StubProtocol.Handler) {
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [StubProtocol.self]
        session = URLSession(configuration: config)
        info = ClientInfo(identifier: identifier, product: product, version: version, deviceName: id)
        StubProtocol.handlers.withLock { $0[id] = handler }
    }

    var http: Http { Http(info: info, session: session) }

    func server(_ base: String = "http://127.0.0.1:32400", token: String = "tok-123") throws -> PlexServer {
        try PlexServer(http: http, baseUrl: base, token: token)
    }

    func tv(_ token: String? = nil) -> PlexTv {
        PlexTv(info: info, token: token, http: http)
    }

    var seen: [Seen] { StubProtocol.seen.withLock { $0[id] ?? [] } }
    var last: Seen { seen.last! }

    func reset() {
        StubProtocol.seen.withLock { $0[id] = [] }
    }
}
