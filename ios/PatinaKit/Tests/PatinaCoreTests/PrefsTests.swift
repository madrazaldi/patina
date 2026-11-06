// Port of desktop/core/prefs.test.ts.

import Foundation
import PlexKit
import Testing

@testable import PatinaCore

private let AUDIO = StreamType.audio
private let SUBTITLE = StreamType.subtitle
private let VIDEO = StreamType.video

private func stream(_ id: Int, _ kind: Int, _ lang: String, _ title: String) -> MediaStream {
    MediaStream(id: id, streamType: kind, index: id, languageCode: lang, title: title.isEmpty ? nil : title)
}

private func selected(_ s: MediaStream) -> MediaStream {
    var s = s
    s.selected = true
    return s
}

private func forced(_ s: MediaStream) -> MediaStream {
    var s = s
    s.forced = true
    return s
}

private func part(_ streams: [MediaStream]) -> Part {
    Part(id: 1, stream: streams)
}

private func rules(_ f: (inout Prefs.Rules) -> Void) -> Prefs.Resolved {
    var r = Prefs.Rules()
    f(&r)
    return r.resolve()
}

@Suite struct PrefsTests {
    @Test func languagesMatchByCodeTagOrName() {
        #expect(Prefs.languageKey("en") == "eng")
        #expect(Prefs.languageKey("eng") == "eng")
        #expect(Prefs.languageKey("English") == "eng")
        #expect(Prefs.languageKey("en-US") == "eng")
        #expect(Prefs.languageKey("pt-BR") == "por")
        #expect(Prefs.languageKey("Portuguese (Brazil)") == "por")
        #expect(Prefs.languageKey("fre") == Prefs.languageKey("fra"))
        #expect(Prefs.languageKey("und") == "")
        #expect(Prefs.languageKey("Klingon") == "klingon")
        var tagged = stream(1, AUDIO, "", "")
        tagged.languageCode = nil
        tagged.languageTag = "ja"
        #expect(Prefs.streamLang(tagged) == "jpn")
    }

    @Test func commentaryIsFoundByTitle() {
        #expect(Prefs.isCommentary(stream(1, AUDIO, "eng", "Director's Commentary")))
        #expect(Prefs.isCommentary(stream(1, AUDIO, "eng", "English Audio Description")))
        #expect(Prefs.isCommentary(stream(1, SUBTITLE, "eng", "Commentary by the cast")))
        #expect(!Prefs.isCommentary(stream(1, AUDIO, "eng", "English (AC3 5.1)")))
        var byDisplay = stream(1, AUDIO, "eng", "")
        byDisplay.extendedDisplayTitle = "English (Commentary) (AC3 2.0)"
        #expect(Prefs.isCommentary(byDisplay))
    }

    @Test func picksAudioByLanguageOrder() {
        let p = part([selected(stream(1, AUDIO, "eng", "")), stream(2, AUDIO, "jpn", ""), stream(3, AUDIO, "fra", "")])
        #expect(Prefs.chooseStreams(rules { $0.audioLanguages = ["ja", "en"] }, p).audio == 2)
        #expect(Prefs.chooseStreams(rules { $0.audioLanguages = ["de", "French"] }, p).audio == 3)
        // None of the languages is there: Plex's choice stands.
        #expect(Prefs.chooseStreams(rules { $0.audioLanguages = ["ko"] }, p) == Prefs.Choice())
    }

    @Test func avoidsCommentaryButNotWhenItIsAllThereIs() {
        let p = part([selected(stream(1, AUDIO, "eng", "Commentary")), stream(2, AUDIO, "eng", "")])
        let avoid = rules {
            $0.audioLanguages = ["eng"]
            $0.avoidCommentary = true
        }
        #expect(Prefs.chooseStreams(avoid, p).audio == 2)
        // Without the rule the selected track stays.
        #expect(Prefs.chooseStreams(rules { $0.audioLanguages = ["eng"] }, p).audio == 1)
        // Only commentary exists in the language: use it.
        let only = part([stream(1, AUDIO, "eng", "Commentary")])
        #expect(Prefs.chooseStreams(avoid, only).audio == 1)
        // No language rule: still step off commentary.
        #expect(Prefs.chooseStreams(rules { $0.avoidCommentary = true }, p).audio == 2)
    }

    @Test func keepsTheSelectedTrackAmongEquals() {
        let p = part([stream(1, AUDIO, "eng", "AAC"), selected(stream(2, AUDIO, "eng", "AC3"))])
        #expect(Prefs.chooseStreams(rules { $0.audioLanguages = ["eng"] }, p).audio == 2)
    }

