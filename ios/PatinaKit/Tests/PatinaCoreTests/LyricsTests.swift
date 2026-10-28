// Port of desktop/core/lyrics.test.ts.

import Testing

@testable import PatinaCore

@Suite struct LyricsTests {
    @Test func parsesLrc() {
        let l = Lyrics.parse("[ti:Song]\n[00:01.50]One\n[00:03.00][00:10.00]Two\n")
        #expect(l.synced)
        #expect(l.lines.map(\.timeMs) == [1500, 3000, 10000])
        #expect(l.lines[2].text == "Two")
    }

    @Test func keepsPlainText() {
        let l = Lyrics.parse("First line\n\nSecond\n\n")
        #expect(!l.synced)
        #expect(l.lines.count == 3)
    }

    // No TypeScript twin: Windows line ends, which Swift reads as one Character.
    @Test func crlfLinesSplit() {
        let l = Lyrics.parse("[00:01.5]One\r\n[00:02,25]Two\r\n")
        #expect(l.lines.map(\.timeMs) == [1500, 2250])
        #expect(l.lines.map(\.text) == ["One", "Two"])
    }
}
