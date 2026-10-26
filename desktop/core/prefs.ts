// Automatic playback preferences: which audio and subtitle tracks and which
// version of a title to use, remembered per profile and per show.
//
// Rules are stored in `playback-prefs.json` next to the config file, scoped
// by account and server (a profile on one server never affects another).
// A show's rules override the profile's, field by field; a field a show
// leaves unset inherits the profile's value.
//
// When playback starts the rules are matched against the streams of the file
// (`chooseStreams`) and the versions of the title (`pickVersion`). Both
// are pure functions, so they apply to newly added episodes too: nothing is
// stored per episode.
// Port of crates/core/src/prefs.rs.

import { join } from "node:path";

import { Media, Metadata, Part, type PlexServer, Stream, streamType } from "../plex";
import * as config from "./config";
import { logger } from "./log";
import { errorText } from "./ops/util";
import { Scope } from "./recovery";
import type { AppState } from "./state";
import * as storage from "./storage";
import * as tracks from "./tracks";

const log = logger("prefs");

// Rules ----------------------------------------------------------------------

/**
 * When subtitles are turned on. `keep` leaves the subtitle track as Plex has
 * it; `foreign` is only for dialogue you can't follow: forced subtitles while
 * the audio is in one of your languages, full subtitles when it is not.
 */
export type SubtitleMode = "keep" | "off" | "always" | "foreign";

/**
 * Which kind of subtitle track is wanted: `auto` full subtitles (forced ones
 * when there are no others), `prefer` forced first then full, `avoid` never a
 * forced track, `only` only forced tracks.
 */
export type ForcedMode = "auto" | "prefer" | "avoid" | "only";

export type Resolution = "any" | "4k" | "1080" | "720" | "sd";

export type HdrMode = "any" | "prefer" | "avoid";

const SUBTITLE_MODES: readonly string[] = ["keep", "off", "always", "foreign"];
const FORCED_MODES: readonly string[] = ["auto", "prefer", "avoid", "only"];
const RESOLUTIONS: readonly string[] = ["any", "4k", "1080", "720", "sd"];
const HDR_MODES: readonly string[] = ["any", "prefer", "avoid"];

/** 1 (SD) to 4 (4K); `null` for "any". */
function resolutionRank(r: Resolution): number | null {
  switch (r) {
    case "any":
      return null;
    case "4k":
      return 4;
    case "1080":
      return 3;
    case "720":
      return 2;
    case "sd":
      return 1;
  }
}

export function resolutionLabel(r: Resolution): string {
  switch (r) {
    case "any":
      return "Any";
    case "4k":
      return "4K";
    case "1080":
      return "1080p";
    case "720":
      return "720p";
    case "sd":
      return "SD";
  }
}

/** The fields of `Rules`; `null` means "not set" (a show then inherits the profile's value). */
export interface RulesFields {
  /** Languages in order of preference, as ISO codes or names ("eng", "ja", "French"). */
  audioLanguages: string[] | null;
  /** Subtitle languages; the audio languages when empty. */
  subtitleLanguages: string[] | null;
  subtitles: SubtitleMode | null;
  forced: ForcedMode | null;
  /** Skip commentary and audio-description tracks. */
  avoidCommentary: boolean | null;
  resolution: Resolution | null;
  /** Preferred video codec ("hevc", "h264", "av1", ...); empty for any. */
  codec: string | null;
  hdr: HdrMode | null;
}

function stringList(v: unknown): string[] | null {
  return Array.isArray(v) ? v.filter((s): s is string => typeof s === "string") : null;
}

function oneOf<T extends string>(v: unknown, allowed: readonly string[]): T | null {
  return typeof v === "string" && allowed.includes(v) ? (v as T) : null;
}

/**
 * What a profile or show asks for. Every field is optional: `null` means
 * "not set" (a show then inherits the profile's value).
 */
