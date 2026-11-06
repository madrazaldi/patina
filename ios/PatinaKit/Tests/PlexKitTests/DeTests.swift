// Tests for the lenient decoding helpers (desktop/plex/de.ts) and the JSON round trip of the
// models. The TypeScript covers part of this in client.test.ts and decode.test.ts.

import Foundation
import Testing

@testable import PlexKit

@Suite struct DeTests {
    @Test func numbersFromStrings() {
        #expect(De.optI64("1e3") == 1000)
        #expect(De.optI64("+7") == 7)
        #expect(De.optI64(".5") == 0)
        #expect(De.optI64("1.") == 1)
        #expect(De.optI64("0x10") == nil)
        #expect(De.optI64("1 2") == nil)
        #expect(De.optI64("inf") == Int.max)
        #expect(De.optI64("-Infinity") == -Int.max)
        #expect(De.optI64("NaN") == 0)
        #expect(De.optF64("nan")?.isNaN == true)
        #expect(De.optI64(1e30) == Int.max)
        #expect(De.optString(1.5) == "1.5")
        #expect(De.optString(1e21) == "1e+21")
        #expect(De.optString(12345678901) == "12345678901")
    }

    @Test func objAndDescriptions() {
        #expect {
            try De.obj("x", "Media")
        } throws: { e in
            (e as? DecodeFailure)?.message == #"invalid type: string "x", expected struct Media"#
        }
        #expect(De.describe(1) == "integer `1`")
        #expect(De.describe(1.5) == "floating point `1.5`")
        #expect(De.describe(true) == "boolean `true`")
        #expect(De.describe([1]) == "sequence")
        #expect(De.describe([:]) == "map")
        #expect(De.describe(nil) == "null")
    }

    @Test func failurePathsNest() throws {
        let inner = DecodeFailure("", "bad")
        #expect(inner.under("Media").message == "Media: bad")
        #expect(inner.under("Media").under("[0]").message == "[0].Media: bad")
        #expect(DecodeFailure("[2]", "bad").under("Part").message == "Part[2]: bad")
        #expect {
            try De.list([1, "x"], { v in try De.obj(v, "Part") }, key: "Part")
        } throws: { e in
            (e as? DecodeFailure)?.message == "Part[0]: invalid type: integer `1`, expected struct Part"
        }
        #expect(try De.optional(.null, { $0 }, key: "a") == nil)
        #expect(De.value(nil) == .null)
        #expect(De.extraOf(["a": 1, "b": 2], ["a"]) == ["b": 2])
    }

    @Test func pointer() {
        let v: JSONValue = ["A": ["B": [10, 20]], "a/b": 1, "n": nil]
        #expect(De.pointer(v, "/A/B/1") == 20)
        #expect(De.pointer(v, "/A/B/01") == nil)
        #expect(De.pointer(v, "/A/C") == nil)
        #expect(De.pointer(v, "/a~1b") == 1)
        #expect(De.pointer(v, "/n") == .null)
        #expect(De.pointer(v, "") == v)
    }

    @Test func emptyBodiesAndBadJson() throws {
        #expect(try decode("/p", Data(" \n".utf8)) { $0 } == [:])
        #expect {
            try decode("/p", Data("{bad".utf8)) { $0 }
        } throws: { e in
            (e as? PlexError)?.message.hasPrefix("could not decode response from /p: ") == true
        }
        #expect {
            try decode("/p", Data(#"{"MediaContainer": {"size": 1, "Metadata": {"a": 1}}}"#.utf8), Envelope.init(json:))
        } throws: { e in
            (e as? PlexError)?.message == "could not decode response from /p: MediaContainer.Metadata: invalid type: map, expected a sequence"
        }
    }

    @Test func modelsRoundTripThroughCodable() throws {
        let json = """
            {"MediaContainer":{"size":2,"identifier":"x","friendlyName":"Den","Meta":{"Type":[{"key":"1","Filter":[{"filter":"genre"}]}]},
              "Setting":[{"id":"a","value":true,"default":{"x":[1,"y"]}}],"Activity":[{"uuid":"u","Context":{"key":"/k"}}],
              "Metadata":[{"ratingKey":"1","type":"episode","index":"3","rating":"7.5","Guid":[{"id":"imdb://tt1"}],"Rating":[{"value":8}],
                "User":{"id":1},"Media":[{"Part":[{"Stream":[{"id":1,"DOVIPresent":"1","default":1}]}]}],
                "Stations":{"title":"Radio"},"OnDeck":{"Metadata":{"ratingKey":"2"}},"patinaServer":"abc","odd":[1,{"z":null}]}]}}
            """
        let env = try Envelope(json: JSONValue.parse(json))
        let data = try JSONEncoder().encode(env)
        let back = try JSONDecoder().decode(Envelope.self, from: data)
        #expect(back == env)
        let m = back.mediaContainer.metadata[0]
        #expect(m.guids[0].id == "imdb://tt1")
        #expect(m.guid == nil)
        #expect(m.ratings[0].value == 8)
        #expect(m.rating == 7.5)
        #expect(m.stations.map(\.title) == ["Radio"])
        #expect(m.media[0].part[0].stream[0].doviPresent)
        #expect(m.media[0].part[0].stream[0].default)
        #expect(Metadata.serverId(m) == "abc")
        #expect(m.extra["odd"] == [1, ["z": nil]])
        #expect(back.mediaContainer.setting[0].default == ["x": [1, "y"]])
        #expect(Setting.valueString(back.mediaContainer.setting[0]) == "true")
        #expect(back.mediaContainer.activity[0].context["key"] == "/k")
        #expect(back.mediaContainer.meta?.type[0].filter[0].filter == "genre")
        // The encoded keys are Plex's.
        let out = try JSONValue.parse(data)
        #expect(out["MediaContainer"]?["Metadata"]?[0]?["Media"]?[0]?["Part"]?[0]?["Stream"]?[0]?["DOVIPresent"] == true)
        #expect(out["MediaContainer"]?["Metadata"]?[0]?["Guid"] == [["id": "imdb://tt1"]])
        #expect(out["MediaContainer"]?["friendlyName"] == "Den")
        // A model stored as JSON (downloads.json, the saved queue) reads back through `normalize`.
        #expect(try Metadata.normalize(m.json) == m)
    }

    @Test func modelHelpers() throws {
        var m = Metadata()
        m.type = "episode"
        m.grandparentTitle = "Show"
        m.title = "Pilot"
        m.index = 2
        #expect(Metadata.fullTitle(m) == "Show — E02 · Pilot")
        m.guid = "plex://episode/abc123"
        #expect(Metadata.discoverId(m) == "abc123")
        #expect(Metadata.isPlayable(m))
        #expect(!Metadata.isContainer(m))
        m.leafCount = 4
        m.viewedLeafCount = 4
        #expect(Metadata.isWatched(m))
        #expect(Metadata.unwatchedLeaves(m) == 0)
        var s = Setting()
        s.enumValues = "0:Off|1:On|x"
        #expect(Setting.enumOptions(s).map { "\($0.0)=\($0.1)" } == ["0=Off", "1=On", "x=x"])
        var r = Resource()
        r.provides = "client,server"
        #expect(Resource.isServer(r))
        #expect(Resource.name(r) == "Plex Media Server")
        #expect(Directory.title(Directory()) == "")
        #expect(MediaType.fromName("photoalbum") == .photoAlbum)
        #expect(MediaType.episode.id == 4)
        #expect(channelLabel(6) == "5.1")
    }
}
