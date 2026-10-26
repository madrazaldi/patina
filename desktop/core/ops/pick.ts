// "Pick something to watch": a few random candidates that fit what you feel like.
//
// The server does the heavy filtering through library query parameters
// (unwatched, genre, content rating, year, rating, length, random order), so even
// a large library only sends one page per section. Everything is checked again
// here (`matches`), which keeps the result right when a server ignores a filter.

import { type Directory, type Metadata, Params, type PlexServer } from "../../plex";
import { logger } from "../log";
import type { AppState } from "../state";
import { errorText } from "./util";

const log = logger("pick");

/** Candidates fetched per library; the random order makes each fetch a fresh sample. */
const PER_SECTION = 120;
/** Used when the server refuses the numeric filters and everything is checked here. */
const FALLBACK_PER_SECTION = 400;

export type PickKind = "movie" | "show" | "any";

/** The media type a section of `sectionKind` is searched as for this kind of pick. */
function mediaTypeOf(kind: PickKind, sectionKind: string): "movie" | "show" | null {
  if ((kind === "movie" || kind === "any") && sectionKind === "movie") return "movie";
  if ((kind === "show" || kind === "any") && sectionKind === "show") return "show";
  return null;
}

/** `PickKind::parse`. */
export function parseKind(s: string): PickKind | null {
  switch (s.replace(/[A-Z]/g, (c) => c.toLowerCase())) {
    case "movie":
    case "movies":
    case "film":
    case "films":
      return "movie";
    case "show":
    case "shows":
    case "series":
    case "tv":
      return "show";
    case "any":
    case "all":
      return "any";
    default:
      return null;
  }
}

/** What to watch. The defaults are an unwatched film, four candidates. */
export interface PickQuery {
  kind: PickKind;
  /** Only things with something left to watch. */
  unwatched: boolean;
  /** Longest running time in minutes (a film, or one episode of a show). */
  maxMinutes: number | null;
  minMinutes: number | null;
  /** Any of these genres (by name). */
  genres: string[];
  /** Any of these content ratings ("PG-13", "TV-MA", ...). */
  contentRatings: string[];
  /** Library section ids; empty means every movie and show library. */
  sections: string[];
  yearMin: number | null;
  yearMax: number | null;
  /** Lowest audience rating, 0 to 10. */
  minRating: number | null;
  /** How many candidates to return. */
  count: number;
  /** Rating keys shown before, so a reroll offers something new. */
  exclude: string[];
}

export function defaultPickQuery(): PickQuery {
  return {
    kind: "movie",
    unwatched: true,
    maxMinutes: null,
    minMinutes: null,
    genres: [],
    contentRatings: [],
    sections: [],
    yearMin: null,
    yearMax: null,
    minRating: null,
    count: 4,
    exclude: [],
  };
}

/** A query as the front end sends it: every field optional (`#[serde(default)]`), unset ones null. */
export function pickQueryFrom(raw: Partial<{ [K in keyof PickQuery]: PickQuery[K] | null }> | null | undefined): PickQuery {
  const q = defaultPickQuery();
  if (!raw) return q;
  if (raw.kind != null) {
    if (raw.kind !== "movie" && raw.kind !== "show" && raw.kind !== "any") {
      throw new Error(`unknown variant \`${String(raw.kind)}\`, expected one of \`movie\`, \`show\`, \`any\``);
    }
    q.kind = raw.kind;
  }
  if (raw.unwatched != null) q.unwatched = raw.unwatched;
  if (raw.maxMinutes != null) q.maxMinutes = raw.maxMinutes;
  if (raw.minMinutes != null) q.minMinutes = raw.minMinutes;
  if (raw.genres != null) q.genres = raw.genres;
  if (raw.contentRatings != null) q.contentRatings = raw.contentRatings;
  if (raw.sections != null) q.sections = raw.sections;
  if (raw.yearMin != null) q.yearMin = raw.yearMin;
  if (raw.yearMax != null) q.yearMax = raw.yearMax;
  if (raw.minRating != null) q.minRating = raw.minRating;
  if (raw.count != null) q.count = raw.count;
  if (raw.exclude != null) q.exclude = raw.exclude;
  return q;
}

