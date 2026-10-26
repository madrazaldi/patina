// Player controls beyond transport (volume, speed, chapters, repeat, a sleep
// timer) and the live diagnostics shown beside them (buffering, hardware
// decoding, dropped frames, connection type).
//
// The controls act on mpv directly. What the front ends show arrives as one
// `PlaybackState` event, emitted when something changes (diagnostics at
// most once a second). The state is also kept here, for front ends that open
// while something is already playing.

import { isIPv4, isIPv6 } from "node:net";
import type { Mpv } from "../mpv";
import { Resource } from "../plex";
import type { Chapter, PlexServer } from "../plex";
import * as player from "./player";
import type { AppState, Session } from "./state";

/** What happens when the queue or item ends. */
export type Repeat = "off" | "one" | "all";

/** Off, then the whole queue, then the item, then off again. */
export function nextRepeat(r: Repeat): Repeat {
  switch (r) {
    case "off":
      return "all";
    case "all":
      return "one";
    case "one":
      return "off";
  }
}

export function repeatLabel(r: Repeat): string {
  switch (r) {
    case "off":
      return "off";
    case "one":
      return "repeat one";
    case "all":
      return "repeat all";
  }
}

/** Sleep timer presets cycled through by `cycleSleep`, in minutes. */
export const SLEEP_PRESETS: readonly number[] = [15, 30, 45, 60, 90];

/** A change to the running player. */
export type Tune =
  /** Volume in percent (0 to 130). */
  | { kind: "volume"; value: number }
  | { kind: "volumeBy"; delta: number }
  /** Mute, unmute, or toggle with `null`. */
  | { kind: "mute"; on?: boolean | null }
  /** Playback speed (0.25 to 4). */
  | { kind: "speed"; value: number }
  | { kind: "speedBy"; delta: number }
  | { kind: "repeat"; mode: Repeat }
  | { kind: "cycleRepeat" }
  /** Jump to a chapter by index. */
  | { kind: "chapter"; index: number }
  | { kind: "chapterBy"; delta: number }
  | { kind: "sleepOff" }
  | { kind: "sleepMinutes"; minutes: number }
  | { kind: "sleepEndOfItem" }
  /** Off, then each of `SLEEP_PRESETS`, then the end of the item, then off. */
  | { kind: "cycleSleep" };

export interface ChapterView {
  title: string;
  timeMs: number;
}

/** How the server is reached. */
export interface ConnectionInfo {
  /** `local`, `remote`, `relay` or `offline` (a downloaded file). */
  kind: string;
  address: string | null;
}

export function offlineConnection(): ConnectionInfo {
  return { kind: "offline", address: null };
}

export interface Diagnostics {
  /** Playback is stalled waiting for data. */
  buffering: boolean;
  /** How full the buffer is while stalled, in percent. */
  bufferPercent: number | null;
  /** Seconds of media ahead of the playhead that are already buffered. */
  bufferedSecs: number | null;
  /** The hardware decoder in use; `no` is software decoding, `null` is unknown (or audio only). */
  hwdec: string | null;
  /** Frames the video output dropped. */
  droppedFrames: number;
  /** Frames the decoder dropped. */
  decoderDroppedFrames: number;
  connection: ConnectionInfo | null;
  /** Where playback restarted from a local checkpoint after the connection failed, if it did. */
  recoveredFromMs: number | null;
}

export interface SleepState {
  /** `minutes` or `endOfItem`. */
  mode: string;
  /** When the timer ends, in milliseconds since the epoch (`minutes` only). */
  endsAtMs: number | null;
}

export interface PlaybackState {
  /** Percent; mpv allows up to 130. */
  volume: number;
  muted: boolean;
  speed: number;
  repeat: Repeat;
  sleep: SleepState | null;
  chapters: ChapterView[];
  /** Index into `chapters` of the playing chapter. */
  chapter: number | null;
  diagnostics: Diagnostics;
}

// Process-wide settings ---------------------------------------------------------

type Sleep = { kind: "minutes"; id: number; endsAtMs: number } | { kind: "endOfItem" };

interface Tuning {
  volume: number | null;
  muted: boolean;
  repeat: Repeat;
  sleep: Sleep | null;
  /** Mirrors the player, which owns "stop after this". */
  stopAfter: boolean;
}

