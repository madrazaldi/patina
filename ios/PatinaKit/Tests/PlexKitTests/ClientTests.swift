// Port of desktop/plex/client.test.ts.
// Params, ClientInfo, lenient decoding and plex.tv helpers (the Rust crate has no unit tests
// for these; they pin the behaviour other modules rely on).

import Foundation
import Testing

@testable import PlexKit

@Suite struct ClientTests {
    @Test func paramsBuildersKeepOrderAndSkipAbsentValues() {
        var p = Params().set("a", 1).opt("b", nil as Int?).opt("c", "x y").flag("d", false).flag("e", true)
        p.push("f", true)
        #expect(p.entries.map { "\($0.key)=\($0.value)" } == ["a=1", "c=x y", "e=1", "f=true"])
        #expect(p.toQueryString() == "a=1&c=x+y&e=1&f=true")
        #expect(Params().isEmpty)
        let q = Params().set("z", 0).extend(p)
        #expect(q.entries.count == 5)
        #expect(p == p)
        #expect(Params().set("k", "a&b=c/d").toQueryString() == "k=a%26b%3Dc%2Fd")
        let literal: Params = ["type": 1, "sort": "titleSort"]
        #expect(literal == Params().set("type", 1).set("sort", "titleSort"))
        #expect(Params().set("x", 12.5).set("y", 4000.0).toQueryString() == "x=12.5&y=4000")
    }

    @Test func clientInfoHeaders() {
        var info = ClientInfo(identifier: "id-1", product: "Patina", version: "0.9")
        let h = Dictionary(uniqueKeysWithValues: info.headers())
        #expect(h["X-Plex-Client-Identifier"] == "id-1")
        #expect(h["X-Plex-Model"] == "desktop")
        #expect(h["X-Plex-Platform"] == "macOS")
        #expect(h["X-Plex-Device"] == h["X-Plex-Platform"])
        #expect(h["X-Plex-Platform-Version"] == "")
        info.deviceName = "Café"  // not visible ASCII: reqwest drops the header
        #expect(info.headerMap()["X-Plex-Device-Name"] == nil)
        #expect(info.headerMap()["Accept"] == "application/json")

        let ios = ClientInfo(identifier: "i", product: "Patina", version: "1", platform: "iOS", device: "iPhone", deviceName: "Phone")
        let hi = Dictionary(uniqueKeysWithValues: ios.headers())
        #expect(hi["X-Plex-Platform"] == "iOS")
        #expect(hi["X-Plex-Device"] == "iPhone")
        #expect(hi["X-Plex-Device-Name"] == "Phone")
    }

    @Test func redactHidesTheToken() {
        let out = redact(URL(string: "http://h:1/a?x=1&X-Plex-Token=secret&y=2")!)
        #expect(!out.contains("secret"))
        #expect(out.contains("X-Plex-Token="))
        #expect(redact(URL(string: "http://h:1/a")!) == "http://h:1/a")
    }

    @Test func lenientScalars() {
        #expect(De.optString(5) == "5")
        #expect(De.optString(true) == "true")
        #expect(De.optString(.null) == nil)
        #expect(De.optString([:]) == nil)
        #expect(De.optString([1]) == nil)
        #expect(De.optI64("12") == 12)
        #expect(De.optI64(" 3.9 ") == 3)
        #expect(De.optI64("abc") == nil)
        #expect(De.optI64("") == nil)
        #expect(De.optI64(true) == 1)
        #expect(De.optI64(-2.7) == -2)
        #expect(De.optF64("1.5") == 1.5)
        #expect(De.optF64(false) == 0)
        #expect(De.boolean("1"))
        #expect(De.boolean("True"))
        #expect(De.boolean("yes"))
        #expect(!De.boolean("0"))
        #expect(De.boolean(2))
        #expect(!De.boolean(0))
        #expect(!De.boolean(nil))
    }

    @Test func oneOrManyAndList() throws {
        let id: (JSONValue) -> JSONValue = { $0 }
        #expect(try De.oneOrMany(.null, id) == [])
        #expect(try De.oneOrMany([1, 2], id) == [1, 2])
        #expect(try De.oneOrMany(["a": 1], id) == [["a": 1]])
        #expect(try De.list(nil, id) == [])
        #expect(throws: DecodeFailure.self) { try De.list(["a": 1], id) }
    }

