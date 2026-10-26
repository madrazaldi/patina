import { expect, test } from "bun:test";

import { type Media, type Part, Part as PartNs, type Stream, defaultMedia, defaultPart, defaultStream, streamType } from "../plex";
import {
  type Choice,
  type ForcedMode,
  type HdrMode,
  Rules,
  type RulesFields,
  type SubtitleMode,
  applyChoice,
  chooseStreams,
  effective,
  inScope,
  isCommentary,
  languageKey,
  mediaIsHdr,
  noChoice,
  pickVersion,
  rulesFromTracks,
  store,
  streamLang,
} from "./prefs";
import { Scope } from "./recovery";
import type { Entry } from "./prefs";

const { AUDIO, SUBTITLE, VIDEO } = streamType;

function stream(id: number, kind: number, lang: string, title: string): Stream {
  return defaultStream({ id, streamType: kind, languageCode: lang, title: title === "" ? null : title, index: id });
}
function selected(s: Stream): Stream {
  s.selected = true;
  return s;
}
function forced(s: Stream): Stream {
  s.forced = true;
  return s;
}
function part(streams: Stream[]): Part {
  return defaultPart({ id: 1, Stream: streams });
}
function rules(f: (r: Rules) => void) {
  const r = new Rules();
  f(r);
  return r.resolve();
}
function langs(l: string[]): string[] {
  return l;
}

test("languages_match_by_code_tag_or_name", () => {
  expect(languageKey("en")).toBe("eng");
  expect(languageKey("eng")).toBe("eng");
  expect(languageKey("English")).toBe("eng");
  expect(languageKey("en-US")).toBe("eng");
  expect(languageKey("pt-BR")).toBe("por");
  expect(languageKey("Portuguese (Brazil)")).toBe("por");
  expect(languageKey("fre")).toBe(languageKey("fra"));
  expect(languageKey("und")).toBe("");
  expect(languageKey("Klingon")).toBe("klingon");
  const tagged = stream(1, AUDIO, "", "");
  tagged.languageCode = null;
  tagged.languageTag = "ja";
  expect(streamLang(tagged)).toBe("jpn");
});

test("commentary_is_found_by_title", () => {
  expect(isCommentary(stream(1, AUDIO, "eng", "Director's Commentary"))).toBe(true);
  expect(isCommentary(stream(1, AUDIO, "eng", "English Audio Description"))).toBe(true);
  expect(isCommentary(stream(1, SUBTITLE, "eng", "Commentary by the cast"))).toBe(true);
  expect(isCommentary(stream(1, AUDIO, "eng", "English (AC3 5.1)"))).toBe(false);
  const byDisplay = stream(1, AUDIO, "eng", "");
  byDisplay.extendedDisplayTitle = "English (Commentary) (AC3 2.0)";
  expect(isCommentary(byDisplay)).toBe(true);
});

test("picks_audio_by_language_order", () => {
  const p = part([selected(stream(1, AUDIO, "eng", "")), stream(2, AUDIO, "jpn", ""), stream(3, AUDIO, "fra", "")]);
  let r = rules((r) => (r.audioLanguages = langs(["ja", "en"])));
  expect(chooseStreams(r, p).audio).toBe(2);
  r = rules((r) => (r.audioLanguages = langs(["de", "French"])));
  expect(chooseStreams(r, p).audio).toBe(3);
  // None of the languages is there: Plex's choice stands.
  r = rules((r) => (r.audioLanguages = langs(["ko"])));
  expect(chooseStreams(r, p)).toEqual(noChoice());
});

test("avoids_commentary_but_not_when_it_is_all_there_is", () => {
  const p = part([selected(stream(1, AUDIO, "eng", "Commentary")), stream(2, AUDIO, "eng", "")]);
  let r = rules((r) => {
    r.audioLanguages = langs(["eng"]);
    r.avoidCommentary = true;
  });
  expect(chooseStreams(r, p).audio).toBe(2);
  // Without the rule the selected track stays.
  r = rules((r) => (r.audioLanguages = langs(["eng"])));
  expect(chooseStreams(r, p).audio).toBe(1);
  // Only commentary exists in the language: use it.
  const only = part([stream(1, AUDIO, "eng", "Commentary")]);
  r = rules((r) => {
    r.audioLanguages = langs(["eng"]);
    r.avoidCommentary = true;
  });
  expect(chooseStreams(r, only).audio).toBe(1);
  // No language rule: still step off commentary.
  r = rules((r) => (r.avoidCommentary = true));
  expect(chooseStreams(r, p).audio).toBe(2);
});

test("keeps_the_selected_track_among_equals", () => {
  const p = part([stream(1, AUDIO, "eng", "AAC"), selected(stream(2, AUDIO, "eng", "AC3"))]);
  const r = rules((r) => (r.audioLanguages = langs(["eng"])));
  expect(chooseStreams(r, p).audio).toBe(2);
});