function queryCount(q: PickQuery): number {
  return Math.min(Math.max(q.count, 1), 12);
}

export interface PickResult {
  items: Metadata[];
  /**
   * How many titles fit the query in the libraries searched (a lower bound when a
   * library has more than one sample page of them).
   */
  matches: number;
  /** How many of those have not been offered yet. */
  remaining: number;
  /** Everything that fits was already offered, so the choice started over. */
  wrapped: boolean;
  /** Names of the libraries that were searched. */
  libraries: string[];
}

export interface PickSection {
  id: string;
  title: string;
  /** `movie` or `show`. */
  kind: string;
}

/** The choices the form offers. */
export interface PickOptions {
  sections: PickSection[];
  genres: string[];
  contentRatings: string[];
}

function isPickable(d: Directory): boolean {
  return d.type === "movie" || d.type === "show";
}

/** `Directory::title`. */
function dirTitle(d: Directory): string {
  return d.title ?? d.tag ?? "";
}

/** Libraries, genres and content ratings to choose from. */
export async function options(app: AppState): Promise<PickOptions> {
  const server = app.server();
  const sections: PickSection[] = [];
  for (const d of app.sections().filter(isPickable)) {
    if (d.key != null && d.type != null) sections.push({ id: d.key, title: dirTitle(d), kind: d.type });
  }
  const lists = await Promise.all(
    sections.map(async (s) => {
      const kind = s.kind === "show" ? 2 : 1;
      const [genres, ratings] = await Promise.all([
        server.sectionFilterValues(s.id, "genre", kind).catch((): Directory[] => []),
        server.sectionFilterValues(s.id, "contentRating", kind).catch((): Directory[] => []),
      ]);
      return [genres, ratings] as const;
    }),
  );
  const genres: string[] = [];
  const ratings: string[] = [];
  for (const [g, r] of lists) {
    for (const d of g) pushUnique(genres, d.title ?? d.tag);
    for (const d of r) pushUnique(ratings, d.title ?? d.tag ?? d.key);
  }
  genres.sort((a, b) => cmpStr(a.toLowerCase(), b.toLowerCase()));
  ratings.sort((a, b) => {
    const [ra, sa] = ratingOrder(a);
    const [rb, sb] = ratingOrder(b);
    return ra - rb || cmpStr(sa, sb);
  });
  return { sections, genres, contentRatings: ratings };
}

