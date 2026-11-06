// Port of desktop/plex/decode.test.ts (itself a port of crates/plex/tests/decode.rs).

import Foundation
import Testing

@testable import PlexKit

private func envelope(_ json: String) throws -> MediaContainer {
    try Envelope(json: JSONValue.parse(json)).mediaContainer
}

@Suite struct DecodeTests {
    @Test func toleratesPlexTypeQuirks() throws {
        // ids as numbers or strings, booleans as 0/1/"1", unknown fields kept in `extra`.
        let json = """
            {"MediaContainer":{"size":"1","totalSize":40,"offset":0,"allowSync":1,"friendlyName":"Den",
              "Metadata":[{"ratingKey":123,"type":"episode","title":"Pilot","index":"1","parentIndex":2,"viewOffset":"60000","duration":120000,
                "grandparentTitle":"Show","someNewField":{"x":1},
                "Media":[{"id":"7","optimizedForStreaming":"1","videoResolution":1080,"Part":[{"id":8,"key":"/library/parts/8/1/file.mkv",
                  "Stream":[{"id":9,"streamType":2,"index":1,"selected":true,"DOVIPresent":0},{"id":10,"streamType":3,"key":"/library/streams/10","selected":"1"}]}]}],
                "Marker":[{"type":"intro","startTimeOffset":1000,"endTimeOffset":30000,"final":"1"}]}]}}
            """
        let mc = try envelope(json)
        #expect(mc.size == 1)
        #expect(MediaContainer.hasMore(mc))
        #expect(MediaContainer.attr(mc, "friendlyName") == "Den")
        #expect(MediaContainer.attr(mc, "allowSync") == "1")
        #expect(MediaContainer.attr(mc, "size") == nil)
        let m = mc.metadata[0]
        #expect(Metadata.ratingKey(m) == "123")
        #expect(Metadata.episodeCode(m) == "S02E01")
        #expect(Metadata.progress(m) == 0.5)
        #expect(m.extra["someNewField"] != nil)
        let media = m.media[0]
        #expect(media.optimizedForStreaming)
        #expect(media.videoResolution == "1080")
        let part = media.part[0]
        #expect(Part.selectedAudio(part)?.id == 9)
        #expect(MediaStream.isExternal(Part.selectedSubtitle(part)!))
        #expect(Metadata.markersOf(m, "intro")[0].final)
    }

    @Test func ranksConnections() throws {
        let r = try Resource(
            json: JSONValue.parse(
                """
                {"name":"S","provides":"server","connections":[
                  {"protocol":"https","address":"1.2.3.4","port":32400,"uri":"https://relay.plex.direct:8443","local":false,"relay":true},
                  {"protocol":"https","address":"192.168.1.5","port":32400,"uri":"https://192-168-1-5.abc.plex.direct:32400","local":true,"relay":false}]}
                """))
        #expect(Resource.isServer(r))
        let c = ConnectionCandidate.candidates(r)
        #expect(c[0].local && !c[0].relay)
        #expect(c.contains { $0.uri == "http://192.168.1.5:32400" })
        #expect(c.last!.relay)
    }

    @Test func acceptsSingleObjectWhereAListIsExpected() throws {
        // Real servers send `OnDeck.Metadata` as an object, not an array.
        let json = """
            {"MediaContainer":{"Metadata":[{"ratingKey":"2606","type":"show","title":"Show",
              "OnDeck":{"Metadata":{"ratingKey":"2610","type":"episode","title":"Next"}},
              "Related":{"Hub":[{"hubIdentifier":"similar","title":"Similar","Metadata":[]}]},
              "Extras":{"size":0}}]}}
            """
        let m = try envelope(json).metadata[0]
        #expect(Metadata.onDeckItem(m).map(Metadata.ratingKey) == "2610")
        #expect(m.related?.hub.count == 1)
    }

    // ---- additions: the serialized shape the UI and downloads.json rely on ----

    @Test func serializedShape() throws {
        let m = try envelope(
            #"{"MediaContainer":{"Metadata":[{"ratingKey":5,"type":"movie","collectionSort":"1","accessToken":"secret"}]}}"#
        ).metadata[0]
        let out = try JSONValue.parse(JSONEncoder().encode(m))
        #expect(out["ratingKey"] == "5")
        #expect(out["type"] == "movie")
        #expect(out["title"] == .null)
        #expect(out["Media"] == [])
        #expect(out["smart"] == false)
        #expect(out["collectionSort"] == "1")
        #expect(out["User"] == .null)
        #expect(out["Extras"] == .null)
        #expect(Media.label(Media()) == "")
    }

    @Test func serverTagRoundTripsThroughExtra() throws {
        var m = try envelope(#"{"MediaContainer":{"Metadata":[{"ratingKey":"1"}]}}"#).metadata[0]
        #expect(Metadata.serverId(m) == nil)
        Metadata.setServerId(&m, "abc")
        #expect(Metadata.serverId(m) == "abc")
        #expect(try JSONValue.parse(JSONEncoder().encode(m))["patinaServer"] == "abc")
        Metadata.setServerId(&m, nil)
        #expect(Metadata.serverId(m) == nil)
        #expect(m.extra["patinaServer"] == nil)
    }

    @Test func mediaLabel() throws {
        let m = try envelope(
            """
            {"MediaContainer":{"Metadata":[{"Media":[{"videoResolution":"4k","videoCodec":"hevc","audioCodec":"truehd","audioChannels":8,
              "Part":[{"Stream":[{"streamType":1,"DOVIPresent":1}]}]}]}]}}
            """
        ).metadata[0].media[0]
        #expect(Media.label(m) == "4K · HEVC · Dolby Vision · TRUEHD 7.1")
    }

    @Test func decodeErrorsNameThePath() throws {
        #expect {
            try envelope(#"{"MediaContainer":{"Metadata":[{"Media":"x"}]}}"#)
        } throws: { e in
            (e as? DecodeFailure)?.message
                == #"MediaContainer.Metadata[0].Media: invalid type: string "x", expected a sequence"#
        }
    }
}