export class Rules implements RulesFields {
  audioLanguages: string[] | null = null;
  subtitleLanguages: string[] | null = null;
  subtitles: SubtitleMode | null = null;
  forced: ForcedMode | null = null;
  avoidCommentary: boolean | null = null;
  resolution: Resolution | null = null;
  codec: string | null = null;
  hdr: HdrMode | null = null;

  /** `Rules { .., ..Default::default() }`. */
  constructor(fields: Partial<RulesFields> = {}) {
    Object.assign(this, fields);
  }

  /**
   * Lenient `Deserialize` (`#[serde(default)]`): missing or invalid fields are unset.
   * Accepts what the UI sends (absent fields) and what the Rust core wrote (explicit nulls).
   */
  static from(v: unknown): Rules {
    const o = (typeof v === "object" && v !== null ? v : {}) as Record<string, unknown>;
    return new Rules({
      audioLanguages: stringList(o.audioLanguages),
      subtitleLanguages: stringList(o.subtitleLanguages),
      subtitles: oneOf<SubtitleMode>(o.subtitles, SUBTITLE_MODES),
      forced: oneOf<ForcedMode>(o.forced, FORCED_MODES),
      avoidCommentary: typeof o.avoidCommentary === "boolean" ? o.avoidCommentary : null,
      resolution: oneOf<Resolution>(o.resolution, RESOLUTIONS),
      codec: typeof o.codec === "string" ? o.codec : null,
      hdr: oneOf<HdrMode>(o.hdr, HDR_MODES),
    });
  }

  isEmpty(): boolean {
    return (
      this.audioLanguages === null &&
      this.subtitleLanguages === null &&
      this.subtitles === null &&
      this.forced === null &&
      this.avoidCommentary === null &&
      this.resolution === null &&
      this.codec === null &&
      this.hdr === null
    );
  }

  clone(): Rules {
    return new Rules(structuredClone({ ...this }));
  }

  /** These rules, with `base` filling every field this leaves unset. */
  over(base: Rules): Rules {
    return new Rules({
      audioLanguages: this.audioLanguages ?? base.audioLanguages,
      subtitleLanguages: this.subtitleLanguages ?? base.subtitleLanguages,
      subtitles: this.subtitles ?? base.subtitles,
      forced: this.forced ?? base.forced,
      avoidCommentary: this.avoidCommentary ?? base.avoidCommentary,
      resolution: this.resolution ?? base.resolution,
      codec: this.codec ?? base.codec,
      hdr: this.hdr ?? base.hdr,
    }).clone();
  }

  resolve(): Resolved {
    const langs = (l: string[] | null): string[] => {
      const out: string[] = [];
      for (const key of (l ?? []).map(languageKey).filter((k) => k !== "")) {
        if (!out.includes(key)) out.push(key);
      }
      return out;
    };
    const codec = this.codec !== null ? codecKey(this.codec) : null;
    return new Resolved({
      audio: langs(this.audioLanguages),
      subtitleLangs: langs(this.subtitleLanguages),
      subtitles: this.subtitles ?? "keep",
      forced: this.forced ?? "auto",
      avoidCommentary: this.avoidCommentary ?? false,
      resolution: this.resolution ?? "any",
      codec: codec !== null && codec !== "" ? codec : null,
      hdr: this.hdr ?? "any",
    });
  }
}

/** `Rules` with defaults filled in and languages normalised, ready to match. */
export class Resolved {
  audio: string[];
  subtitleLangs: string[];
  subtitles: SubtitleMode;
  forced: ForcedMode;
  avoidCommentary: boolean;
  resolution: Resolution;
  codec: string | null;
  hdr: HdrMode;

  constructor(f: {
    audio: string[];
    subtitleLangs: string[];
    subtitles: SubtitleMode;
    forced: ForcedMode;
    avoidCommentary: boolean;
    resolution: Resolution;
    codec: string | null;
    hdr: HdrMode;
  }) {
    this.audio = f.audio;
    this.subtitleLangs = f.subtitleLangs;
    this.subtitles = f.subtitles;
    this.forced = f.forced;
    this.avoidCommentary = f.avoidCommentary;
    this.resolution = f.resolution;
    this.codec = f.codec;
    this.hdr = f.hdr;
  }

