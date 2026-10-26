import { expect, test } from "bun:test";

import { Metadata, defaultMedia, defaultMetadata, defaultPart, defaultStream, streamType, type Stream } from "../plex";
import { type PartChange, parseSubChoice, plan } from "./tracks";

function stream(id: number, kind: number, lang: string, selected: boolean): Stream {
  return defaultStream({ id, streamType: kind, languageCode: lang, selected, index: id });
}

function episode(n: number, streams: Stream[]): Metadata {
  return defaultMetadata({
    index: n,
    parentIndex: 1,
    Media: [defaultMedia({ Part: [defaultPart({ id: n, Stream: streams })] })],
  });
}

const label = (e: Metadata) => `${Metadata.episodeCode(e)} · ${Metadata.title(e)}`;

test("plans_only_what_changes", () => {
  const a = streamType.AUDIO;
  const s = streamType.SUBTITLE;
  const eps = [
    // Already on Japanese audio.
    episode(1, [stream(11, a, "eng", false), stream(12, a, "jpn", true), stream(13, s, "eng", true)]),
    // English selected; Japanese available.
    episode(2, [stream(21, a, "eng", true), stream(22, a, "jpn", false)]),
    // No Japanese at all.
    episode(3, [stream(31, a, "eng", true), stream(32, s, "eng", false)]),
  ];
  const tpl = stream(0, a, "jpn", false);

  let p = plan(eps, tpl, null);
  expect(p.changes).toEqual([
    { partId: 1, audio: null, subtitle: 0 },
    { partId: 2, audio: 22, subtitle: null },
  ] satisfies PartChange[]);
  expect(p.episodes).toBe(2);
  expect(p.noAudio).toEqual([label(eps[2]!)]);

  const subTpl = stream(0, s, "eng", false);
  p = plan(eps, null, subTpl);
  expect(p.changes).toEqual([{ partId: 3, audio: null, subtitle: 32 }]);
  expect(p.noSubtitle).toEqual([label(eps[1]!)]);
});

test("parses_subtitle_choice", () => {
  expect(parseSubChoice("keep")).toEqual({ kind: "keep" });
  expect(parseSubChoice("off")).toEqual({ kind: "off" });
  expect(parseSubChoice("42")).toEqual({ kind: "track", id: 42 });
});
