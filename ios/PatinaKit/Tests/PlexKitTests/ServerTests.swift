// Port of desktop/plex/server.test.ts.
// Request shapes of the server client against a fake server: paths, query parameters and headers
// are the ones the Rust implementation sends.

import Foundation
import Testing

@testable import PlexKit

private let sse =
    "event: playing\ndata: {\"a\":1}\n\n: comment\ndata: plain\n\nevent: x\n\nevent: status\ndata: l1\ndata: l2\n\n"

private func pms() -> Fake {
    Fake { req in
        switch req.path {
        case "/identity": .json(#"{"MediaContainer": {"machineIdentifier": "m-1", "version": "1.0", "claimed": 1}}"#)
        case "/unauthorized": .text("nope", status: 401)
        case "/missing": .text("no such item", status: 404)
        case "/html": .text("<html>bad gateway</html>", status: 502)
        case "/empty": .text("")
        case "/library/sections/1/all":
            .json(#"{"MediaContainer": {"size": 1, "totalSize": 3, "offset": 0, "Metadata": [{"ratingKey": 7, "type": "movie", "title": "A"}]}}"#)
        case "/playQueues":
            .json(#"{"MediaContainer": {"playQueueID": 9, "playQueueSelectedItemID": 11, "Metadata": [{"ratingKey": "7", "playQueueItemID": 11}]}}"#)
        case "/image": StubResponse(headers: ["Content-Type": "image/jpeg"], chunks: [Data([1, 2, 3])])
        case "/range": .text("x")
        case "/:/eventsource/notifications":
            // Split mid-line, as a network would.
            StubResponse(headers: ["Content-Type": "text/event-stream"], chunks: [Data(sse.prefix(20).utf8), Data(sse.dropFirst(20).utf8)])
        case "/library/metadata/1/allLeaves":
            .json(#"{"MediaContainer": {"Metadata": [{"ratingKey": "3"}, {"ratingKey": "2"}, {"ratingKey": "1"}]}}"#)
        case "/library/metadata/3,2,1":
            // Reordered, and item 2 without streams.
            .json(
                #"{"MediaContainer": {"Metadata": [{"ratingKey": "1", "Media": [{"Part": [{"Stream": [{"id": 1}]}]}]}, "#
                    + #"{"ratingKey": "2", "Media": [{"Part": [{}]}]}, {"ratingKey": "3", "Media": [{"Part": [{"Stream": [{"id": 3}]}]}]}]}}"#)
        case "/library/metadata/2":
            .json(#"{"MediaContainer": {"Metadata": [{"ratingKey": "2", "title": "full", "Media": [{"Part": [{"Stream": [{"id": 2}]}]}]}]}}"#)
        default: .json(#"{"MediaContainer": {}}"#)
        }
    }
}

@Suite struct ServerTests {
    @Test func libraryItemsQuery() async throws {
        let fake = pms()
        let s = try fake.server()
        let mc = try await s.sectionItems(
            "1", query: LibraryQuery(kind: 4, sort: "addedAt:desc", unwatched: true, filters: Params().set("genre", 12), start: 0, size: 50))
        #expect(mc.metadata[0].ratingKey == "7")
        let r = fake.last
        #expect(r.method == "GET")
        #expect(r.path == "/library/sections/1/all")
        #expect(
            r.pairs == [
                "type=4", "sort=addedAt:desc", "unwatched=1", "genre=12", "includeGuids=1", "X-Plex-Container-Start=0",
                "X-Plex-Container-Size=50",
            ])
        #expect(r.header("x-plex-token") == "tok-123")
        #expect(r.header("x-plex-client-identifier") == "test-client")
        #expect(r.header("x-plex-product") == "Patina")
        #expect(r.header("x-plex-version") == "1.2.3")
        #expect(r.header("x-plex-model") == "desktop")
        #expect(r.header("x-plex-provides") == "player")
        #expect(r.header("accept") == "application/json")
        // the token travels in a header, not in the URL
        #expect(!r.query.contains { $0.key == "X-Plex-Token" })
    }

    @Test func libraryQueryWithoutSizeOrFilters() async throws {
        let fake = pms()
        _ = try await fake.server().sectionItems("2", query: .page(40, 0))
        #expect(fake.last.pairs == ["includeGuids=1", "X-Plex-Container-Start=40"])
    }

    @Test func timelineReport() async throws {
        let fake = pms()
        try await fake.server().timeline(
            TimelineUpdate(
                ratingKey: "7", key: "/library/metadata/7", state: .playing, timeMs: -5, durationMs: 120000,
                playQueueItemId: 11, session: "sess"))
        let r = fake.last
        #expect(r.method == "GET")
        #expect(r.path == "/:/timeline")
        #expect(
            r.pairs == [
                "ratingKey=7", "key=/library/metadata/7", "state=playing", "time=0", "duration=120000",
                "identifier=com.plexapp.plugins.library", "hasMDE=1", "playQueueItemID=11", "X-Plex-Session-Identifier=sess",
            ])
    }

    @Test func selectStreams() async throws {
        let fake = pms()
        let s = try fake.server()
        try await s.selectStreams(55, audio: 3, subtitle: 0, allParts: true)
        let r = fake.last
        #expect(r.method == "PUT")
        #expect(r.path == "/library/parts/55")
        #expect(r.pairs == ["audioStreamID=3", "subtitleStreamID=0", "allParts=1"])

        try await s.selectStreams(55, audio: nil, subtitle: nil, allParts: false)
        #expect(fake.last.query.isEmpty)
    }

    @Test func playQueueCreate() async throws {
        let fake = pms()
        let mc = try await fake.server().createPlayQueue(
            PlayQueueRequest(
                uri: "server://m/com.plexapp.plugins.library/library/metadata/7", kind: "video", key: "/library/metadata/7",
                continuous: true, includeChapters: true))
        #expect(mc.playQueueID == 9)
        let r = fake.last
        #expect(r.method == "POST")
        #expect(r.path == "/playQueues")
        #expect(
            r.pairs == [
                "type=video", "uri=server://m/com.plexapp.plugins.library/library/metadata/7", "key=/library/metadata/7",
                "shuffle=0", "repeat=0", "continuous=1", "includeChapters=1", "own=1",
            ])
    }

    @Test func metadataOptions() async throws {
        let fake = pms()
        _ = try await fake.server().metadata("7", opts: .full())
        #expect(fake.last.path == "/library/metadata/7")
        #expect(
            fake.last.pairs == [
                "includeExtras=1", "includeRelated=1", "includeMarkers=1", "includeChapters=1", "includeOnDeck=1",
                "includeReviews=1", "includeGuids=1", "asyncAugmentMetadata=0",
            ])
    }

    @Test func pathsWithAColonAndEncodedValues() async throws {
        let fake = pms()
        let s = try fake.server()
        try await s.scrobble("7")
        #expect(fake.last.path == "/:/scrobble")
        #expect(fake.last.pairs == ["key=7", "identifier=com.plexapp.plugins.library"])
        _ = try await s.searchHubs("a b&c", limit: 5, sectionId: nil)
        #expect(fake.last.path == "/hubs/search")
        #expect(fake.last.query[0] == ("query", "a b&c"))
        _ = try await s.lineupChannels("/tv.plex.providers.epg.cloud:2/", lineup: "lineup://x")
        #expect(fake.last.path == "/tv.plex.providers.epg.cloud:2/lineups/dvr/channels")
    }

    @Test func editMetadataAndTags() async throws {
        let fake = pms()
        let s = try fake.server()
        try await s.editMetadata("7", sectionId: 3, kind: 1, fields: [("title", "New")], lock: true)
        #expect(fake.last.method == "PUT")
        #expect(fake.last.path == "/library/sections/3/all")
        #expect(fake.last.pairs == ["type=1", "id=7", "title.value=New", "title.locked=1"])
        try await s.editTags("7", kind: 1, tag: "genre", add: ["A", "B"], remove: ["C", "D"])
        #expect(fake.last.path == "/library/metadata/7")
        #expect(fake.last.pairs == ["type=1", "id=7", "genre[0].tag.tag=A", "genre[1].tag.tag=B", "genre[].tag.tag-=C,D"])
    }

    @Test func transcodeUrlCarriesTheTokenAndParameters() throws {
        let s = try pms().server("http://127.0.0.1:32400")
        let url = try s.transcodeUrl(
            "/library/metadata/7", options: TranscodeOptions(session: "abc", maxVideoBitrate: 4000, offsetSecs: 12.5))
        let q = Dictionary(formDecode(URLComponents(url: url, resolvingAgainstBaseURL: false)!.percentEncodedQuery!), uniquingKeysWith: { a, _ in a })
        #expect(URLComponents(url: url, resolvingAgainstBaseURL: false)!.percentEncodedPath == "/video/:/transcode/universal/start.m3u8")
        #expect(q["X-Plex-Token"] == "tok-123")
        #expect(q["offset"] == "12.500")
        #expect(q["maxVideoBitrate"] == "4000")
        #expect(q["videoBitrate"] == "4000")
        #expect(q["session"] == "abc")
        #expect(q["directStream"] == "1")
        #expect(q["X-Plex-Platform"] == "Chrome")
        let audio = try s.audioTranscodeUrl("/library/metadata/9", options: TranscodeOptions(session: "abc"))
        let aq = Dictionary(formDecode(URLComponents(url: audio, resolvingAgainstBaseURL: false)!.percentEncodedQuery!), uniquingKeysWith: { a, _ in a })
        #expect(audio.path == "/music/:/transcode/universal/start.mp3")
        #expect(aq["protocol"] == "http")
        #expect(aq["musicBitrate"] == "320")
        // The whole URL, as the TypeScript builds it.
        #expect(
            url.absoluteString
                == "http://127.0.0.1:32400/video/:/transcode/universal/start.m3u8?path=%2Flibrary%2Fmetadata%2F7&mediaIndex=0&partIndex=0&protocol=hls&fastSeek=1&directPlay=0&directStream=1&directStreamAudio=1&subtitles=auto&maxVideoBitrate=4000&videoBitrate=4000&offset=12.500&copyts=1&mediaBufferSize=102400&session=abc&X-Plex-Session-Identifier=abc&X-Plex-Client-Identifier=test-client&X-Plex-Product=Patina&X-Plex-Version=1.2.3&X-Plex-Device=macOS&X-Plex-Platform=Chrome&X-Plex-Client-Profile-Extra=add-transcode-target-audio-codec%28type%3DvideoProfile%26context%3Dstreaming%26protocol%3Dhls%26audioCodec%3Dac3%2Ceac3%29&X-Plex-Token=tok-123"
        )
    }

    @Test func authedUrlsAndLibraryUris() throws {
        let base = "http://127.0.0.1:32400"
        let s = try pms().server("\(base)/prefix", token: "tok").withMachineIdentifier("mach")
        #expect(try s.url("/library/x").absoluteString == "\(base)/prefix/library/x")
        #expect(s.itemUri(["1", "2"]) == "server://mach/com.plexapp.plugins.library/library/metadata/1,2")
        let u = try s.authedUrl("/library/x?a=b%20c", Params().set("d", "e f"))
        #expect(u.query(percentEncoded: true) == "a=b%20c&d=e+f&X-Plex-Token=tok")
        #expect(try s.url("http://other/x").absoluteString == "http://other/x")
        #expect(try s.imageUrl("/library/metadata/1/thumb", width: 100, height: 150).absoluteString
            == "\(base)/prefix/photo/:/transcode?url=%2Flibrary%2Fmetadata%2F1%2Fthumb&width=100&height=150&minSize=1&upscale=1&X-Plex-Token=tok")
        #expect(try s.partUrl("/library/parts/8/1/file.mkv", download: true).absoluteString
            == "\(base)/prefix/library/parts/8/1/file.mkv?download=1&X-Plex-Token=tok")
        #expect(s.baseUrl.absoluteString == "\(base)/prefix/")
        #expect(s == (try pms().server("\(base)/prefix", token: "tok")))
        #expect(!s.description.contains("tok"))
    }

    @Test func errorsUnauthorizedStatusHtmlSnippetEmptyBody() async throws {
        let s = try pms().server()
        await #expect {
            try await s.raw("GET", "/unauthorized")
        } throws: { e in
            let e = e as! PlexError
            return e.message == "not authorized (token missing, expired or lacking access)" && e.status == 401
        }
        await #expect {
            try await s.raw("GET", "/missing")
        } throws: { e in
            let e = e as! PlexError
            return e.message == "GET /missing failed with HTTP 404: no such item" && e.isNotFound()
        }
        await #expect {
            try await s.raw("GET", "/html")
        } throws: { e in
            (e as! PlexError).message == "GET /html failed with HTTP 502"
        }
        #expect(try await s.raw("GET", "/empty") == [:])
        #expect(try await s.fetch("/empty").metadata.isEmpty)
    }

    @Test func networkErrorsReadLikeTheRustOnesAndHideTheToken() async throws {
        let fake = Fake { _ in .down }
        let s = try fake.server("http://127.0.0.1:1", token: "secret-token")
        await #expect {
            try await s.raw("GET", "/x", Params().set("X-Plex-Token", "secret-token"))
        } throws: { e in
            let e = e as! PlexError
            return e.kind == .http && e.message.hasPrefix("network error: error sending request for url (http://127.0.0.1:1/x")
                && !e.message.contains("secret-token")
        }
    }

    @Test func bytesReturnsTheContentTypeIdentityDecodes() async throws {
        let s = try pms().server()
        let (bytes, ctype) = try await s.bytes("/image")
        #expect([UInt8](bytes) == [1, 2, 3])
        #expect(ctype == "image/jpeg")
        let id = try await s.identity()
        #expect(id.machineIdentifier == "m-1")
        #expect(id.claimed)
    }

    @Test func rangedDownloadHeaders() async throws {
        let fake = pms()
        let s = try fake.server()
        let resp = try await s.downloadRange("/range", offset: 100, validator: "etag-1")
        #expect(resp.status == 200)
        let r = fake.last
        #expect(r.pairs == ["download=1"])
        #expect(r.header("range") == "bytes=100-")
        #expect(r.header("if-range") == "etag-1")
        #expect(r.header("accept-encoding") == "identity")
        _ = try await s.downloadRange("/range", offset: 0)
        #expect(fake.last.header("range") == nil)
        let req = try s.downloadRequest("/range", offset: 5, validator: nil)
        #expect(req.value(forHTTPHeaderField: "Range") == "bytes=5-")
        #expect(req.value(forHTTPHeaderField: "X-Plex-Token") == "tok-123")
        #expect(req.url?.query == "download=1")
    }

    @Test func notificationsStream() async throws {
        let fake = pms()
        var got: [PlexNotification] = []
        for try await n in try await fake.server().notifications(["playing", "status"]) { got.append(n) }
        #expect(fake.last.pairs == ["filters=playing,status"])
        #expect(fake.last.header("accept") == "text/event-stream")
        #expect(fake.last.header("x-plex-token") == "tok-123")
        #expect(
            got == [
                PlexNotification(event: "playing", data: ["a": 1]),
                PlexNotification(event: "message", data: "plain"),
                PlexNotification(event: "status", data: "l1\nl2"),
            ])
    }

    @Test func notificationsStreamCanBeAborted() async throws {
        let fake = Fake { _ in
            StubResponse(headers: ["Content-Type": "text/event-stream"], chunks: [Data("event: a\ndata: 1\n\n".utf8)], hang: true)
        }
        let s = try fake.server()
        let task = Task {
            var got: [String] = []
            for try await n in try await s.notifications() {
                got.append(n.event)
                break
            }
            return got
        }
        #expect(try await task.value == ["a"])

        // Cancelling the consuming task ends the iteration too.
        let stream = try await s.notifications()
        let consumer = Task {
            var count = 0
            for try await _ in stream { count += 1 }
            return count
        }
        try await Task.sleep(for: .milliseconds(100))
        consumer.cancel()
        #expect(try await consumer.value == 1)
    }

    @Test func sseParserHandlesCrlf() {
        var p = SSEParser()
        var out: [PlexNotification] = []
        for line in ["event: a\r", "data: {\"b\":2}\r", "\r", "data:x", ""] {
            if let n = p.line(Array(line.utf8)) { out.append(n) }
        }
        #expect(out == [PlexNotification(event: "a", data: ["b": 2]), PlexNotification(event: "message", data: "x")])
    }

    @Test func allLeavesWithStreamsKeepsOrderAndFillsMissingStreams() async throws {
        let fake = pms()
        let items = try await fake.server().allLeavesWithStreams("1")
        #expect(items.map(Metadata.ratingKey) == ["3", "2", "1"])
        #expect(items[1].title == "full")
    }

    @Test func connectResourcePicksTheReachableConnectionAndBindsTheMachineId() async throws {
        let fake = Fake { req in
            if req.url.port == 1 { return .down }
            return req.path == "/identity" ? .json(#"{"MediaContainer": {"machineIdentifier": "m-1"}}"#) : .json("{}")
        }
        let base = "http://127.0.0.1:32400"
        let tv = fake.tv("account-token")
        var resource = Resource()
        resource.name = "Den"
        resource.accessToken = "server-token"
        var unreachable = Connection()
        unreachable.uri = "http://127.0.0.1:1"
        unreachable.local = true
        unreachable.address = "127.0.0.1"
        unreachable.port = 1
        var remote = Connection()
        remote.uri = base
        resource.connections = [unreachable, remote]
        let s = try await connectResource(tv, resource)
        #expect(s.baseUrl.absoluteString == "\(base)/")
        #expect(s.machineIdentifier == "m-1")
        #expect(s.token == "server-token")
        #expect(fake.seen.contains { $0.path == "/identity" && $0.header("x-plex-token") == "server-token" })

        var none = Resource()
        none.name = "Gone"
        none.accessToken = "t"
        await #expect {
            try await connectResource(tv, none)
        } throws: { e in
            (e as? PlexError)?.message == #"no reachable connection for server "Gone""#
        }
        var noToken = Resource()
        noToken.name = "X"
        noToken.connections = [remote]
        await #expect {
            try await connectResource(fake.tv(), noToken)
        } throws: { e in
            (e as? PlexError)?.message == "not authorized (token missing, expired or lacking access)"
        }
    }

    @Test func connectResourcePrefersALocalConnectionThatAnswersWithinTheGracePeriod() async throws {
        let fake = Fake { req in
            req.path == "/identity" ? .json(#"{"MediaContainer": {"machineIdentifier": "m-\#(req.url.port!)"}}"#) : .json("{}")
        }
        var resource = Resource()
        resource.accessToken = "t"
        var remote = Connection()
        remote.uri = "http://10.0.0.1:2"
        var local = Connection()
        local.uri = "http://192.168.1.2:3"
        local.local = true
        resource.connections = [remote, local]
        let s = try await connectResource(fake.tv(), resource)
        #expect(s.machineIdentifier == "m-3")
    }
}
