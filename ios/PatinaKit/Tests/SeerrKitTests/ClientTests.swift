// Port of desktop/seerr/client.test.ts.

import Foundation
import Synchronization
import Testing

@testable import SeerrKit

@Suite struct ClientTests {
    @Test func addressesAreNormalized() throws {
        #expect(try normalizeUrl("seerr.local:5055") == "http://seerr.local:5055")
        #expect(try normalizeUrl(" https://requests.example.com/ ") == "https://requests.example.com")
        #expect(try normalizeUrl("http://10.0.0.2:5055/api/v1/") == "http://10.0.0.2:5055")
        #expect(try normalizeUrl("https://example.com/seerr/?x=1#top") == "https://example.com/seerr")
        #expect(message { try normalizeUrl("") }.contains("Enter the address"))
        #expect(message { try normalizeUrl("ftp://example.com") }.contains("http:// or https://"))
    }

    @Test func queriesEncodeTheCharactersSeerrRejects() {
        #expect(
            encodeQuery(["query": "Kid's Own (Director's Cut)!*", "page": 1, "skip": nil])
                == "query=Kid%27s%20Own%20%28Director%27s%20Cut%29%21%2A&page=1"
        )
    }

    @Test func aPlexSignInIsKeptAndRenewedOnceTheSessionIsGone() async throws {
        struct Server {
            var sessions = 0
            var valid = ""
            var seen: [String] = []
            var bodies: [String] = []
        }
        let server = Mutex(Server())
        let (url, configuration) = Stub.serve { req in
            server.withLock { s in
                s.seen.append("\(req.method) \(req.path)")
                if req.path == "/api/v1/auth/plex" {
                    s.bodies.append(String(decoding: req.body ?? Data(), as: UTF8.self))
                    s.sessions += 1
                    s.valid = "s\(s.sessions)"
                    return .json(["id": 2], headers: ["Set-Cookie": "connect.sid=\(s.valid); Path=/; HttpOnly"])
                }
                if req.header("Cookie") != "connect.sid=\(s.valid)" { return .json(["message": "no"], status: 403) }
                return .json(["id": 2, "displayName": "Alice", "permissions": 32])
            }
        }
        let client = try SeerrClient(url, auth: .plexToken("plex-token"), configuration: configuration)
        #expect(client.authKind == .plex)
        async let a = client.me()
        async let b = client.me()
        _ = try await (a, b)
        #expect(server.withLock { $0.sessions } == 1)
        #expect(server.withLock { $0.bodies } == [#"{"authToken":"plex-token"}"#])
        server.withLock { $0.valid = "expired" }
        // Requests that find the session gone at the same time share one new sign-in.
        async let c = client.me()
        async let d = client.me()
        async let e = client.me()
        let all = try await [c, d, e]
        #expect(all.map(\.displayName) == ["Alice", "Alice", "Alice"])
        #expect(server.withLock { $0.sessions } == 2)
        #expect(server.withLock { $0.seen.filter { $0 == "POST /api/v1/auth/plex" }.count } == 2)
    }

    @Test func errorsCarrySeerrsMessageAndABadKeyReadsAsASignInProblem() async throws {
        let (url, configuration) = Stub.serve { req in
            if req.header("X-Api-Key") != "good" { return .json(["message": "Unauthorized"], status: 401) }
            return .json(["message": "Request for this media already exists."], status: 409)
        }
        let bad = try SeerrClient(url, auth: .apiKey("bad"), configuration: configuration)
        await #expect(asyncMessage { _ = try await bad.me() }.contains("did not accept the sign-in"))
        let good = try SeerrClient(url, auth: .apiKey("good"), configuration: configuration)
        let error = await asyncError { _ = try await good.createRequest(.init(mediaType: .movie, mediaId: 1, is4k: false)) }
        #expect(error?.message == "Request for this media already exists.")
        #expect(error?.status == 409)
    }

    // Beyond the TypeScript tests: what Swift does differently.

    @Test func requestsHaveTheShapesSeerrExpects() async throws {
        let seen = Mutex<[StubRequest]>([])
        let (url, configuration) = Stub.serve { req in
            seen.withLock { $0.append(req) }
            switch (req.method, req.path) {
            case ("POST", "/api/v1/request"): return .json(["message": "No seasons available to request"], status: 202)
            case ("DELETE", _): return StubResponse(status: 204)
            default: return .json(["pageInfo": ["page": 2, "pages": 3, "results": 41], "results": []])
            }
        }
        let client = try SeerrClient(url + "/api/v1/", auth: .apiKey("k"), configuration: configuration)
        #expect(client.authKind == .apiKey)
        let page = try await client.requests(take: 20, skip: 20, filter: "all", sort: "added")
        #expect(page.pageInfo == .init(page: 2, pages: 3, results: 41))
        let made = try await client.createRequest(.init(mediaType: .tv, mediaId: 8, seasons: [1, 2], is4k: false, rootFolder: "/tv"))
        #expect(made?.status == nil)
        #expect(made?.message == "No seasons available to request")
        try await client.deleteRequest(5)

        let reqs = seen.withLock { $0 }
        #expect(reqs.map { "\($0.method) \($0.url.absoluteString.replacingOccurrences(of: url, with: ""))" } == [
            "GET /api/v1/request?take=20&skip=20&filter=all&sort=added",
            "POST /api/v1/request",
            "DELETE /api/v1/request/5",
        ])
        #expect(reqs.allSatisfy { $0.header("X-Api-Key") == "k" && $0.header("Accept") == "application/json" && $0.header("Cookie") == nil })
        #expect(reqs[1].header("Content-Type") == "application/json")
        let body = try #require(reqs[1].json() as? [String: Any])
        #expect(Set(body.keys) == ["mediaType", "mediaId", "seasons", "is4k", "rootFolder"])
        #expect(String(decoding: reqs[1].body ?? Data(), as: UTF8.self).contains(#""rootFolder":"/tv""#))
    }

    @Test func oddFieldsDoNotFailAResponse() async throws {
        let (url, configuration) = Stub.serve { _ in
            .json([
                "page": "1", "totalPages": 1.0,
                "results": [
                    ["id": "7", "mediaType": "movie", "title": 42, "voteAverage": "6.5",
                     "mediaInfo": ["status": 5, "ratingKey": 100, "requests": "odd", "seasons": ["seasonNumber": 1]]],
                    "not a result",
                    ["id": 8, "mediaType": "tv", "mediaInfo": NSNull(), "posterPath": NSNull()],
                ],
            ])
        }
        let client = try SeerrClient(url, auth: .apiKey("k"), configuration: configuration)
        let page = try await client.search("x")
        #expect(page.page == 1 && page.totalPages == 1)
        #expect(page.results.map(\.id) == [7, 8])
        let first = page.results[0]
        #expect(first.title == "42" && first.voteAverage == 6.5)
        #expect(first.mediaInfo?.ratingKey == "100")
        #expect(first.mediaInfo?.requests == nil)
        #expect(first.mediaInfo?.seasons == [.init(seasonNumber: 1)])
        #expect(page.results[1].mediaInfo == nil && page.results[1].posterPath == nil)
    }

    @Test func failuresNameWhatWentWrong() async throws {
        let (url, configuration) = Stub.serve { req in
            switch req.path {
            case "/api/v1/request/1/approve": return .json(["message": "You do not have permission."], status: 403)
            case "/api/v1/request/1/retry": return StubResponse(status: 302, headers: ["Location": "/login"])
            case "/api/v1/status": return StubResponse(status: 200, body: Data("<html>".utf8))
            default: return StubResponse(status: 500)
            }
        }
        let client = try SeerrClient(url, auth: .apiKey("k"), configuration: configuration)
        await #expect(asyncMessage { try await client.setRequestStatus(1, .approve) } == "Seerr refused: You do not have permission.")
        await #expect(asyncMessage { try await client.retryRequest(1) } == "Seerr redirected POST /request/1/retry; check the address.")
        await #expect(asyncMessage { _ = try await client.status() } == "\(url) did not answer like a Seerr server.")
        await #expect(asyncMessage { _ = try await client.services(.radarr) } == "GET /service/radarr failed with HTTP 500")
    }
}

private func message(_ body: () throws -> some Any) -> String {
    do {
        _ = try body()
        return ""
    } catch {
        return error.localizedDescription
    }
}

private func asyncError(_ body: () async throws -> Void) async -> SeerrError? {
    do {
        try await body()
        return nil
    } catch {
        return error as? SeerrError
    }
}

private func asyncMessage(_ body: () async throws -> some Any) async -> String {
    do {
        _ = try await body()
        return ""
    } catch {
        return error.localizedDescription
    }
}
