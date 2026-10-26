// Port of crates/plex/tests/tracks.rs.

import { expect, test } from "bun:test";

import { Part, Stream, defaultPart, defaultStream, streamType } from "./index";

function audio(id: number, lang: string, codec: string, channels: number, title: string | null): Stream {
  return defaultStream({
    id,
    streamType: streamType.AUDIO,
    languageCode: lang,
    codec,
    channels,
    title,
    index: id,
  });
}

function sub(id: number, lang: string, forced: boolean, sdh: boolean, title: string | null): Stream {
  return defaultStream({
    id,
    streamType: streamType.SUBTITLE,
    languageCode: lang,
    codec: "srt",
    forced,
    hearingImpaired: sdh,
    title,
    index: id,
  });
}

function part(streams: Stream[]): Part {
  return defaultPart({ id: 1, Stream: streams });
}

function matched(p: Part, template: Stream): number | null {
  return Part.matchingStream(p, template)?.id ?? null;
}

test("audio_requires_language_and_prefers_closest", () => {
  const template = audio(0, "jpn", "flac", 2, null);
  const p = part([audio(1, "eng", "flac", 2, null), audio(2, "jpn", "aac", 6, null), audio(3, "jpn", "flac", 2, null)]);
  expect(matched(p, template)).toBe(3);

  const englishOnly = part([audio(1, "eng", "flac", 2, null)]);
  expect(matched(englishOnly, template)).toBeNull();
});

test("title_outweighs_codec", () => {
  const template = audio(0, "eng", "ac3", 6, "Commentary");
  const p = part([audio(1, "eng", "ac3", 6, null), audio(2, "eng", "aac", 2, "Commentary")]);
  expect(matched(p, template)).toBe(2);
});

test("subtitles_keep_forced_apart", () => {
  const full = sub(0, "eng", false, false, null);
  const forced = sub(0, "eng", true, false, "Signs");
  const p = part([sub(1, "eng", true, false, "Signs"), sub(2, "eng", false, true, null), sub(3, "eng", false, false, null)]);
  expect(matched(p, full)).toBe(3);
  expect(matched(p, forced)).toBe(1);

  // Only forced subs available: a full-subtitle choice finds nothing.
  const onlyForced = part([sub(1, "eng", true, false, null)]);
  expect(matched(onlyForced, full)).toBeNull();
});

test("ties_go_to_the_first_stream", () => {
  const template = audio(0, "eng", "aac", 2, null);
  const p = part([audio(5, "eng", "aac", 2, null), audio(6, "eng", "aac", 2, null)]);
  expect(matched(p, template)).toBe(5);
});

test("stream labels", () => {
  expect(Stream.label(defaultStream({ index: 3 }))).toBe("Track 3");
  expect(Stream.label(defaultStream())).toBe("Track 0");
  expect(Stream.shortLabel(defaultStream({ displayTitle: "English (AC3 5.1)", title: "x" }))).toBe("English (AC3 5.1)");
});
