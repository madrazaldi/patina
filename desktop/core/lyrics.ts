// Track lyrics from Plex lyric streams (stream type 4): LRC files become
// timed lines, plain text stays untimed. Port of crates/core/src/lyrics.rs.

import { Params, streamType } from "../plex";
import type { PlexServer } from "../plex";

export interface Line {
  timeMs: number | null;
  text: string;
}

export interface Lyrics {
  synced: boolean;
  lines: Line[];
}

export async function load(server: PlexServer, ratingKey: string): Promise<Lyrics | null> {
  const item = await server.metadata(ratingKey, {});
  if (!item) return null;
  let key: string | undefined;
  for (const m of item.Media) {
    for (const p of m.Part) {
      for (const s of p.Stream) {
        if (s.streamType === streamType.LYRICS && s.key !== null && key === undefined) key = s.key;
      }
    }
  }
  if (key === undefined) return null;
  const [bytes] = await server.bytes(key, new Params());
  const lyrics = parse(new TextDecoder("utf-8").decode(bytes));
  return lyrics.lines.length > 0 ? lyrics : null;
}

/** Rust's `str::lines`: split on `\n`, drop a trailing `\r`, no empty final line. */
function lines(text: string): string[] {
  const out = text.split("\n").map((l) => (l.endsWith("\r") ? l.slice(0, -1) : l));
  if (out.length > 0 && out[out.length - 1] === "") out.pop();
  return out;
}

/** Parse LRC (`[mm:ss.xx] text`, several stamps per line allowed) or plain text. */
export function parse(text: string): Lyrics {
  const timed: Line[] = [];
  const plain: Line[] = [];
  for (const raw of lines(text)) {
    let rest = raw.trim();
    const stamps: number[] = [];
    while (rest.startsWith("[")) {
      const body = rest.slice(1);
      const end = body.indexOf("]");
      if (end < 0) break;
      const tag = body.slice(0, end);
      rest = body.slice(end + 1).trimStart();
      const ms = stamp(tag);
      if (ms !== null) stamps.push(ms);
      // Other tags ([ar:], [ti:], [offset:]) are metadata; drop them.
    }
    if (stamps.length === 0) {
      if (!raw.trimStart().startsWith("[")) plain.push({ timeMs: null, text: raw.trim() });
    } else {
      for (const ms of stamps) timed.push({ timeMs: ms, text: rest });
    }
  }
  if (timed.length === 0) {
    while (plain.length > 0 && plain[plain.length - 1]!.text === "") plain.pop();
    return { synced: false, lines: plain };
  }
  // Stable sort by time, like `sort_by_key`.
  timed.sort((a, b) => a.timeMs! - b.timeMs!);
  return { synced: true, lines: timed };
}

function stamp(tag: string): number | null {
  const i = tag.indexOf(":");
  if (i < 0) return null;
  const m = tag.slice(0, i).trim();
  const s = tag.slice(i + 1).trim().replace(",", ".");
  // Rust's integer / f64 parsing is strict: no stray characters.
  if (!/^[+-]?\d+$/.test(m) || !/^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/.test(s)) return null;
  return Number(m) * 60_000 + Math.round(Number(s) * 1000);
}