test("subtitle_modes", () => {
  const p = part([
    selected(stream(1, AUDIO, "jpn", "")),
    stream(2, SUBTITLE, "eng", ""),
    forced(stream(3, SUBTITLE, "eng", "Forced")),
    selected(stream(4, SUBTITLE, "fra", "")),
  ]);
  const set = (mode: SubtitleMode, forcedMode: ForcedMode | null) =>
    rules((r) => {
      r.subtitleLanguages = langs(["eng"]);
      r.subtitles = mode;
      r.forced = forcedMode;
    });
  expect(chooseStreams(set("keep", null), p).subtitle).toBeNull();
  expect(chooseStreams(set("off", null), p).subtitle).toEqual({ kind: "off" });
  expect(chooseStreams(set("always", null), p).subtitle).toEqual({ kind: "track", id: 2 });
  expect(chooseStreams(set("always", "prefer"), p).subtitle).toEqual({ kind: "track", id: 3 });
  expect(chooseStreams(set("always", "only"), p).subtitle).toEqual({ kind: "track", id: 3 });
  expect(chooseStreams(set("always", "avoid"), p).subtitle).toEqual({ kind: "track", id: 2 });
  // A language with nothing in it leaves the selection alone.
  const none = rules((r) => {
    r.subtitleLanguages = langs(["ko"]);
    r.subtitles = "always";
  });
  expect(chooseStreams(none, p).subtitle).toBeNull();
});

test("forced_subtitles_fall_back_to_full_ones_only_in_auto", () => {
  const p = part([stream(1, AUDIO, "eng", ""), forced(stream(2, SUBTITLE, "eng", ""))]);
  const make = (mode: ForcedMode) =>
    rules((r) => {
      r.subtitleLanguages = langs(["eng"]);
      r.subtitles = "always";
      r.forced = mode;
    });
  expect(chooseStreams(make("auto"), p).subtitle).toEqual({ kind: "track", id: 2 });
  expect(chooseStreams(make("avoid"), p).subtitle).toBeNull();
  // A track titled "Forced" counts even without the flag.
  const titled = part([stream(1, SUBTITLE, "eng", "English Forced"), stream(2, SUBTITLE, "eng", "English")]);
  expect(chooseStreams(make("only"), titled).subtitle).toEqual({ kind: "track", id: 1 });
  expect(chooseStreams(make("avoid"), titled).subtitle).toEqual({ kind: "track", id: 2 });
});

test("foreign_mode_follows_the_audio_language", () => {
  const streams = (audio: string) => [
    selected(stream(1, AUDIO, audio, "")),
    stream(2, SUBTITLE, "eng", ""),
    forced(stream(3, SUBTITLE, "eng", "")),
  ];
  const r = rules((r) => {
    r.audioLanguages = langs(["eng"]);
    r.subtitles = "foreign";
  });
  // English audio: only the forced track (foreign dialogue).
  expect(chooseStreams(r, part(streams("eng"))).subtitle).toEqual({ kind: "track", id: 3 });
  // Japanese audio: full English subtitles.
  expect(chooseStreams(r, part(streams("jpn"))).subtitle).toEqual({ kind: "track", id: 2 });
  // English audio and no forced track: subtitles off.
  const bare = part([selected(stream(1, AUDIO, "eng", "")), selected(stream(2, SUBTITLE, "eng", ""))]);
  expect(chooseStreams(r, bare).subtitle).toEqual({ kind: "off" });
  // The audio the rules pick is the one that counts: Japanese is chosen and is "native" for
  // this profile, so only its forced track is shown (English audio would have meant full subtitles).
  const both = part([
    selected(stream(1, AUDIO, "eng", "")),
    stream(4, AUDIO, "jpn", ""),
    stream(2, SUBTITLE, "eng", ""),
    forced(stream(3, SUBTITLE, "eng", "")),
  ]);
  const jp = rules((r) => {
    r.audioLanguages = langs(["jpn"]);
    r.subtitleLanguages = langs(["eng"]);
    r.subtitles = "foreign";
  });
  const c = chooseStreams(jp, both);
  expect([c.audio, c.subtitle]).toEqual([4, { kind: "track", id: 3 }]);
});

test("applying_a_choice_changes_only_what_differs", () => {
  const p = part([
    selected(stream(1, AUDIO, "eng", "")),
    stream(2, AUDIO, "jpn", ""),
    selected(stream(3, SUBTITLE, "eng", "")),
    stream(4, SUBTITLE, "fra", ""),
  ]);
  const changed = applyChoice(p, { audio: 2, subtitle: { kind: "off" } });
  expect(changed).toEqual([2, 0]);
  expect(PartNs.selectedAudio(p)?.id).toBe(2);
  expect(PartNs.selectedSubtitle(p)).toBeNull();
  // Already as asked: nothing to save.
  expect(applyChoice(p, { audio: 2, subtitle: { kind: "off" } })).toEqual([null, null]);
  expect(applyChoice(p, { audio: null, subtitle: { kind: "track", id: 4 } } satisfies Choice)).toEqual([null, 4]);
  expect(PartNs.selectedSubtitle(p)?.id).toBe(4);
});