    @Test func subtitleModes() {
        let p = part([
            selected(stream(1, AUDIO, "jpn", "")),
            stream(2, SUBTITLE, "eng", ""),
            forced(stream(3, SUBTITLE, "eng", "Forced")),
            selected(stream(4, SUBTITLE, "fra", "")),
        ])
        func set(_ mode: Prefs.SubtitleMode, _ forcedMode: Prefs.ForcedMode?) -> Prefs.Resolved {
            rules {
                $0.subtitleLanguages = ["eng"]
                $0.subtitles = mode
                $0.forced = forcedMode
            }
        }
        #expect(Prefs.chooseStreams(set(.keep, nil), p).subtitle == nil)
        #expect(Prefs.chooseStreams(set(.off, nil), p).subtitle == .off)
        #expect(Prefs.chooseStreams(set(.always, nil), p).subtitle == .track(2))
        #expect(Prefs.chooseStreams(set(.always, .prefer), p).subtitle == .track(3))
        #expect(Prefs.chooseStreams(set(.always, .only), p).subtitle == .track(3))
        #expect(Prefs.chooseStreams(set(.always, .avoid), p).subtitle == .track(2))
        // A language with nothing in it leaves the selection alone.
        let none = rules {
            $0.subtitleLanguages = ["ko"]
            $0.subtitles = .always
        }
        #expect(Prefs.chooseStreams(none, p).subtitle == nil)
    }

    @Test func forcedSubtitlesFallBackToFullOnesOnlyInAuto() {
        let p = part([stream(1, AUDIO, "eng", ""), forced(stream(2, SUBTITLE, "eng", ""))])
        func make(_ mode: Prefs.ForcedMode) -> Prefs.Resolved {
            rules {
                $0.subtitleLanguages = ["eng"]
                $0.subtitles = .always
                $0.forced = mode
            }
        }
        #expect(Prefs.chooseStreams(make(.auto), p).subtitle == .track(2))
        #expect(Prefs.chooseStreams(make(.avoid), p).subtitle == nil)
        // A track titled "Forced" counts even without the flag.
        let titled = part([stream(1, SUBTITLE, "eng", "English Forced"), stream(2, SUBTITLE, "eng", "English")])
        #expect(Prefs.chooseStreams(make(.only), titled).subtitle == .track(1))
        #expect(Prefs.chooseStreams(make(.avoid), titled).subtitle == .track(2))
    }

    @Test func foreignModeFollowsTheAudioLanguage() {
        func streams(_ audio: String) -> [MediaStream] {
            [selected(stream(1, AUDIO, audio, "")), stream(2, SUBTITLE, "eng", ""), forced(stream(3, SUBTITLE, "eng", ""))]
        }
        let r = rules {
            $0.audioLanguages = ["eng"]
            $0.subtitles = .foreign
        }
        // English audio: only the forced track (foreign dialogue).
        #expect(Prefs.chooseStreams(r, part(streams("eng"))).subtitle == .track(3))
        // Japanese audio: full English subtitles.
        #expect(Prefs.chooseStreams(r, part(streams("jpn"))).subtitle == .track(2))
        // English audio and no forced track: subtitles off.
        let bare = part([selected(stream(1, AUDIO, "eng", "")), selected(stream(2, SUBTITLE, "eng", ""))])
        #expect(Prefs.chooseStreams(r, bare).subtitle == .off)
        // The audio the rules pick is the one that counts: Japanese is chosen and is "native" for
        // this profile, so only its forced track is shown.
        let both = part([
            selected(stream(1, AUDIO, "eng", "")),
            stream(4, AUDIO, "jpn", ""),
            stream(2, SUBTITLE, "eng", ""),
            forced(stream(3, SUBTITLE, "eng", "")),
        ])
        let jp = rules {
            $0.audioLanguages = ["jpn"]
            $0.subtitleLanguages = ["eng"]
            $0.subtitles = .foreign
        }
        let c = Prefs.chooseStreams(jp, both)
        #expect(c.audio == 4)
        #expect(c.subtitle == .track(3))
    }

    @Test func applyingAChoiceChangesOnlyWhatDiffers() {
        var p = part([
            selected(stream(1, AUDIO, "eng", "")),
            stream(2, AUDIO, "jpn", ""),
            selected(stream(3, SUBTITLE, "eng", "")),
            stream(4, SUBTITLE, "fra", ""),
        ])
        var changed = Prefs.applyChoice(&p, Prefs.Choice(audio: 2, subtitle: .off))
        #expect(changed.0 == 2 && changed.1 == 0)
        #expect(Part.selectedAudio(p)?.id == 2)
        #expect(Part.selectedSubtitle(p) == nil)
        // Already as asked: nothing to save.
        changed = Prefs.applyChoice(&p, Prefs.Choice(audio: 2, subtitle: .off))
        #expect(changed.0 == nil && changed.1 == nil)
        changed = Prefs.applyChoice(&p, Prefs.Choice(audio: nil, subtitle: .track(4)))
        #expect(changed.0 == nil && changed.1 == 4)
        #expect(Part.selectedSubtitle(p)?.id == 4)
    }

    private func version(_ res: String, _ codec: String, _ hdr: Bool) -> Media {
        var video = MediaStream(streamType: VIDEO)
        if hdr { video.colorTrc = "smpte2084" }
        return Media(videoCodec: codec, videoResolution: res, part: [Part(stream: [video])])
    }