  hasTrackRules(): boolean {
    return this.audio.length > 0 || this.avoidCommentary || this.subtitles !== "keep";
  }

  hasVersionRules(): boolean {
    return this.resolution !== "any" || this.codec !== null || this.hdr !== "any";
  }

  /** Languages subtitles are wanted in. */
  subLangs(): string[] {
    return this.subtitleLangs.length === 0 ? this.audio : this.subtitleLangs;
  }

  /** Languages that need no subtitles: the audio ones, else the subtitle ones. */
  native(): string[] {
    return this.audio.length === 0 ? this.subtitleLangs : this.audio;
  }
}

// Languages --------------------------------------------------------------------

/** Names a language goes by; the first is the canonical ISO 639-2 code. */
const LANGUAGES: readonly (readonly string[])[] = [
  ["eng", "en", "english"],
  ["jpn", "ja", "japanese"],
  ["fra", "fre", "fr", "french"],
  ["deu", "ger", "de", "german"],
  ["spa", "es", "spanish"],
  ["ita", "it", "italian"],
  ["por", "pt", "portuguese"],
  ["rus", "ru", "russian"],
  ["zho", "chi", "zh", "chinese", "mandarin", "cantonese"],
  ["kor", "ko", "korean"],
  ["nld", "dut", "nl", "dutch"],
  ["swe", "sv", "swedish"],
  ["nor", "nob", "nb", "no", "norwegian"],
  ["dan", "da", "danish"],
  ["fin", "fi", "finnish"],
  ["pol", "pl", "polish"],
  ["tur", "tr", "turkish"],
  ["ara", "ar", "arabic"],
  ["heb", "he", "hebrew"],
  ["hin", "hi", "hindi"],
  ["tha", "th", "thai"],
  ["ces", "cze", "cs", "czech"],
  ["hun", "hu", "hungarian"],
  ["ell", "gre", "el", "greek"],
  ["ukr", "uk", "ukrainian"],
  ["ind", "id", "indonesian"],
  ["vie", "vi", "vietnamese"],
  ["ron", "rum", "ro", "romanian"],
  ["bul", "bg", "bulgarian"],
  ["hrv", "hr", "croatian"],
  ["cat", "ca", "catalan"],
  ["fas", "per", "fa", "persian"],
  ["msa", "may", "ms", "malay"],
  ["tam", "ta", "tamil"],
  ["lat", "la", "latin"],
];

/** Rust `to_ascii_lowercase`: only A-Z change. */
function asciiLower(s: string): string {
  return s.replace(/[A-Z]/g, (c) => c.toLowerCase());
}

/**
 * A language as a comparable key: "en", "eng", "English" and "en-US" are all "eng".
 * Unknown languages are compared by their lowercase name.
 */