const TUNING: Tuning = { volume: null, muted: false, repeat: "off", sleep: null, stopAfter: false };
let STATE: PlaybackState | null = null;
let NEXT_TIMER = 1;

function nowMs(): number {
  return Date.now();
}

/** The last published state; `null` when nothing is playing. */
export function playbackState(): PlaybackState | null {
  return STATE;
}

export function repeat(): Repeat {
  return TUNING.repeat;
}

/** The player toggled "stop after this"; repeat-one must not keep the item from ending. */
export function setStopAfter(on: boolean): void {
  TUNING.stopAfter = on;
  if (!on && TUNING.sleep?.kind === "endOfItem") TUNING.sleep = null;
}

/** Whether the sleep timer is "end of this item". */
export function sleepAtItemEnd(): boolean {
  return TUNING.sleep?.kind === "endOfItem";
}

/** Make a freshly started mpv honour the volume, mute and repeat choices made earlier. */
export async function applySaved(mpv: Mpv): Promise<void> {
  const { volume, muted, repeat: mode, stopAfter } = TUNING;
  if (volume !== null) await mpv.setProperty("volume", volume).catch(() => {});
  if (muted) await mpv.setProperty("mute", true).catch(() => {});
  await applyRepeat(mpv, mode, stopAfter);
}

/** mpv's loop properties for a repeat mode. "Stop after this" wins over repeating the item. */
export async function applyRepeat(mpv: Mpv, mode: Repeat, stopAfter: boolean): Promise<void> {
  const file = mode === "one" && !stopAfter ? "inf" : "no";
  const list = mode === "all" ? "inf" : "no";
  await mpv.setProperty("loop-file", file).catch(() => {});
  await mpv.setProperty("loop-playlist", list).catch(() => {});
}

/** Playback ended: drop what only applies to it. */
export function finished(app: AppState): void {
  TUNING.sleep = null;
  TUNING.stopAfter = false;
  STATE = null;
  app.emit({ kind: "playbackState", state: null });
}

function asF64(v: unknown): number | null {
  return typeof v === "number" ? v : null;
}

function asI64(v: unknown): number | null {
  return typeof v === "number" && Number.isInteger(v) ? v : null;
}

function asBool(v: unknown): boolean | null {
  return typeof v === "boolean" ? v : null;
}

/** Apply a `Tune` to the running player. */
export async function apply(app: AppState, mpv: Mpv, cmd: Tune): Promise<void> {
  switch (cmd.kind) {
    case "volume":
      await setVolume(mpv, cmd.value);
      break;
    case "volumeBy": {
      const now = asF64(await mpv.getProperty("volume").catch(() => null)) ?? TUNING.volume ?? 100;
      await setVolume(mpv, now + cmd.delta);
      break;
    }
    case "mute": {
      const now = asBool(await mpv.getProperty("mute").catch(() => null)) ?? false;
      const muted = cmd.on ?? !now;
      await mpv.setProperty("mute", muted);
      TUNING.muted = muted;
      break;
    }
    case "speed":
      await setSpeed(mpv, cmd.value);
      break;
    case "speedBy": {
      const now = asF64(await mpv.getProperty("speed").catch(() => null)) ?? 1;
      await setSpeed(mpv, now + cmd.delta);
      break;
    }
    case "repeat":
      await setRepeat(mpv, cmd.mode);
      break;
    case "cycleRepeat":
      await setRepeat(mpv, nextRepeat(repeat()));
      break;
    case "chapter":
      await jumpChapter(mpv, cmd.index);
      break;
    case "chapterBy": {
      const state = playbackState();
      if (!state) throw new Error("Nothing is playing");
      const current = state.chapter ?? (cmd.delta > 0 ? -1 : state.chapters.length);
      await jumpChapter(mpv, current + cmd.delta);
      break;
    }
    case "sleepOff":
      setSleep(app, null);
      break;
    case "sleepMinutes":
      setSleep(app, minutesTimer(cmd.minutes));
      break;
    case "sleepEndOfItem":
      setSleep(app, { kind: "endOfItem" });
      break;
    case "cycleSleep": {
      const sleep = TUNING.sleep;
      let next: Sleep | null;
      if (sleep === null) {
        next = minutesTimer(SLEEP_PRESETS[0]!);
      } else if (sleep.kind === "minutes") {
        // The preset at or above what is left, then the one after it.
        const left = Math.ceil(Math.max(0, sleep.endsAtMs - nowMs()) / 60_000);
        const i = SLEEP_PRESETS.findIndex((p) => p >= left);
        next = i >= 0 && i + 1 < SLEEP_PRESETS.length ? minutesTimer(SLEEP_PRESETS[i + 1]!) : { kind: "endOfItem" };
      } else {
        next = null;
      }
      setSleep(app, next);
      break;
    }
  }
  refresh(app);
}