function version(res: string, codec: string, hdr: boolean): Media {
  const video = defaultStream({ streamType: VIDEO });
  if (hdr) video.colorTrc = "smpte2084";
  return defaultMedia({ videoResolution: res, videoCodec: codec, Part: [defaultPart({ Stream: [video] })] });
}

test("ranks_versions", () => {
  const versions = [version("1080", "h264", false), version("4k", "hevc", true), version("720", "h264", false)];
  const pick = (f: (r: Rules) => void) => {
    const r = new Rules();
    f(r);
    return pickVersion(r.resolve(), versions);
  };
  expect(pick(() => {})).toBe(0); // no preference keeps Plex's order
  expect(pick((r) => (r.resolution = "4k"))).toBe(1);
  expect(pick((r) => (r.resolution = "1080"))).toBe(0);
  expect(pick((r) => (r.resolution = "720"))).toBe(2);
  expect(pick((r) => (r.codec = "H.265"))).toBe(1);
  expect(pick((r) => (r.hdr = "prefer" as HdrMode))).toBe(1);
  expect(pick((r) => (r.hdr = "avoid" as HdrMode))).toBe(0);
  // 4K wanted, but HDR avoided: resolution weighs more.
  expect(
    pick((r) => {
      r.resolution = "4k";
      r.hdr = "avoid";
    }),
  ).toBe(1);
  // 1080p equidistant from 4K and 720p: the higher one wins.
  const two = [version("720", "h264", false), version("4k", "hevc", false)];
  const fhd = new Rules({ resolution: "1080" }).resolve();
  expect(pickVersion(fhd, two)).toBe(1);
  // A single version is never "changed".
  expect(pickVersion(fhd, two.slice(0, 1))).toBe(0);
});

test("hdr_is_also_recognised_by_version_name", () => {
  const m = version("4k", "hevc", false);
  m.title = "2160p DV";
  expect(mediaIsHdr(m)).toBe(true);
  m.title = "Director's Cut";
  expect(mediaIsHdr(m)).toBe(false);
});

test("show_rules_override_the_profile_field_by_field", () => {
  const scope = new Scope("alice", "home");
  const entries: Entry[] = [];
  store(entries, scope, null, null, new Rules({ audioLanguages: ["eng"], subtitles: "off", avoidCommentary: true }));
  store(entries, scope, "42", "Anime", new Rules({ audioLanguages: ["jpn"], subtitles: "always" }));
  const anime = effective(entries, scope, "42");
  expect(anime.audioLanguages).toEqual(["jpn"]);
  expect(anime.subtitles).toBe("always");
  expect(anime.avoidCommentary).toBe(true); // inherited from the profile
  const other = effective(entries, scope, "7");
  expect(other.audioLanguages).toEqual(["eng"]);
  expect(effective(entries, scope, null).subtitles).toBe("off");
  // Empty rules clear the show again.
  store(entries, scope, "42", null, new Rules());
  expect(effective(entries, scope, "42").audioLanguages).toEqual(["eng"]);
});

test("rules_never_cross_accounts_or_servers", () => {
  const alice = new Scope("alice", "home");
  const bob = new Scope("bob", "home");
  const away = new Scope("alice", "away");
  const entries: Entry[] = [];
  store(entries, alice, null, null, new Rules({ audioLanguages: ["jpn"] }));
  store(entries, alice, "1", "Show", new Rules({ subtitles: "off" }));
  for (const other of [bob, away]) {
    expect(effective(entries, other, "1").isEmpty()).toBe(true);
    expect(inScope(entries, other).length).toBe(0);
  }
  // Saving for another scope leaves the first one untouched.
  store(entries, bob, null, null, new Rules({ audioLanguages: ["eng"] }));
  expect(effective(entries, alice, null).audioLanguages).toEqual(["jpn"]);
  expect(effective(entries, bob, null).audioLanguages).toEqual(["eng"]);
});

test("remembers_the_tracks_chosen_in_the_dialog", () => {
  const audio = stream(1, AUDIO, "jpn", "");
  const sub = stream(2, SUBTITLE, "eng", "");
  const r = rulesFromTracks(audio, sub);
  expect(r.audioLanguages).toEqual(["jpn"]);
  expect(r.subtitleLanguages).toEqual(["eng"]);
  expect([r.subtitles, r.forced]).toEqual(["always", "avoid"]);
  expect(rulesFromTracks(null, null).subtitles).toBe("off");
  expect(rulesFromTracks(null, undefined).isEmpty()).toBe(true);
  const forcedSub = forced(stream(3, SUBTITLE, "eng", ""));
  expect(rulesFromTracks(null, forcedSub).forced).toBe("only");
});

test("rules_round_trip_through_json", () => {
  const r = new Rules({ resolution: "4k", hdr: "avoid", subtitles: "foreign" });
  const json = JSON.parse(JSON.stringify(r)) as RulesFields;
  expect(json.resolution).toBe("4k");
  expect(json.hdr).toBe("avoid");
  expect(json.subtitles).toBe("foreign");
  expect(Rules.from(json)).toEqual(r);
  expect(Rules.from({})).toEqual(new Rules());
});
