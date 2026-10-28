// Port of desktop/core/ops/xsearch.test.ts.

import Foundation
import PlexKit
import Testing

@testable import PatinaCore

@Suite struct XSearchTests {
    typealias X = Ops.XSearch

    func item(_ kind: String, _ rk: String, _ title: String, _ guid: String?, _ year: Int) throws -> Metadata {
        try Metadata(json: ["ratingKey": .string(rk), "type": .string(kind), "title": .string(title), "guid": guid.map(JSONValue.string) ?? .null, "year": .number(Double(year))])
    }

    func target(_ id: String, _ current: Bool) -> X.Target {
        X.Target(id: id, name: "Server \(id)", current: current, online: true)
    }

    @Test func sameGuidIsOneGroupWithACopyPerServer() throws {
        let (a, b) = (target("a", true), target("b", false))
        var r = X.Results("copper", [a, b])
        r.add(b, [try item("movie", "7", "Copper Sky", "plex://movie/1", 2015)])
        r.add(a, [try item("movie", "100", "Copper Sky", "plex://movie/1", 2015), try item("movie", "101", "Other", "plex://movie/2", 2000)])
        #expect(r.groups.count == 2)
        let g = r.groups[0]
        #expect(g.copies.count == 2)
        // The current server's copy comes first even though it answered second.
        #expect(g.copies[0].current)
        #expect(Metadata.serverId(g.copies[0].item) == nil)
        #expect(Metadata.serverId(g.copies[1].item) == "b")
        #expect(r.done)
    }

    @Test func itemsWithoutAUsableGuidGroupByTitleAndYear() throws {
        let (a, b) = (target("a", true), target("b", false))
        var r = X.Results("x", [a, b])
        r.add(a, [try item("movie", "1", "Paper Moons!", "local://5", 2019)])
        r.add(b, [try item("movie", "9", "paper moons", nil, 2019), try item("movie", "10", "Paper Moons", nil, 1999)])
        #expect(r.groups.count == 2)
        #expect(r.groups[0].copies.count == 2)
        #expect(r.groups[1].copies.count == 1)
    }

    @Test func episodesGroupByShowAndNumberWithoutAGuid() throws {
        func mk(_ rk: String) throws -> Metadata {
            try Metadata(json: [
                "ratingKey": .string(rk), "type": "episode", "title": "Relay", "grandparentTitle": "Harbor Lights",
                "parentIndex": 2, "index": 3,
            ])
        }
        #expect(X.groupKey(try mk("1")) == X.groupKey(try mk("2")))
        #expect(X.subtitle(try mk("1")) == "Harbor Lights S02E03")
    }

    @Test func duplicatesOnOneServerAreIgnoredButOtherItemsAreKept() throws {
        let a = target("a", true)
        var r = X.Results("x", [a])
        let m = try item("movie", "1", "Same", "plex://movie/1", 2000)
        r.add(a, [m, m, try item("movie", "2", "Same", "plex://movie/1", 2000)])
        #expect(r.groups.count == 1)
        #expect(r.groups[0].copies.count == 2)
    }

    @Test func aFailingServerDoesNotHideTheOthers() throws {
        let (a, b, c) = (target("a", true), target("b", false), target("c", false))
        var r = X.Results("x", [a, b, c])
        r.fail(b, .timedOut, "slow")
        #expect(!r.done)
        r.add(a, [try item("movie", "1", "Found", "plex://movie/1", 2000)])
        #expect(!r.done)
        r.fail(c, .failed, "refused")
        #expect(r.done)
        #expect(r.groups.count == 1)
        #expect(r.servers.map(\.state) == [.done, .timedOut, .failed])
        #expect(r.servers[1].error == "slow")
    }

    @Test func offlineServersAreNotWaitedFor() {
        var off = target("b", false)
        off.online = false
        let r = X.Results("x", [target("a", true), off])
        #expect(r.servers[1].state == .offline)
        #expect(!r.done)
    }

    @Test func versionsDescribeQuality() throws {
        let media = try Media.normalize([
            "videoResolution": "1080", "videoCodec": "hevc", "container": "mkv", "bitrate": 6200,
            "Part": [["size": 2_254_857_830]],
        ])
        let v = X.version(media)
        #expect(v.label == "1080p · HEVC · MKV · 6.2 Mbps · 2.1 GB")
        #expect(v.resolution == "1080p")
        #expect(X.version(try Media.normalize([:])).label == "Unknown quality")
        #expect(X.version(try Media.normalize(["videoResolution": "4k", "bitrate": 800])).label == "4K · 800 kbps")
    }
}
