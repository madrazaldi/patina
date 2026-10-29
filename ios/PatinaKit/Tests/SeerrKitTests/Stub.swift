// A stub Seerr server for the tests; no TypeScript twin (those run `Bun.serve`).
// Each `Stub.serve` gets its own host, so tests running in parallel don't share routes.

import Foundation
import Synchronization

struct StubResponse: Sendable {
    var status: Int = 200
    var headers: [String: String] = [:]
    var body: Data = Data()

    static func json(_ object: Any, status: Int = 200, headers: [String: String] = [:]) -> StubResponse {
        var headers = headers
        headers["Content-Type"] = "application/json"
        return StubResponse(status: status, headers: headers, body: try! JSONSerialization.data(withJSONObject: object))
    }
}

/// A request as the stub saw it, with its body.
struct StubRequest: Sendable {
    let method: String
    let url: URL
    let headers: [String: String]
    let body: Data?

    var path: String { url.path }

    func header(_ name: String) -> String? {
        headers.first { $0.key.caseInsensitiveCompare(name) == .orderedSame }?.value
    }

    func json() -> Any? {
        body.flatMap { try? JSONSerialization.jsonObject(with: $0) }
    }
}

final class Stub: URLProtocol {
    typealias Handler = @Sendable (StubRequest) -> StubResponse

    private static let routes = Mutex<[String: Handler]>([:])

    /// The base URL of a new stub server and the session configuration that reaches it.
    static func serve(_ handler: @escaping Handler) -> (url: String, configuration: URLSessionConfiguration) {
        let host = "seerr-\(UUID().uuidString.lowercased())"
        routes.withLock { $0[host] = handler }
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [Stub.self]
        return ("http://\(host):5055", configuration)
    }

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let url = request.url, let handler = Self.routes.withLock({ $0[url.host ?? ""] }) else {
            client?.urlProtocol(self, didFailWithError: URLError(.cannotConnectToHost))
            return
        }
        let seen = StubRequest(
            method: request.httpMethod ?? "GET", url: url, headers: request.allHTTPHeaderFields ?? [:], body: bodyOf(request)
        )
        let out = handler(seen)
        let resp = HTTPURLResponse(url: url, statusCode: out.status, httpVersion: "HTTP/1.1", headerFields: out.headers)!
        client?.urlProtocol(self, didReceive: resp, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: out.body)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}

/// URLSession hands a protocol the body as a stream.
private func bodyOf(_ request: URLRequest) -> Data? {
    if let body = request.httpBody { return body }
    guard let stream = request.httpBodyStream else { return nil }
    stream.open()
    defer { stream.close() }
    var data = Data()
    var buffer = [UInt8](repeating: 0, count: 4096)
    while stream.hasBytesAvailable {
        let n = stream.read(&buffer, maxLength: buffer.count)
        if n <= 0 { break }
        data.append(buffer, count: n)
    }
    return data
}