export function languageKey(name: string): string {
  const lower = asciiLower(name.trim());
  const base = lower.split(/[ \-_(]/).find((s) => s !== "") ?? "";
  if (["und", "zxx", "mis", "mul", "unknown"].includes(base)) return "";
  const names = LANGUAGES.find((n) => n.includes(base));
  return names ? names[0]! : base;
}

export function streamLang(s: Stream): string | null {
  for (const l of [s.languageCode, s.languageTag, s.language]) {
    if (l === null || l === undefined) continue;
    const k = languageKey(l);
    if (k !== "") return k;
  }
  return null;
}

function titles(s: Stream): string {
  return [s.title, s.displayTitle, s.extendedDisplayTitle]
    .filter((t): t is string => t !== null && t !== undefined)
    .map((t) => t.toLowerCase())
    .join(" | ");
}

/** Commentary and audio-description tracks, spotted by their titles. */
export function isCommentary(s: Stream): boolean {
  const t = titles(s);
  return [
    "commentary",
    "commentaire",
    "comentario",
    "kommentar",
    "audio description",
    "audio-description",
    "described video",
    "descriptive",
    "isolated score",
  ].some((k) => t.includes(k));
}

/** Forced-subtitle tracks, by flag or by title. */
export function isForced(s: Stream): boolean {
  return s.forced || titles(s).includes("forced");
}

// Choosing tracks ------------------------------------------------------------

/** The subtitle outcome of a rule. */
export type SubPick = { kind: "off" } | { kind: "track"; id: number };

/** The streams rules ask for; `null` leaves Plex's selection alone. */
export interface Choice {
  audio: number | null;
  subtitle: SubPick | null;
}

/** `Choice::default()`. */
export function noChoice(): Choice {
  return { audio: null, subtitle: null };
}

/** `Iterator::min_by_key`: the first element with the smallest (lexicographic) key. */
function minByKey<T>(items: readonly T[], key: (t: T) => number[]): T | null {
  let best: T | null = null;
  let bestKey: number[] | null = null;
  for (const item of items) {
    const k = key(item);
    if (bestKey === null || compareKeys(k, bestKey) < 0) {
      best = item;
      bestKey = k;
    }
  }
  return best;
}

function compareKeys(a: number[], b: number[]): number {
  for (let i = 0; i < a.length; i++) {
    if (a[i]! !== b[i]!) return a[i]! < b[i]! ? -1 : 1;
  }
  return 0;
}

const flag = (b: boolean): number => (b ? 1 : 0);

/** The stream to keep: the selected one, else the default one, else the first. */
function best(list: readonly Stream[]): Stream | null {
  return minByKey(list, (s) => [flag(!s.selected), flag(!s.default), s.index ?? s.id ?? Infinity]);
}

function pickAudio(r: Resolved, part: Part): Stream | null {
  const all = Part.audioStreams(part);
  const plain = all.filter((s) => !(r.avoidCommentary && isCommentary(s)));
  const pool = plain.length === 0 ? all : plain;
  for (const lang of r.audio) {
    const hits = pool.filter((s) => streamLang(s) === lang);
    const s = best(hits);
    if (s) return s;
  }
  // No preferred language is there; at least step off a commentary track.
  const current = all.find((s) => s.selected);
  if (r.avoidCommentary && current && isCommentary(current) && plain.length > 0) return best(plain);
  return null;
}

/**
 * Best subtitle track of `lang` of the wanted kind, preferring the selected
 * one, then non-SDH tracks.
 */
function pickKind(subs: readonly Stream[], lang: string, forced: boolean): Stream | null {
  return minByKey(
    subs.filter((s) => streamLang(s) === lang && isForced(s) === forced),
    (s) => [flag(!s.selected), flag(s.hearingImpaired), flag(!s.default), s.index ?? s.id ?? Infinity],
  );
}

function pickFullSubtitle(r: Resolved, subs: readonly Stream[]): Stream | null {
  const plain = subs.filter((s) => !(r.avoidCommentary && isCommentary(s)));
  const pool = plain.length === 0 ? subs : plain;
  let order: boolean[];
  switch (r.forced) {
    case "auto":
      order = [false, true];
      break;
    case "prefer":
      order = [true, false];
      break;
    case "avoid":
      order = [false];
      break;
    case "only":
      order = [true];
      break;
  }
  for (const lang of r.subLangs()) {
    for (const forced of order) {
      const s = pickKind(pool, lang, forced);
      if (s) return s;
    }
  }
  return null;
}

const track = (s: Stream): SubPick => ({ kind: "track", id: s.id ?? 0 });

function pickSubtitle(r: Resolved, part: Part, audio: Stream | null): SubPick | null {
  const subs = Part.subtitleStreams(part);
  switch (r.subtitles) {
    case "keep":
      return null;
    case "off":
      return { kind: "off" };
    case "always": {
      const s = pickFullSubtitle(r, subs);
      return s ? track(s) : null;
    }
    case "foreign": {
      const audioLang = audio ? streamLang(audio) : null;
      const native = audioLang !== null && r.native().includes(audioLang);
      if (native) {
        // Only the foreign dialogue: a forced track in a subtitle language, else nothing.
        let forced: Stream | null = null;
        if (r.forced !== "avoid") {
          for (const lang of r.subLangs()) {
            forced = pickKind(subs, lang, true);
            if (forced) break;
          }
        }
        return forced ? track(forced) : { kind: "off" };
      }
      const s = pickFullSubtitle(r, subs);
      return s ? track(s) : null;
    }
  }
}

/**
 * The audio and subtitle streams `rules` ask for in `part`. The choice may
 * name streams that are already selected.
 */
export function chooseStreams(r: Resolved, part: Part): Choice {
  if (!r.hasTrackRules()) return noChoice();
  const audio = pickAudio(r, part);
  // Subtitles follow the audio that will actually play.
  const playing = audio ?? Part.selectedAudio(part);
  return { audio: audio?.id ?? null, subtitle: pickSubtitle(r, part, playing) };
}

/**
 * Select `choice` in `part`. Returns the stream ids that changed, as Plex
 * takes them (`0` for subtitles off).
 */
export function applyChoice(part: Part, choice: Choice): [number | null, number | null] {
  const changed: [number | null, number | null] = [null, null];
  if (choice.audio !== null) {
    const id = choice.audio;
    if (Part.audioStreams(part).some((s) => s.id === id && !s.selected)) {
      for (const s of part.Stream.filter((s) => s.streamType === streamType.AUDIO)) s.selected = s.id === id;
      changed[0] = id;
    }
  }
  const sub = choice.subtitle;
  if (sub !== null) {
    if (sub.kind === "off") {
      if (Part.selectedSubtitle(part)) {
        for (const s of part.Stream.filter((s) => s.streamType === streamType.SUBTITLE)) s.selected = false;
        changed[1] = 0;
      }
    } else {
      const id = sub.id;
      if (Part.subtitleStreams(part).some((s) => s.id === id && !s.selected)) {
        for (const s of part.Stream.filter((s) => s.streamType === streamType.SUBTITLE)) s.selected = s.id === id;
        changed[1] = id;
      }
    }
  }
  return changed;
}

// Choosing versions ----------------------------------------------------------

function codecKey(codec: string): string {
  const c = asciiLower(codec.trim());
  switch (c) {
    case "hevc":
    case "h265":
    case "h.265":
    case "x265":
      return "hevc";
    case "h264":
    case "h.264":
    case "avc":
    case "x264":
    case "avc1":
      return "h264";
    default:
      return c;
  }
}

function mediaResolution(m: Media): number | null {
  if (m.videoResolution !== null) {
    switch (asciiLower(m.videoResolution)) {
      case "4k":
      case "2160":
        return 4;
      case "1080":
        return 3;
      case "720":
        return 2;
      case "576":
      case "480":
      case "sd":
        return 1;
    }
  }
  const part = m.Part[0];
  const h = m.height ?? (part ? (Part.videoStream(part)?.height ?? null) : null);
  if (h === null) return null;
  if (h >= 2000) return 4;
  if (h >= 1000) return 3;
  if (h >= 700) return 2;
  return 1;
}

export function mediaIsHdr(m: Media): boolean {
  const part = m.Part[0];
  const video = part ? Part.videoStream(part) : null;
  if (video && Stream.hdrLabel(video) !== null) return true;
  const name = [m.title, m.editionTitle]
    .filter((t): t is string => t !== null)
    .map((t) => t.toLowerCase())
    .join(" ");
  return (
    ["hdr", "dolby vision", "hlg"].some((k) => name.includes(k)) ||
    name.split(/[^\p{Alphabetic}\p{N}]/u).some((w) => w === "dv")
  );
}

/**
 * The index of the version of a title that best fits the rules; 0 (Plex's
 * first) when the rules have no version preference or nothing fits better.
 */
export function pickVersion(r: Resolved, versions: readonly Media[]): number {
  if (versions.length < 2 || !r.hasVersionRules()) return 0;
  const want = resolutionRank(r.resolution);
  const score = (m: Media): number => {
    let s = 0;
    if (want !== null) {
      const have = mediaResolution(m);
      // Closest wins; at equal distance the higher resolution does.
      s += have !== null ? Math.abs(have - want) * 100 + flag(have < want) : 150;
    }
    if (r.codec !== null) {
      const c = m.videoCodec !== null ? codecKey(m.videoCodec) : null;
      if (c !== r.codec) s += 20;
    }
    const hdr = mediaIsHdr(m);
    if ((r.hdr === "prefer" && !hdr) || (r.hdr === "avoid" && hdr)) s += 30;
    return s;
  };
  const picked = minByKey(
    versions.map((m, i) => i),
    (i) => [score(versions[i]!), i],
  );
  return picked ?? 0;
}

// Storage ----------------------------------------------------------------------

export { Scope };

/** The connected account and server. */
export function currentScope(app: AppState): Scope | null {
  const s = app.session();
  return s ? Scope.of(s) : null;
}

/** The scope whose rules apply to `item`: its own server's when it was found on another one. */
export function scopeFor(app: AppState, item: Metadata): Scope | null {
  const scope = currentScope(app);
  if (!scope) return null;
  const id = Metadata.serverId(item);
  if (id !== null) scope.server = id;
  return scope;
}

/** One saved set of rules: a profile's (`show` is `null`) or a show's. */
export interface Entry {
  account: string;
  server: string;
  /** Rating key of the show; `null` for the profile's rules. */
  show: string | null;
  title: string | null;
  rules: Rules;
}

function path(): string {
  return join(config.configDir(), "playback-prefs.json");
}

function load(): Entry[] {
  try {
    const raw = storage.readJson<unknown[]>(path(), []);
    return (Array.isArray(raw) ? raw : []).map((e) => {
      const o = e as Record<string, unknown>;
      return {
        account: String(o.account ?? ""),
        server: String(o.server ?? ""),
        show: typeof o.show === "string" ? o.show : null,
        title: typeof o.title === "string" ? o.title : null,
        rules: Rules.from(o.rules),
      };
    });
  } catch (e) {
    log.error(`playback preferences: ${errorText(e)}`);
    return [];
  }
}

export function inScope(entries: readonly Entry[], scope: Scope): Entry[] {
  return entries.filter((e) => e.account === scope.account && e.server === scope.server);
}

function find(entries: readonly Entry[], scope: Scope, show: string | null): Entry | undefined {
  return entries.find((e) => e.account === scope.account && e.server === scope.server && e.show === show);
}

/** What applies to `show` (or to the profile alone): the show's rules over the profile's. */
export function effective(entries: readonly Entry[], scope: Scope, show: string | null): Rules {
  const profile = find(entries, scope, null)?.rules.clone() ?? new Rules();
  const e = show !== null ? find(entries, scope, show) : undefined;
  return e ? e.rules.over(profile) : profile;
}

export function store(entries: Entry[], scope: Scope, show: string | null, title: string | null, rules: Rules): void {
  const kept = entries.filter((e) => !(e.account === scope.account && e.server === scope.server && e.show === show));
  entries.splice(0, entries.length, ...kept);
  if (!rules.isEmpty()) {
    entries.push({ account: scope.account, server: scope.server, show, title, rules });
  }
}

/** The show an item belongs to, for per-show rules. */
function showOf(item: Metadata): string | null {
  switch (Metadata.kind(item)) {
    case "episode":
      return item.grandparentRatingKey;
    case "season":
      return item.parentRatingKey;
    case "show":
      return item.ratingKey;
    default:
      return null;
  }
}

/** Everything a front end shows about one profile or show. */
export interface View {
  /** The profile's own rules. */
  profile: Rules;
  /** The show's own rules, when a show was asked for. */
  show: Rules | null;
  /** What applies: the show's rules over the profile's. */
  effective: Rules;
}

/** A show with rules of its own. */
export interface ShowRules {
  ratingKey: string;
  title: string;
  rules: Rules;
}

function scope(app: AppState): Scope {
  const s = currentScope(app);
  if (!s) throw new Error("Not connected to a server.");
  return s;
}

export function view(app: AppState, show: string | null = null): View {
  const sc = scope(app);
  const entries = load();
  return {
    profile: find(entries, sc, null)?.rules.clone() ?? new Rules(),
    show: show !== null ? (find(entries, sc, show)?.rules.clone() ?? new Rules()) : null,
    effective: effective(entries, sc, show),
  };
}

/** The shows of this profile that have rules of their own. */
export function shows(app: AppState): ShowRules[] {
  const sc = scope(app);
  const entries = load();
  const out: ShowRules[] = [];
  for (const e of inScope(entries, sc)) {
    if (e.show === null) continue;
    out.push({ ratingKey: e.show, title: e.title ?? "Show", rules: e.rules.clone() });
  }
  // Stable sort by lowercase title, like `sort_by_key`.
  const key = (s: ShowRules) => s.title.toLowerCase();
  out.sort((a, b) => (key(a) < key(b) ? -1 : key(a) > key(b) ? 1 : 0));
  return out;
}

/**
 * Replace the rules of the profile (`show` is `null`) or of a show. Empty
 * rules remove the entry, so the show follows the profile again.
 */
export function save(app: AppState, show: string | null, title: string | null, rules: Rules | Partial<RulesFields>): void {
  const sc = scope(app);
  const r = rules instanceof Rules ? rules : Rules.from(rules);
  storage.updateJson<Entry[], void>(path(), [], (entries) => store(entries, sc, show, title, r));
}

export function clear(app: AppState, show: string | null = null): void {
  save(app, show, null, new Rules());
}

/**
 * What a person chose in the whole-show track dialog, as rules: the audio
 * language, and subtitles off, or in a language (forced or full as picked).
 * `subtitle` is `undefined` when subtitles were left alone, `null` for off.
 */
export function rulesFromTracks(audio: Stream | null, subtitle: Stream | null | undefined): Rules {
  const rules = new Rules();
  const lang = audio ? streamLang(audio) : null;
  if (audio && lang !== null) {
    rules.audioLanguages = [lang];
    rules.avoidCommentary = !isCommentary(audio);
  }
  if (subtitle === undefined) {
    // nothing chosen
  } else if (subtitle === null) {
    rules.subtitles = "off";
  } else {
    const l = streamLang(subtitle);
    if (l !== null) rules.subtitleLanguages = [l];
    rules.subtitles = "always";
    rules.forced = isForced(subtitle) ? "only" : "avoid";
  }
  return rules;
}

/** Remember the choices of the track dialog as this show's rules. */
export function rememberTracks(app: AppState, q: tracks.Query): void {
  const { audio, subtitle, title } = tracks.chosen(app, q);
  const rules = rulesFromTracks(audio, subtitle);
  if (rules.isEmpty()) throw new Error("Choose an audio or subtitle track to remember.");
  // Keep the show's other rules (version preferences) while replacing its tracks.
  const current = view(app, q.show).show ?? new Rules();
  const merged = new Rules({
    audioLanguages: rules.audioLanguages ?? current.audioLanguages,
    subtitleLanguages: rules.subtitles !== null ? rules.subtitleLanguages : current.subtitleLanguages,
    subtitles: rules.subtitles ?? current.subtitles,
    forced: rules.subtitles !== null ? rules.forced : current.forced,
    avoidCommentary: rules.avoidCommentary ?? current.avoidCommentary,
    resolution: current.resolution,
    codec: current.codec,
    hdr: current.hdr,
  });
  save(app, q.show, title, merged);
  app.toast(`Remembered these tracks for ${title}`);
}

// Hooks used by playback -------------------------------------------------------

/**
 * The version of `item` to play: an explicit choice (`requested`, including
 * the first version) always stands; otherwise the pick of the rules of `scope`
 * (the account and the server the item plays from).
 */
export function mediaIndex(scope: Scope | null, item: Metadata, requested: number | null = null): number {
  if (requested !== null) return requested;
  if (item.Media.length < 2 || !["movie", "episode"].includes(Metadata.kind(item))) return 0;
  if (!scope) return 0;
  return pickVersion(effective(load(), scope, showOf(item)).resolve(), item.Media);
}

/** Which version of `item` the rules prefer (for pre-selecting it in a picker). */
export function preferredVersion(app: AppState, item: Metadata): number {
  return mediaIndex(scopeFor(app, item), item, null);
}

/**
 * The track rules that apply to `item` when it starts: `null` for anything but a movie
 * or episode, a resumed item (it keeps the tracks it had), or no track rules.
 */
function trackRules(scope: Scope | null, item: Metadata): Resolved | null {
  if (!["movie", "episode"].includes(Metadata.kind(item)) || (item.viewOffset ?? 0) > 0) return null;
  if (!scope) return null;
  const rules = effective(load(), scope, showOf(item)).resolve();
  return rules.hasTrackRules() ? rules : null;
}

/** Select the rules' tracks in `item`; returns the part and the streams that changed. */
function chooseIn(rules: Resolved, item: Metadata, mediaIdx: number): [number, number | null, number | null] | null {
  const index = mediaIdx < item.Media.length ? mediaIdx : 0;
  const part = item.Media[index]?.Part[0];
  if (!part) return null;
  const partId = part.id;
  if (partId === null) return null;
  const choice = chooseStreams(rules, part);
  const [audio, subtitle] = applyChoice(part, choice);
  return audio !== null || subtitle !== null ? [partId, audio, subtitle] : null;
}

/**
 * Apply the rules to the streams of an item that is about to play: select the
 * tracks in `item` (so mpv is given them) and save them to Plex in the
 * background so other apps agree. A resumed item keeps the tracks it had.
 * `scope` is the account and the server the item plays from.
 */
export function applyAuto(app: AppState, scope: Scope | null, server: PlexServer, item: Metadata, mediaIdx: number): void {
  const rules = trackRules(scope, item);
  if (!rules) return;
  const changed = chooseIn(rules, item, mediaIdx);
  if (!changed) return;
  const [partId, audio, subtitle] = changed;
  app.spawn(
    server.selectStreams(partId, audio, subtitle, false).catch((e) => {
      log.debug(`could not save automatic track choice: ${errorText(e)}`);
    }),
  );
}

/**
 * The same choice for an item that will be transcoded: the transcoder plays (and burns in)
 * the server's selection, so it is saved before the transcode starts. Queue entries can lack
 * streams; the full item is fetched then.
 */
export async function saveAuto(scope: Scope | null, server: PlexServer, item: Metadata, mediaIdx: number): Promise<void> {
  const rules = trackRules(scope, item);
  if (!rules) return;
  const first = (item.Media[mediaIdx] ?? item.Media[0])?.Part[0];
  const hasStreams = first !== undefined && first.Stream.length > 0;
  let full: Metadata;
  if (hasStreams) {
    full = structuredClone(item);
  } else {
    const fetched = await server.metadata(Metadata.ratingKey(item));
    if (!fetched) throw new Error(`"${Metadata.title(item)}" was not found`);
    full = fetched;
  }
  const changed = chooseIn(rules, full, mediaIdx);
  if (changed) await server.selectStreams(changed[0], changed[1], changed[2], false);
}
