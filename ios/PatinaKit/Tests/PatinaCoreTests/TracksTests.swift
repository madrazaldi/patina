// Port of desktop/core/tracks.test.ts.

import Foundation
import PlexKit
import Testing

@testable import PatinaCore

private func stream(_ id: Int, _ kind: Int, _ lang: String, _ selected: Bool) -> MediaStream {
    MediaStream(id: id, streamType: kind, selected: selected, index: id, languageCode: lang)
}

private func episode(_ n: Int, _ streams: [MediaStream]) -> Metadata {
    var m = Metadata(index: n, parentIndex: 1)
    m.media = [Media(part: [Part(id: n, stream: streams)])]
    return m
}

private func label(_ e: Metadata) -> String {
    "\(Metadata.episodeCode(e)) · \(Metadata.title(e))"
}

@Suite struct TracksTests {
    @Test func plansOnlyWhatChanges() {
        let a = StreamType.audio
        let s = StreamType.subtitle
        let eps = [
            // Already on Japanese audio.
            episode(1, [stream(11, a, "eng", false), stream(12, a, "jpn", true), stream(13, s, "eng", true)]),
            // English selected; Japanese available.
            episode(2, [stream(21, a, "eng", true), stream(22, a, "jpn", false)]),
            // No Japanese at all.
            episode(3, [stream(31, a, "eng", true), stream(32, s, "eng", false)]),
        ]
        let tpl = stream(0, a, "jpn", false)

        var p = Tracks.plan(eps, tpl, .off)
        #expect(
            p.changes == [
                Tracks.PartChange(partId: 1, audio: nil, subtitle: 0), Tracks.PartChange(partId: 2, audio: 22, subtitle: nil),
            ])
        #expect(p.episodes == 2)
        #expect(p.noAudio == [label(eps[2])])

        let subTpl = stream(0, s, "eng", false)
        p = Tracks.plan(eps, nil, .stream(subTpl))
        #expect(p.changes == [Tracks.PartChange(partId: 3, audio: nil, subtitle: 32)])
        #expect(p.noSubtitle == [label(eps[1])])
    }

    @Test func parsesSubtitleChoice() {
        #expect(Tracks.parseSubChoice("keep") == .keep)
        #expect(Tracks.parseSubChoice("off") == .off)
        #expect(Tracks.parseSubChoice("42") == .track(42))
    }
}