function minutesTimer(minutes: number): Sleep {
  return { kind: "minutes", id: NEXT_TIMER++, endsAtMs: nowMs() + minutes * 60_000 };
}

async function setVolume(mpv: Mpv, value: number): Promise<void> {
  const v = Math.round(Math.min(130, Math.max(0, value)));
  await mpv.setProperty("volume", v);
  TUNING.volume = v;
}

async function setSpeed(mpv: Mpv, value: number): Promise<void> {
  await mpv.setProperty("speed", Math.round(Math.min(4, Math.max(0.25, value)) * 100) / 100);
}

async function setRepeat(mpv: Mpv, mode: Repeat): Promise<void> {
  TUNING.repeat = mode;
  await applyRepeat(mpv, mode, TUNING.stopAfter);
}

async function jumpChapter(mpv: Mpv, index: number): Promise<void> {
  const state = playbackState();
  if (!state) throw new Error("Nothing is playing");
  const chapter = Number.isInteger(index) && index >= 0 ? state.chapters[index] : undefined;
  if (!chapter) throw new Error("No such chapter");
  await mpv.seekAbsolute(chapter.timeMs / 1000);
}

function setSleep(app: AppState, sleep: Sleep | null): void {
  TUNING.sleep = sleep;
  if (sleep?.kind === "minutes") {
    const { id, endsAtMs } = sleep;
    const wait = Math.max(0, endsAtMs - nowMs());
    const timer = setTimeout(() => {
      const cur = TUNING.sleep;
      if (cur?.kind === "minutes" && cur.id === id) {
        TUNING.sleep = null;
        app.toast("Sleep timer: playback stopped");
        player.control({ kind: "stop" });
        refresh(app);
      }
    }, wait);
    // The timer never keeps the process alive.
    (timer as { unref?: () => void }).unref?.();
  }
}

/** Publish the shared state again after a control changed the process-wide parts of it. */
function refresh(app: AppState): void {
  if (!STATE) return;
  const [rep, sleep] = globalParts();
  STATE = { ...STATE, repeat: rep, sleep };
  app.emit({ kind: "playbackState", state: structuredClone(STATE) });
}

function globalParts(): [Repeat, SleepState | null] {
  const s = TUNING.sleep;
  const sleep: SleepState | null = s
    ? s.kind === "minutes"
      ? { mode: "minutes", endsAtMs: s.endsAtMs }
      : { mode: "endOfItem", endsAtMs: null }
    : null;
  return [TUNING.repeat, sleep];
}

// Following mpv -----------------------------------------------------------------

/** mpv properties `Tracker` follows; observe each with a distinct id. */
export const OBSERVED: readonly string[] = [
  "volume",
  "mute",
  "speed",
  "chapter-list",
  "chapter",
  "paused-for-cache",
  "cache-buffering-state",
  "demuxer-cache-duration",
  "hwdec-current",
  "frame-drop-count",
  "decoder-frame-drop-count",
];

/** First id used to observe `OBSERVED`; the player's own properties use the ids below it. */
export const OBSERVE_BASE = 100;

function chapterTitle(title: string | null | undefined, i: number): string {
  return title != null && title.trim() !== "" ? title : `Chapter ${i + 1}`;
}

/** Keeps the `PlaybackState` of one player up to date from its property changes. */
export class Tracker {
  private volume = 100;
  private muted = false;
  private speed = 1;
  private mpvChapters: ChapterView[] = [];
  private plexChapters: ChapterView[] = [];
  private mpvChapter: number | null = null;
  private positionMs = 0;
  private diag: Diagnostics;
  private emitted: string | null = null;
  private lastEmit = performance.now() - 5000;

