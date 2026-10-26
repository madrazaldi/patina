// Set audio and subtitle tracks for a whole show or season at once (like
// PASTA): pick tracks in one episode and the closest match is selected in
// every other episode.
//
// The front end loads a show once (`load`), then asks for a fresh `View`
// whenever a choice changes, and finally `apply`s the planned changes.
// Port of crates/core/src/tracks.rs.

import { Metadata, Part, Stream } from "../plex";
import { eachCompleted, errorText } from "./ops/util";
import type { AppState } from "./state";

export type SubChoice = { kind: "keep" } | { kind: "off" } | { kind: "track"; id: number };

export function parseSubChoice(v: string): SubChoice {
  if (v === "off") return { kind: "off" };
  // Rust `str::parse::<i64>`: an optional sign and digits only.
  if (/^[+-]?\d+$/.test(v)) {
    const id = Number(v);
    if (Number.isSafeInteger(id)) return { kind: "track", id };
  }
  return { kind: "keep" };
}

export interface PartChange {
  partId: number;
  audio: number | null;
  subtitle: number | null;
}

/** What applying a choice would do across the episodes in scope. */
export interface Plan {
  changes: PartChange[];
  /** Episodes (with changes) that would be touched. */
  episodes: number;
  noAudio: string[];
  noSubtitle: string[];
}

function emptyPlan(): Plan {
  return { changes: [], episodes: 0, noAudio: [], noSubtitle: [] };
}

function episodeLabel(ep: Metadata): string {
  return `${Metadata.episodeCode(ep)} · ${Metadata.title(ep)}`;
}

/**
 * The subtitle choice of a query: `undefined` leaves subtitles alone ("keep"),
 * `null` turns them off, a stream is the template to match.
 */
export type SubTemplate = Stream | null | undefined;

function planPart(part: Part, audio: Stream | null, subtitle: SubTemplate): [number | null, number | null, boolean, boolean] {
  let a: number | null = null;
  let s: number | null = null;
  let missA = false;
  let missS = false;
  if (audio) {
    const m = Part.matchingStream(part, audio);
    if (m && m.selected) {
      // already selected
    } else if (m) a = m.id;
    else missA = true;
  }
  if (subtitle === undefined) {
    // keep
  } else if (subtitle === null) {
    // Turn subtitles off.
    if (Part.selectedSubtitle(part)) s = 0;
  } else {
    const m = Part.matchingStream(part, subtitle);
    if (m && m.selected) {
      // already selected
    } else if (m) s = m.id;
    else missS = true;
  }
  return [a, s, missA, missS];
}

export function plan(episodes: readonly Metadata[], audio: Stream | null, subtitle: SubTemplate): Plan {
  const out = emptyPlan();
  for (const ep of episodes) {
    let touched = false;
    let missA = false;
    let missS = false;
    for (const part of ep.Media.flatMap((m) => m.Part)) {
      if (part.id === null) continue;
      const [a, s, ma, ms] = planPart(part, audio, subtitle);
      missA ||= ma;
      missS ||= ms;
      if (a !== null || s !== null) {
        touched = true;
        out.changes.push({ partId: part.id, audio: a, subtitle: s });
      }
    }
    if (touched) out.episodes += 1;
    if (missA) out.noAudio.push(episodeLabel(ep));
    if (missS) out.noSubtitle.push(episodeLabel(ep));
  }
  return out;
}

/** Load every episode of a show (with streams) for the dialog. */
export async function load(app: AppState, show: string): Promise<void> {
  const episodes = await app.server().allLeavesWithStreams(show);
  app.tracks = [show, episodes];
}

/** The dialog's current choices. */
export interface Query {
  show: string;
  /** Season index; `null` means every season. */
  season?: number | null;
  /** Episode whose tracks are offered; defaults to the first in scope. */
  reference?: string | null;
  audio?: number | null;
  /** "keep", "off" or a stream id. */
  subtitle: string;
}

export interface Season {
  index: number;
  title: string;
  count: number;
}

export interface Episode {
  ratingKey: string;
  label: string;
}

export interface Track {
  id: number;
  label: string;
  external: boolean;
}

function trackOf(s: Stream): Track {
  return { id: s.id ?? 0, label: Stream.label(s), external: Stream.isExternal(s) };
}

/** Everything the dialog renders for a query. */
export interface View {
  total: number;
  seasons: Season[];
  scope: Episode[];
  reference: string | null;
  audio: Track[];
  subtitles: Track[];
  audioName: string | null;
  subtitleName: string | null;
  nothingChosen: boolean;
  plan: Plan;
}