function cmpStr(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

function asciiLower(s: string): string {
  return s.replace(/[A-Z]/g, (c) => c.toLowerCase());
}

function asciiUpper(s: string): string {
  return s.replace(/[a-z]/g, (c) => c.toUpperCase());
}

function pushUnique(list: string[], value: string | null | undefined): void {
  if (value != null && value !== "" && !list.some((x) => asciiLower(x) === asciiLower(value))) list.push(value);
}

/** Orders ratings roughly from gentlest to strictest (G, PG, PG-13, R, TV-*, others). */
export function ratingOrder(r: string): [number, string] {
  let rank: number;
  switch (asciiUpper(r)) {
    case "G":
    case "TV-Y":
    case "TV-G":
      rank = 0;
      break;
    case "PG":
    case "TV-PG":
    case "TV-Y7":
      rank = 1;
      break;
    case "PG-13":
    case "TV-14":
      rank = 2;
      break;
    case "R":
    case "TV-MA":
      rank = 3;
      break;
    case "NC-17":
      rank = 4;
      break;
    default:
      rank = 5;
  }
  return [rank, r.toLowerCase()];
}

/** Request parameters narrowing a section's listing on the server. */
export function serverFilters(q: PickQuery, genreIds: string[], numeric: boolean): Params {
  const p = new Params();
  if (genreIds.length > 0) p.push("genre", genreIds.join(","));
  if (q.contentRatings.length > 0) p.push("contentRating", q.contentRatings.join(","));
  if (numeric) {
    // `key>>=value` means at least, `key<<=value` at most.
    if (q.yearMin != null) p.push("year>>", q.yearMin);
    if (q.yearMax != null) p.push("year<<", q.yearMax);
    if (q.minRating != null) p.push("audienceRating>>", q.minRating);
    if (q.maxMinutes != null) p.push("duration<<", q.maxMinutes * 60_000);
    if (q.minMinutes != null && q.minMinutes > 0) p.push("duration>>", q.minMinutes * 60_000);
  }
  return p;
}

/** The id Plex filters a genre by, from a `genre` filter value entry. */
export function filterId(d: Directory): string | null {
  const key = d.key;
  if (key == null) return null;
  const at = key.lastIndexOf("genre=");
  return at >= 0 ? key.slice(at + "genre=".length) : key;
}

/** Genre filter ids for the genres named in the query, if this section has any of them. */
async function genreIds(server: PlexServer, section: string, kind: number, wanted: string[]): Promise<string[]> {
  const values = await server.sectionFilterValues(section, "genre", kind).catch((): Directory[] => []);
  const out: string[] = [];
  for (const d of values) {
    const name = d.title ?? d.tag ?? "";
    if (!wanted.some((w) => asciiLower(w) === asciiLower(name))) continue;
    const id = filterId(d);
    if (id !== null) out.push(id);
  }
  return out;
}

/** One library's candidates and how many titles fit in all. */
async function candidates(server: PlexServer, section: Directory, q: PickQuery): Promise<[Metadata[], number]> {
  const id = section.key;
  if (id == null) throw new Error("library without an id");
  const type = mediaTypeOf(q.kind, section.type ?? "");
  if (type === null) return [[], 0];
  const kind = type === "show" ? 2 : 1;
  const ids = q.genres.length === 0 ? [] : await genreIds(server, id, kind, q.genres);
  if (q.genres.length > 0 && ids.length === 0) {
    // No genre of this library is among the wanted ones.
    return [[], 0];
  }
  const fetch = (numeric: boolean, size: number) =>
    server.sectionItems(id, {
      kind,
      sort: "random",
      unwatched: q.unwatched,
      filters: serverFilters(q, ids, numeric),
      start: 0,
      size,
    });
  let mc;
  try {
    mc = await fetch(true, PER_SECTION);
  } catch (e) {
    // A server that rejects the numeric filters still gets a (larger) unfiltered sample.
    log.debug(`pick filters refused by library ${id}: ${errorText(e)}`);
    mc = await fetch(false, FALLBACK_PER_SECTION);
  }
  const total = mc.totalSize != null ? Math.max(mc.totalSize, 0) : mc.Metadata.length;
  return [mc.Metadata, total];
}

function hasUnwatched(item: Metadata): boolean {
  if (item.type === "show") return (item.leafCount ?? 1) > (item.viewedLeafCount ?? 0);
  return (item.viewCount ?? 0) === 0;
}

/** Whether an item fits the query. The same checks the server was asked to make. */
export function matches(item: Metadata, q: PickQuery): boolean {
  const kind = item.type ?? "";
  const kindOk =
    (q.kind === "movie" && kind === "movie") ||
    (q.kind === "show" && kind === "show") ||
    (q.kind === "any" && (kind === "movie" || kind === "show"));
  if (!kindOk) return false;
  if (q.unwatched && !hasUnwatched(item)) return false;
  if (q.maxMinutes != null || q.minMinutes != null) {
    if (item.duration == null || item.duration <= 0) return false;
    const minutes = item.duration / 60_000;
    if (q.maxMinutes != null && minutes > q.maxMinutes) return false;
    if (q.minMinutes != null && minutes < q.minMinutes) return false;
  }
  if (q.genres.length > 0 && !item.Genre.some((g) => q.genres.some((w) => asciiLower(w) === asciiLower(g.tag ?? "")))) {
    return false;
  }
  if (q.contentRatings.length > 0) {
    const rating = item.contentRating ?? "";
    if (!q.contentRatings.some((w) => asciiLower(w) === asciiLower(rating))) return false;
  }
  if (q.yearMin != null || q.yearMax != null) {
    if (item.year == null) return false;
    if (q.yearMin != null && item.year < q.yearMin) return false;
    if (q.yearMax != null && item.year > q.yearMax) return false;
  }
  if (q.minRating != null) {
    const r = item.audienceRating ?? item.rating;
    if (r == null || r < q.minRating) return false;
  }
  return true;
}

/** A small seeded generator (the `fastrand::Rng` of the Rust code); unseeded ones start from the clock. */
export class Rng {
  private state: number;

  constructor(seed?: number) {
    this.state = (seed ?? Math.floor(Math.random() * 0x1_0000_0000)) >>> 0;
  }

  /** mulberry32: a float in `0..1`. */
  next(): number {
    this.state = (this.state + 0x6d2b79f5) >>> 0;
    let t = this.state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  }

  /** Uniform in `0..n`. */
  below(n: number): number {
    return Math.floor(this.next() * n);
  }

  /** Fisher-Yates, in place. */
  shuffle<T>(list: T[]): void {
    for (let i = list.length - 1; i > 0; i--) {
      const j = this.below(i + 1);
      [list[i], list[j]] = [list[j]!, list[i]!];
    }
  }
}

/**
 * Choose from the candidates that fit: not offered before, in random order. When
 * everything that fits was offered already, the choice starts over (`wrapped`).
 * Returns the chosen items, how many fit, and how many of those are still unseen.
 */
export function choose(fits: Metadata[], q: PickQuery, rng: Rng): [Metadata[], number, number, boolean] {
  const seen = new Set<string>();
  fits = fits.filter((m) => {
    const key = m.ratingKey ?? "";
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
  const matchCount = fits.length;
  const excluded = new Set(q.exclude);
  const fresh: Metadata[] = [];
  const stale: Metadata[] = [];
  for (const m of fits) (excluded.has(m.ratingKey ?? "") ? stale : fresh).push(m);
  const wrapped = fresh.length === 0 && stale.length > 0;
  const remaining = fresh.length;
  const pool = wrapped ? stale : fresh;
  rng.shuffle(pool);
  pool.length = Math.min(pool.length, queryCount(q));
  return [pool, matchCount, remaining, wrapped];
}

/** Pick a few things to watch. */
export async function pick(app: AppState, query: PickQuery | Parameters<typeof pickQueryFrom>[0]): Promise<PickResult> {
  const q = pickQueryFrom(query);
  const server = app.server();
  const sections = app
    .sections()
    .filter(isPickable)
    .filter((d) => q.sections.length === 0 || (d.key != null && q.sections.includes(d.key)))
    .filter((d) => mediaTypeOf(q.kind, d.type ?? "") !== null);
  if (sections.length === 0) throw new Error("There is no movie or show library to pick from.");
  const fetched = await Promise.all(
    sections.map((s) =>
      candidates(server, s, q).then(
        (value) => ({ ok: true as const, value }),
        (error: unknown) => ({ ok: false as const, error }),
      ),
    ),
  );
  let fits: Metadata[] = [];
  let total = 0;
  const libraries: string[] = [];
  let failure: unknown = null;
  sections.forEach((section, i) => {
    const result = fetched[i]!;
    if (result.ok) {
      const [items, count] = result.value;
      libraries.push(dirTitle(section));
      // The server's count covers the whole library; the sample is checked here.
      const kept = items.filter((m) => matches(m, q));
      total += kept.length === 0 ? 0 : Math.max(count, kept.length);
      fits = fits.concat(kept);
    } else {
      failure = result.error;
    }
  });
  if (libraries.length === 0) throw failure ?? new Error("Nothing could be searched.");
  const [items, fit, fresh, wrapped] = choose(fits, q, new Rng());
  // A library with more matches than one sample page reports its real count.
  const matchCount = Math.max(total, fit);
  const remaining = wrapped ? matchCount : Math.max(matchCount - (fit - fresh), 0);
  return { items, matches: matchCount, remaining, wrapped, libraries };
}