  constructor(connection: ConnectionInfo) {
    this.diag = {
      buffering: false,
      bufferPercent: null,
      bufferedSecs: null,
      hwdec: null,
      droppedFrames: 0,
      decoderDroppedFrames: 0,
      connection,
      recoveredFromMs: null,
    };
  }

  static observes(name: string): boolean {
    return OBSERVED.includes(name);
  }

  /** Start observing; errors for properties an older mpv lacks are ignored. */
  static async observe(mpv: Mpv): Promise<void> {
    for (let i = 0; i < OBSERVED.length; i++) {
      await mpv.observeProperty(OBSERVE_BASE + i, OBSERVED[i]!).catch(() => {});
    }
  }

  /** Take in a property change; true when it should be published right away. */
  onProperty(name: string, data: unknown): boolean {
    switch (name) {
      case "volume":
        this.volume = asF64(data) ?? this.volume;
        break;
      case "mute":
        this.muted = asBool(data) ?? false;
        break;
      case "speed":
        this.speed = asF64(data) ?? 1;
        break;
      case "chapter-list":
        this.mpvChapters = Array.isArray(data)
          ? data.map((c: unknown, i: number) => {
              const o = (typeof c === "object" && c !== null ? c : {}) as Record<string, unknown>;
              const title = typeof o.title === "string" && o.title.trim() !== "" ? o.title : null;
              return { title: chapterTitle(title, i), timeMs: Math.trunc((asF64(o.time) ?? 0) * 1000) };
            })
          : [];
        break;
      case "chapter":
        this.mpvChapter = asI64(data);
        break;
      case "paused-for-cache":
        this.diag.buffering = asBool(data) ?? false;
        break;
      case "cache-buffering-state":
        this.diag.bufferPercent = asI64(data);
        break;
      case "demuxer-cache-duration": {
        const s = asF64(data);
        this.diag.bufferedSecs = s === null ? null : Math.round(s * 10) / 10;
        break;
      }
      case "hwdec-current":
        this.diag.hwdec = typeof data === "string" && data !== "" ? data : null;
        break;
      case "frame-drop-count":
        this.diag.droppedFrames = asI64(data) ?? 0;
        break;
      case "decoder-frame-drop-count":
        this.diag.decoderDroppedFrames = asI64(data) ?? 0;
        break;
      default:
        return false;
    }
    return !["demuxer-cache-duration", "cache-buffering-state", "frame-drop-count", "decoder-frame-drop-count"].includes(name);
  }

  onTime(ms: number): void {
    this.positionMs = ms;
  }

  /** A new file: per-file figures start over. */
  newItem(plex: readonly Chapter[]): void {
    this.mpvChapters = [];
    this.mpvChapter = null;
    this.diag.buffering = false;
    this.diag.bufferPercent = null;
    this.diag.bufferedSecs = null;
    this.diag.droppedFrames = 0;
    this.diag.decoderDroppedFrames = 0;
    this.diag.recoveredFromMs = null;
    this.setPlexChapters(plex);
  }

  /** Chapters Plex knows, for streams (transcodes) that carry none of their own. */
  setPlexChapters(plex: readonly Chapter[]): void {
    this.plexChapters = plex.map((c, i) => ({ title: chapterTitle(c.tag, i), timeMs: c.startTimeOffset ?? 0 }));
  }

  recoveredFrom(ms: number | null): void {
    this.diag.recoveredFromMs = ms;
  }

  view(): PlaybackState {
    const chapters = (this.mpvChapters.length === 0 ? this.plexChapters : this.mpvChapters).map((c) => ({ ...c }));
    let chapter: number | null;
    if (this.mpvChapters.length > 0) {
      chapter = this.mpvChapter !== null && this.mpvChapter >= 0 ? this.mpvChapter : null;
    } else {
      chapter = null;
      for (let i = chapters.length - 1; i >= 0; i--) {
        if (chapters[i]!.timeMs <= this.positionMs) {
          chapter = i;
          break;
        }
      }
    }
    const [rep, sleep] = globalParts();
    return {
      volume: this.volume,
      muted: this.muted,
      speed: this.speed,
      repeat: rep,
      sleep,
      chapters,
      chapter,
      diagnostics: { ...this.diag, connection: this.diag.connection ? { ...this.diag.connection } : null },
    };
  }