/**
 * The episodes in scope for a query, the reference episode, its tracks and
 * the templates the choices name.
 */
interface Chosen {
  scope: Metadata[];
  reference: Metadata | null;
  audio: Stream[];
  subs: Stream[];
  audioTpl: Stream | null;
  subTpl: SubTemplate;
}

function choose(all: readonly Metadata[], q: Query): Chosen {
  const season = q.season ?? null;
  const scope = all.filter((ep) => season === null || ep.parentIndex === season);
  const reference = (q.reference != null ? scope.find((ep) => Metadata.ratingKey(ep) === q.reference) : undefined) ?? scope[0] ?? null;
  const refPart = reference?.Media[0]?.Part[0];
  const audio = refPart ? Part.audioStreams(refPart) : [];
  const subs = refPart ? Part.subtitleStreams(refPart) : [];
  const audioTpl = q.audio != null ? (audio.find((s) => s.id === q.audio) ?? null) : null;
  const choice = parseSubChoice(q.subtitle);
  let subTpl: SubTemplate;
  switch (choice.kind) {
    case "keep":
      subTpl = undefined;
      break;
    case "off":
      subTpl = null;
      break;
    case "track":
      // A track id that is not offered leaves the choice unset, like `.find(..).map(..)` yielding None.
      subTpl = subs.find((s) => s.id === choice.id);
      break;
  }
  return { scope, reference, audio, subs, audioTpl, subTpl };
}

function loaded(app: AppState, show: string): Metadata[] {
  if (app.tracks && app.tracks[0] === show) return app.tracks[1];
  throw new Error("The episode list is not loaded.");
}

/**
 * The streams a query names (copies), and the show's title; what the dialog's
 * "remember for this show" turns into playback rules. The subtitle is
 * `undefined` for "keep", `null` for "off".
 */
export function chosen(app: AppState, q: Query): { audio: Stream | null; subtitle: SubTemplate; title: string } {
  const all = loaded(app, q.show);
  const c = choose(all, q);
  const title = all[0]?.grandparentTitle ?? "";
  return {
    audio: c.audioTpl ? structuredClone(c.audioTpl) : null,
    subtitle: c.subTpl ? structuredClone(c.subTpl) : c.subTpl,
    title,
  };
}

export function view(app: AppState, q: Query): View {
  const all = loaded(app, q.show);

  // Seasons in library order, with episode counts.
  const seasons: Season[] = [];
  for (const ep of all) {
    const idx = ep.parentIndex ?? 0;
    const s = seasons.find((s) => s.index === idx);
    if (s) s.count += 1;
    else seasons.push({ index: idx, title: ep.parentTitle ?? `Season ${idx}`, count: 1 });
  }
  const { scope, reference, audio, subs, audioTpl, subTpl } = choose(all, q);
  const p = plan(scope, audioTpl, subTpl);

  return {
    total: all.length,
    seasons,
    scope: scope.map((ep) => ({ ratingKey: Metadata.ratingKey(ep), label: episodeLabel(ep) })),
    reference: reference ? Metadata.ratingKey(reference) : null,
    audio: audio.map(trackOf),
    subtitles: subs.map(trackOf),
    audioName: audioTpl ? Stream.shortLabel(audioTpl) : null,
    subtitleName: subTpl ? Stream.shortLabel(subTpl) : null,
    nothingChosen: audioTpl === null && subTpl === undefined,
    plan: p,
  };
}

/** Save the changes to Plex a few at a time, reporting `(done, total)` as they land. */
export async function apply(app: AppState, changes: PartChange[], progress: (done: number, total: number) => void): Promise<void> {
  const server = app.server();
  const total = changes.length;
  progress(0, total);
  let done = 0;
  let failed = 0;
  let firstError: string | null = null;
  await eachCompleted(
    changes,
    6,
    async (c): Promise<Error | null> => {
      try {
        await server.selectStreams(c.partId, c.audio, c.subtitle, false);
        return null;
      } catch (e) {
        return e instanceof Error ? e : new Error(errorText(e));
      }
    },
    (r) => {
      done += 1;
      if (r) {
        failed += 1;
        firstError ??= r.message;
      }
      progress(done, total);
    },
  );
  app.tracks = null;
  app.bump();
  if (firstError === null) {
    app.toast(`Updated tracks for ${total} file${total === 1 ? "" : "s"}`);
    return;
  }
  throw new Error(`${failed} of ${total} updates failed: ${firstError}`);
}
