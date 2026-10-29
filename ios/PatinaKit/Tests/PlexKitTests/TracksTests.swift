// Port of desktop/plex/tracks.test.ts (itself a port of crates/plex/tests/tracks.rs).

import Testing

@testable import PlexKit

private func audio(_ id: Int, _ lang: String, _ codec: String, _ channels: Int, _ title: String?) -> MediaStream {
    MediaStream(id: id, streamType: StreamType.audio, codec: codec, index: id, languageCode: lang, title: title, channels: channels)
}

private func sub(_ id: Int, _ lang: String, _ forced: Bool, _ sdh: Bool, _ title: String?) -> MediaStream {
    MediaStream(
        id: id, streamType: StreamType.subtitle, forced: forced, hearingImpaired: sdh, codec: "srt", index: id,
        languageCode: lang, title: title)
}

private func part(_ streams: [MediaStream]) -> Part {
    Part(id: 1, stream: streams)
}

private func matched(_ p: Part, _ template: MediaStream) -> Int? {
    Part.matchingStream(p, template)?.id
}

@Suite struct TracksTests {
    @Test func audioRequiresLanguageAndPrefersClosest() {
        let template = audio(0, "jpn", "flac", 2, nil)
        let p = part([audio(1, "eng", "flac", 2, nil), audio(2, "jpn", "aac", 6, nil), audio(3, "jpn", "flac", 2, nil)])
        #expect(matched(p, template) == 3)

        let englishOnly = part([audio(1, "eng", "flac", 2, nil)])
        #expect(matched(englishOnly, template) == nil)
    }

    @Test func titleOutweighsCodec() {
        let template = audio(0, "eng", "ac3", 6, "Commentary")
        let p = part([audio(1, "eng", "ac3", 6, nil), audio(2, "eng", "aac", 2, "Commentary")])
        #expect(matched(p, template) == 2)
    }

    @Test func subtitlesKeepForcedApart() {
        let full = sub(0, "eng", false, false, nil)
        let forced = sub(0, "eng", true, false, "Signs")
        let p = part([sub(1, "eng", true, false, "Signs"), sub(2, "eng", false, true, nil), sub(3, "eng", false, false, nil)])
        #expect(matched(p, full) == 3)
        #expect(matched(p, forced) == 1)

        // Only forced subs available: a full-subtitle choice finds nothing.
        let onlyForced = part([sub(1, "eng", true, false, nil)])
        #expect(matched(onlyForced, full) == nil)
    }

    @Test func tiesGoToTheFirstStream() {
        let template = audio(0, "eng", "aac", 2, nil)
        let p = part([audio(5, "eng", "aac", 2, nil), audio(6, "eng", "aac", 2, nil)])
        #expect(matched(p, template) == 5)
    }

    @Test func streamLabels() {
        #expect(MediaStream.label(MediaStream(index: 3)) == "Track 3")
        #expect(MediaStream.label(MediaStream()) == "Track 0")
        #expect(MediaStream.shortLabel(MediaStream(title: "x", displayTitle: "English (AC3 5.1)")) == "English (AC3 5.1)")
    }
}