    @Test func plexErrorTexts() {
        #expect(PlexError.noConnection("S").message == #"no reachable connection for server "S""#)
        #expect(PlexError.other("boom").message == "boom")
        #expect(PlexError.url("relative URL without a base").message == "invalid URL: relative URL without a base")
        #expect(PlexError.statusError("PUT", "/x", 500, "  oops  ").message == "PUT /x failed with HTTP 500: oops")
        #expect(
            PlexError.statusError("PUT", "/x", 500, String(repeating: "x", count: 300)).message
                == "PUT /x failed with HTTP 500: \(String(repeating: "x", count: 200))")
        #expect(PlexError.statusError("GET", "/x", 404, "").isNotFound())
        #expect(PlexError.decode("/p", "a.b: bad").message == "could not decode response from /p: a.b: bad")
        #expect(PlexError.unauthorized().localizedDescription == "not authorized (token missing, expired or lacking access)")
    }

    @Test func homeUserNames() throws {
        let u = try HomeUser(json: ["title": "Kid", "protected": "1"])
        #expect(u.protected)
        #expect(!u.admin)
        #expect(HomeUser.name(u) == "Kid")
    }

    @Test func plexTvUserAndServers() async throws {
        let fake = Fake(identifier: "c", product: "P", version: "1") { req in
            switch req.path {
            case "/api/v2/user": .json(#"{"id": 1, "username": "me", "authToken": "t", "extra": 1}"#)
            case "/api/v2/resources":
                .json(
                    #"[{"name": "A", "provides": "server,client", "clientIdentifier": "a", "connections": [{"uri": "http://x", "local": 1}]},"#
                        + #"{"name": "B", "provides": "player", "clientIdentifier": "b"}]"#)
            default: .json("{}")
            }
        }
        let tv = fake.tv("acct")
        let user = try await tv.user()
        #expect(user.username == "me")
        #expect(user.extra["extra"] == 1)
        #expect(fake.last.url.host == "plex.tv")
        #expect(fake.last.header("X-Plex-Token") == "acct")
        let servers = try await tv.servers()
        #expect(servers.map(\.name) == ["A"])
        #expect(servers[0].connections[0].local)
        #expect(fake.last.url.host == "clients.plex.tv")
        #expect(fake.last.pairs == ["includeHttps=1", "includeRelay=1", "includeIPv6=1"])
        #expect(tv.withToken("other").token == "other")
        let url = tv.authUrl("ABCD")
        #expect(url.hasPrefix("https://app.plex.tv/auth#?clientID=c&code=ABCD&context%5Bdevice%5D%5Bproduct%5D=P"))
    }

    @Test func plexTvSignInAndPins() async throws {
        let fake = Fake { req in
            switch req.path {
            case "/api/v2/pins": .json(#"{"id": 42, "code": "ABCD", "trusted": "1"}"#)
            case "/api/v2/pins/42": .json(#"{"id": "42", "code": "ABCD", "authToken": "tok"}"#)
            case "/api/v2/users/signin": .json(#"{"id": 1, "authToken": "t2", "subscription": {"active": 1, "plan": "lifetime"}}"#)
            case "/api/v2/home/users": .json(#"{"users": [{"id": 1, "title": "Me", "admin": true}]}"#)
            default: .json("{}")
            }
        }
        let tv = fake.tv()
        let pin = try await tv.createPin(true)
        #expect(pin.id == 42 && pin.trusted)
        #expect(fake.last.method == "POST")
        #expect(fake.last.pairs == ["strong=1"])
        #expect(fake.last.header("X-Plex-Token") == nil)
        #expect(try await tv.checkPin(42).authToken == "tok")
        let user = try await tv.signIn("me", password: "pw", code: "123")
        #expect(user.subscription?.active == true)
        #expect(fake.last.header("Content-Type") == "application/x-www-form-urlencoded")
        let home = try await tv.withToken("t2").homeUsers()
        #expect(home.map(\.title) == ["Me"])
        #expect(User.displayName(user) == "Plex user")
    }
}