  /**
   * Emit the state if it changed. Quiet changes (buffer levels, frame counts) wait for the
   * next second; `urgent` ones go out now.
   */
  publish(app: AppState, urgent: boolean): void {
    if (!urgent && performance.now() - this.lastEmit < 1000) return;
    const state = this.view();
    const key = JSON.stringify(state);
    if (this.emitted === key) return;
    this.lastEmit = performance.now();
    this.emitted = key;
    STATE = state;
    app.emit({ kind: "playbackState", state: structuredClone(state) });
  }
}

// Connection ----------------------------------------------------------------------

/**
 * How the session's server is reached: from Plex's connection list when it names the
 * address in use, else from the address itself.
 */
export function connectionInfo(session: Session): ConnectionInfo {
  return connectionInfoOf(session, session.server, session.serverId);
}

/** `connectionInfo` for `server`, one of the account's servers with id `serverId`. */
export function connectionInfoOf(session: Session, server: PlexServer, serverId: string): ConnectionInfo {
  const base = new URL(String(server.baseUrl()));
  const host = base.hostname;
  const address = base.port !== "" ? `${host}:${base.port}` : host;
  const target = base.href.replace(/\/+$/, "");
  const connections = session.servers.filter((r) => Resource.id(r) === serverId).flatMap((r) => r.connections);
  const known =
    connections.find((c) => c.uri != null && c.uri.replace(/\/+$/, "") === target) ??
    connections.find((c) => c.address === host && c.port === (base.port !== "" ? Number(base.port) : null));
  let kind: string;
  if (known) kind = known.relay ? "relay" : known.local ? "local" : "remote";
  else kind = isLocalHost(host) ? "local" : "remote";
  return { kind, address };
}

/** The leading 16-bit group of an IPv6 address, and whether it is `::1`. */
function v6Info(host: string): { first: number; loopback: boolean } | null {
  let h = host;
  const zone = h.indexOf("%");
  if (zone >= 0) h = h.slice(0, zone);
  if (!isIPv6(h)) return null;
  const dbl = h.indexOf("::");
  const parts = (s: string) => (s === "" ? [] : s.split(":"));
  const head = dbl >= 0 ? parts(h.slice(0, dbl)) : parts(h);
  const tail = dbl >= 0 ? parts(h.slice(dbl + 2)) : [];
  const groups = (xs: string[]): number[] =>
    xs.flatMap((x) => {
      if (x.includes(".")) {
        const o = x.split(".").map(Number);
        return [(o[0]! << 8) | o[1]!, (o[2]! << 8) | o[3]!];
      }
      return [parseInt(x, 16)];
    });
  const a = groups(head);
  const b = groups(tail);
  const all = dbl >= 0 ? [...a, ...new Array<number>(Math.max(0, 8 - a.length - b.length)).fill(0), ...b] : a;
  return { first: all[0] ?? 0, loopback: all.length === 8 && all.slice(0, 7).every((x) => x === 0) && all[7] === 1 };
}

/** Whether an address is on this machine or its network. */
export function isLocalHost(hostIn: string): boolean {
  let host = hostIn.replace(/^[[\]]+|[[\]]+$/g, "").toLowerCase();
  if (host.endsWith(".plex.direct")) {
    // `192-168-1-5.<id>.plex.direct` names the address in its first label.
    host = host.slice(0, -".plex.direct".length).split(".")[0]!.replaceAll("-", ".");
  }
  if (host === "localhost" || host.endsWith(".local")) return true;
  if (isIPv4(host)) {
    const [a, b] = host.split(".").map(Number) as [number, number];
    // private, loopback, link-local
    return a === 10 || (a === 172 && b >= 16 && b <= 31) || (a === 192 && b === 168) || a === 127 || (a === 169 && b === 254);
  }
  const v6 = v6Info(host);
  if (v6) return v6.loopback || (v6.first & 0xfe00) === 0xfc00 || (v6.first & 0xffc0) === 0xfe80;
  // A bare machine name only resolves on the local network.
  return !host.includes(".");
}