    @Test func ranksVersions() {
        let versions = [version("1080", "h264", false), version("4k", "hevc", true), version("720", "h264", false)]
        func pick(_ f: (inout Prefs.Rules) -> Void) -> Int {
            Prefs.pickVersion(rules(f), versions)
        }
        #expect(pick { _ in } == 0)  // no preference keeps Plex's order
        #expect(pick { $0.resolution = .uhd } == 1)
        #expect(pick { $0.resolution = .fhd } == 0)
        #expect(pick { $0.resolution = .hd } == 2)
        #expect(pick { $0.codec = "H.265" } == 1)
        #expect(pick { $0.hdr = .prefer } == 1)
        #expect(pick { $0.hdr = .avoid } == 0)
        // 4K wanted, but HDR avoided: resolution weighs more.
        #expect(
            pick {
                $0.resolution = .uhd
                $0.hdr = .avoid
            } == 1)
        // 1080p equidistant from 4K and 720p: the higher one wins.
        let two = [version("720", "h264", false), version("4k", "hevc", false)]
        let fhd = Prefs.Rules(resolution: .fhd).resolve()
        #expect(Prefs.pickVersion(fhd, two) == 1)
        // A single version is never "changed".
        #expect(Prefs.pickVersion(fhd, Array(two.prefix(1))) == 0)
    }

    @Test func hdrIsAlsoRecognisedByVersionName() {
        var m = version("4k", "hevc", false)
        m.title = "2160p DV"
        #expect(Prefs.mediaIsHdr(m))
        m.title = "Director's Cut"
        #expect(!Prefs.mediaIsHdr(m))
    }

    @Test func showRulesOverrideTheProfileFieldByField() {
        let scope = Scope("alice", "home")
        var entries: [Prefs.Entry] = []
        Prefs.store(&entries, scope, nil, nil, Prefs.Rules(audioLanguages: ["eng"], subtitles: .off, avoidCommentary: true))
        Prefs.store(&entries, scope, "42", "Anime", Prefs.Rules(audioLanguages: ["jpn"], subtitles: .always))
        let anime = Prefs.effective(entries, scope, "42")
        #expect(anime.audioLanguages == ["jpn"])
        #expect(anime.subtitles == .always)
        #expect(anime.avoidCommentary == true)  // inherited from the profile
        #expect(Prefs.effective(entries, scope, "7").audioLanguages == ["eng"])
        #expect(Prefs.effective(entries, scope, nil).subtitles == .off)
        // Empty rules clear the show again.
        Prefs.store(&entries, scope, "42", nil, Prefs.Rules())
        #expect(Prefs.effective(entries, scope, "42").audioLanguages == ["eng"])
    }

    @Test func rulesNeverCrossAccountsOrServers() {
        let alice = Scope("alice", "home")
        let bob = Scope("bob", "home")
        let away = Scope("alice", "away")
        var entries: [Prefs.Entry] = []
        Prefs.store(&entries, alice, nil, nil, Prefs.Rules(audioLanguages: ["jpn"]))
        Prefs.store(&entries, alice, "1", "Show", Prefs.Rules(subtitles: .off))
        for other in [bob, away] {
            #expect(Prefs.effective(entries, other, "1").isEmpty)
            #expect(Prefs.inScope(entries, other).isEmpty)
        }
        // Saving for another scope leaves the first one untouched.
        Prefs.store(&entries, bob, nil, nil, Prefs.Rules(audioLanguages: ["eng"]))
        #expect(Prefs.effective(entries, alice, nil).audioLanguages == ["jpn"])
        #expect(Prefs.effective(entries, bob, nil).audioLanguages == ["eng"])
    }

    @Test func remembersTheTracksChosenInTheDialog() {
        let audio = stream(1, AUDIO, "jpn", "")
        let sub = stream(2, SUBTITLE, "eng", "")
        let r = Prefs.rulesFromTracks(audio, .stream(sub))
        #expect(r.audioLanguages == ["jpn"])
        #expect(r.subtitleLanguages == ["eng"])
        #expect(r.subtitles == .always && r.forced == .avoid)
        #expect(Prefs.rulesFromTracks(nil, .off).subtitles == .off)
        #expect(Prefs.rulesFromTracks(nil, .keep).isEmpty)
        #expect(Prefs.rulesFromTracks(nil, .stream(forced(stream(3, SUBTITLE, "eng", "")))).forced == .only)
    }

    @Test func rulesRoundTripThroughJson() throws {
        let r = Prefs.Rules(subtitles: .foreign, resolution: .uhd, hdr: .avoid)
        let data = try JSONEncoder().encode(r)
        let json = try JSONDecoder().decode(JSONValue.self, from: data)
        #expect(json["resolution"] == "4k")
        #expect(json["hdr"] == "avoid")
        #expect(json["subtitles"] == "foreign")
        #expect(json["audioLanguages"] == .null)  // unset fields are written as null, as the Rust core did
        #expect(Prefs.Rules.from(json) == r)
        #expect(Prefs.Rules.from([:]) == Prefs.Rules())
        // Invalid values are unset rather than failing the file.
        #expect(Prefs.Rules.from(["hdr": "sometimes", "codec": 3, "audioLanguages": ["eng", 1]]) == Prefs.Rules(audioLanguages: ["eng"]))
    }
}
