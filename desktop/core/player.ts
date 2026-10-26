// Playback through an external mpv process.
//
// A play request becomes a Plex play queue. Every queue item is appended to
// mpv's playlist so mpv's own next/previous bindings work, while we follow
// along over IPC: reporting timeline progress, marking items watched,
// mirroring audio/subtitle choices between mpv and Plex, and offering
// intro/credits skipping from Plex markers.
//
// The queue can be edited while it plays (play next, remove, reorder,
// shuffle). Edits are applied to mpv's playlist and mirrored to the Plex
// play queue where it has the items.
//
// The Rust playback loop is a `tokio::select!` over mpv's events, remote commands, answers to
// background requests and two timers. Here they all feed one inbox (a `Channel`) that the loop
// drains one message at a time, which keeps the same guarantee: a handler runs to completion
// before the next message is looked at.

import { randomUUID } from "node:crypto";
import { Channel, Mpv, defaultSpawnOptions } from "../mpv";
import type { Event } from "../mpv";
import { Metadata, MetadataOptions, Part, Stream, defaultMetadata, normalizeMetadata } from "../plex";
import type { Marker, MediaContainer, PlayQueueRequestInit, PlexServer, TimelineState } from "../plex";
import { streamType } from "../plex";
import { mpvProgram, playerQuality, quality as qualityByName } from "./config";
import type { PlayerSettings, Quality } from "./config";
import * as controls from "./controls";
import type { Tune } from "./controls";
import { Tracker } from "./controls";
import type { Download } from "./downloads";
import * as failure from "./failure";
import { logger } from "./log";
import * as offline from "./offline";
import * as prefs from "./prefs";
import * as recovery from "./recovery";
import { Scope } from "./recovery";
import * as report from "./report";
import type { QueueOp } from "./report";
import { reporter } from "./report";
import * as servers from "./servers";
import type { AppState, NowPlaying, QueueView } from "./state";

const log = logger("player");

/** `{e:#}` of a caught value. */
function text(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** `anyhow::Context`: "context: error", keeping the original as the cause. */
function context(e: unknown, ctx: string): Error {
  return new Error(`${ctx}: ${text(e)}`, { cause: e });
}

let ACTIVE: Mpv | null = null;
/** The running playback loop's inbox; closed when the loop ends. */
let COMMANDS: Channel<Inbox> | null = null;
let NEXT_UID = 1;
/** Playbacks being driven right now, so quitting can wait for their last reports. */
let RUNS = 0;
/** A position to start the next playback's first item from (retry, resuming a saved queue). */
let START_OFFSET: [string, number] | null = null;

/** Commands for the playback loop, which owns the queue and the player's state. */
type Cmd = { kind: "queue"; cmd: QueueCommand } | { kind: "tune"; cmd: Tune };

/** Results of requests made off the loop, so slow answers never hold up mpv's events. */
type Internal =
  /** The full item (markers, streams, chapters) of a queue entry. */
  | { kind: "detail"; uid: number; item: Metadata | null }
  /** The playable items behind something added to the queue. */
  | { kind: "leaves"; source: Metadata; next: boolean; result: { ok: Metadata[] } | { err: string } }
  /** Plex's play queue after items were added to it. */
  | { kind: "added"; uids: number[]; queue: MediaContainer | null }
  /** The items of the Plex play queue, to find what follows the end of the local one. */
  | { kind: "upcoming"; items: Metadata[] };

/** Everything the playback loop waits for. */
type Inbox =
  | { kind: "event"; event: Event }
  | { kind: "eventsEnded" }
  | { kind: "cmd"; cmd: Cmd }
  | { kind: "internal"; msg: Internal }
  | { kind: "tick" }
  | { kind: "stateTick" };

/** Whether a player is running. */
export function isActive(): boolean {
  return ACTIVE !== null;
}

function uid(): number {
  return NEXT_UID++;
}

/** What to play. */
export interface PlayRequest {
  item: Metadata;
  /** Explicit bulk selection; disables automatic episode expansion. */
  selection: Metadata[];
  /** Resume from the saved position (otherwise start from the beginning). */
  resume: boolean;
  shuffle: boolean;
  /** Which version (media) of the starting item to play; `null` lets the playback preferences pick. */
  mediaIndex: number | null;
  /** Play the item's radio station instead of the item (music). */
  radio: boolean;
  /** A tuned Live TV session; always transcoded, never scrobbled. Not part of the wire format. */
  live: boolean;
  /** Quality preset id or label for this playback; the settings' otherwise. */
  quality: string | null;
}

/** `PlayRequest::default()`. */
export function defaultPlayRequest(): PlayRequest {
  return playRequest();
}

/** `PlayRequest::default()` with fields overridden by `over`. */
export function playRequest(over: Partial<PlayRequest> = {}): PlayRequest {
  return {
    item: defaultMetadata(),
    selection: [],
    resume: true,
    shuffle: false,
    mediaIndex: null,
    radio: false,
    live: false,
    quality: null,
    ...over,
  };
}

/** Read a request as the front end sends it (serde: camelCase, every field optional, `live` never read). */
export function parsePlayRequest(v: unknown): PlayRequest {
  const o = (typeof v === "object" && v !== null ? v : {}) as Record<string, unknown>;
  const list = (x: unknown): Metadata[] => (Array.isArray(x) ? x.map((m) => normalizeMetadata(m)) : []);
  return {
    item: o.item != null ? normalizeMetadata(o.item) : defaultMetadata(),
    selection: list(o.selection),
    resume: typeof o.resume === "boolean" ? o.resume : true,
    shuffle: typeof o.shuffle === "boolean" ? o.shuffle : false,
    mediaIndex: typeof o.mediaIndex === "number" ? o.mediaIndex : null,
    radio: typeof o.radio === "boolean" ? o.radio : false,
    live: false,
    quality: typeof o.quality === "string" ? o.quality : null,
  };
}

/** Remote-control commands for the running player. */
export type Control =
  | { kind: "togglePause" }
  | { kind: "play" }
  | { kind: "pause" }
  | { kind: "next" }
  | { kind: "previous" }
  | { kind: "stop" }
  | { kind: "seekRelative"; seconds: number }
  | { kind: "seekTo"; seconds: number };

/**
 * Edits to the running queue. Entries are addressed by the `uid` sent in
 * `QueueView`, which stays stable while the queue is reordered.
 */
export type QueueCommand =
  /** Queue an item (or every item of a container) next or at the end. */
  | { kind: "add"; item: Metadata; next: boolean }
  | { kind: "remove"; uid: number }
  /** Move an entry before another one, or to the end. */
  | { kind: "move"; uid: number; before?: number | null }
  | { kind: "jump"; uid: number }
  /** Shuffle what comes after the current item, or restore its order. */
  | { kind: "shuffle"; on: boolean }
  /** Quit when the current item ends instead of moving on. */
  | { kind: "stopAfter"; on: boolean };

export function play(app: AppState, req: PlayRequest): void {
  failure.clear(app);
  // The account and server this playback belongs to, for a failure reported after a switch.
  const session = app.session();
  const origin = session ? Scope.of(session) : null;
  void app.spawn(async () => {
    const retry = req;
    try {
      await run(app, req, origin);
    } catch (e) {
      log.error(`playback failed: ${text(e)}`);
      app.setNowPlaying(null);
      app.setQueue(null);
      controls.finished(app);
      // Nothing was playing yet (no server, no queue, mpv would not start): offer to try again.
      const [kind, message] = failure.explainError(e);
      failure.record(app, {
        item: retry.item,
        error: message,
        kind,
        phase: "start",
        positionMs: 0,
        mediaIndex: retry.mediaIndex ?? 0,
        quality: retry.quality ?? playerQuality(app.settings().player).id,
        remaining: Math.max(0, retry.selection.length - 1),
        request: retry,
        scope: origin,
      });
    }
  });
}

/** Play, starting the first item at `offsetMs` instead of the position the server remembers. */
export function playFrom(app: AppState, req: PlayRequest, offsetMs: number): void {
  if (offsetMs > 0) START_OFFSET = [Metadata.ratingKey(req.item), offsetMs];
  play(app, req);
}

export function control(cmd: Control): void {
  const mpv = ACTIVE;
  if (!mpv) return;
  void (async () => {
    try {
      switch (cmd.kind) {
        case "togglePause":
          await mpv.command(["cycle", "pause"]);
          break;
        case "play":
          await mpv.setPause(false);
          break;
        case "pause":
          await mpv.setPause(true);
          break;
        case "next":
          await mpv.playlistNext();
          break;
        case "previous":
          await mpv.playlistPrev();
          break;
        case "stop":
          await mpv.quit();
          break;
        case "seekRelative":
          await mpv.command(["seek", cmd.seconds, "relative"]);
          break;
        case "seekTo":
          await mpv.seekAbsolute(cmd.seconds);
          break;
      }
    } catch {
      // `let _ = ...`
    }
  })();
}

/** The running loop's inbox, if its loop is still going. */
function liveCommands(): Channel<Inbox> | null {
  return COMMANDS && !COMMANDS.closed ? COMMANDS : null;
}

/** Enqueue a selection together, including when no player is running yet. */
export function queueItems(app: AppState, items: Metadata[]): void {
  const first = items[0];
  if (!first) return;
  if (!items.every((m) => queueKind(m) === queueKind(first))) throw new Error("Select only video or only music for a queue.");
  const tx = liveCommands();
  if (tx) {
    for (const item of items) {
      if (!tx.send({ kind: "cmd", cmd: { kind: "queue", cmd: { kind: "add", item, next: false } } })) throw new Error("Playback ended; try again.");
    }
  } else {
    play(app, playRequest({ item: first, selection: items }));
  }
}

/** Edit the running queue. Adding with nothing playing starts playback. */
export function queue(app: AppState, cmd: QueueCommand): void {
  const tx = liveCommands();
  if (tx) {
    tx.send({ kind: "cmd", cmd: { kind: "queue", cmd } });
  } else if (cmd.kind === "add") {
    play(app, playRequest({ item: cmd.item }));
  }
}

/** Change volume, speed, chapter, repeat or the sleep timer of the running player. */
export function tune(app: AppState, cmd: Tune): void {
  const tx = liveCommands();
  if (tx) {
    tx.send({ kind: "cmd", cmd: { kind: "tune", cmd } });
  } else if (ACTIVE) {
    // Playing downloads: no loop to ask, mpv can be changed directly.
    const mpv = ACTIVE;
    void controls.apply(app, mpv, cmd).catch((e) => app.error(text(e)));
  }
}

/**
 * Close mpv when Patina quits, and wait (a few seconds at most) for the last reports to reach Plex.
 * The Rust version blocks; this one resolves when it is done.
 */
export async function shutdown(): Promise<void> {
  const mpv = ACTIVE;
  ACTIVE = null;
  if (mpv) {
    await mpv.quit().catch(() => {});
    await mpv.wait(2000);
  }
  // Let the playback loop send its final report, then wait for Plex to take it.
  const deadline = Date.now() + 3000;
  while (RUNS > 0 && Date.now() < deadline) await Bun.sleep(25);
  await report.flush(5000);
}

interface Queue {
  items: Metadata[];
  start: number;
  kind: string;
  id: number | null;
}

export function queueKind(item: Metadata): "video" | "audio" | "photo" {
  switch (Metadata.kind(item)) {
    case "track":
    case "album":
    case "artist":
      return "audio";
    case "photo":
    case "photoalbum":
      return "photo";
    case "playlist":
      switch (item.playlistType) {
        case "audio":
          return "audio";
        case "photo":
          return "photo";
        default:
          return "video";
      }
    default:
      return "video";
  }
}

async function buildQueue(server: PlexServer, req: PlayRequest, settings: PlayerSettings): Promise<Queue> {
  const item = req.item;
  if (req.live) return { items: [item], start: 0, kind: "video", id: null };
  const rk = Metadata.ratingKey(item);
  const kind = queueKind(item);
  const pq: PlayQueueRequestInit = { kind, shuffle: req.shuffle };
  if (req.selection.length > 0) {
    pq.uri = server.itemUri(req.selection.map((m) => Metadata.ratingKey(m)));
  } else if (req.radio) {
    const station = (await server.station(rk).catch(() => null))?.key ?? null;
    if (station !== null) {
      pq.uri = server.libraryUri(station);
    } else {
      // No sonic analysis: shuffle the artist instead.
      let artist: string | null;
      switch (Metadata.kind(item)) {
        case "track":
          artist = item.grandparentRatingKey;
          break;
        case "album":
          artist = item.parentRatingKey;
          break;
        default:
          artist = rk;
      }
      pq.uri = server.itemUri([artist ?? rk]);
      pq.shuffle = true;
    }
    pq.kind = "audio";
  } else {
    switch (Metadata.kind(item)) {
      case "playlist":
        pq.playlistId = rk;
        break;
      case "collection": {
        const children = (await server.collectionItems(rk)).Metadata;
        pq.uri = server.itemUri(children.map((m) => Metadata.ratingKey(m)));
        break;
      }
      case "show":
      case "season":
      case "artist":
      case "album":
        pq.uri = server.itemUri([rk]);
        if (!req.shuffle) pq.key = Metadata.onDeckItem(item)?.key ?? null;
        break;
      default:
        pq.uri = server.itemUri([rk]);
        pq.key = item.key;
        pq.continuous = Metadata.kind(item) === "episode" && settings.continuous;
    }
  }

  let mc: MediaContainer | null = null;
  let err: unknown = null;
  try {
    mc = await server.createPlayQueue(pq);
  } catch (e) {
    err = e;
  }
  if (mc && mc.Metadata.length > 0) {
    // An explicit selection starts at the requested item; otherwise Plex says where.
    const requested =
      req.selection.length > 0 && item.ratingKey !== null ? mc.Metadata.findIndex((m) => m.ratingKey === item.ratingKey) : -1;
    let start = requested >= 0 ? requested : -1;
    if (start < 0) {
      start = mc.Metadata.findIndex((m) => m.playQueueItemID !== null && m.playQueueItemID === mc.playQueueSelectedItemID);
    }
    if (start < 0) start = 0;
    return { items: mc.Metadata, start, kind, id: mc.playQueueID };
  }
  if (req.selection.length > 0) {
    const items: Metadata[] = [];
    for (const it of req.selection) items.push(...(await leaves(server, it)));
    if (items.length === 0) throw new Error("nothing to play");
    return { items, start: 0, kind, id: null };
  }
  if (Metadata.isPlayable(item)) return { items: [item], start: 0, kind, id: null };
  if (err === null) throw new Error("nothing to play");
  throw context(err, "creating play queue");
}

/** The playable items of an item: itself, or everything below a container. */
async function leaves(server: PlexServer, item: Metadata): Promise<Metadata[]> {
  if (Metadata.isPlayable(item)) return [item];
  const rk = Metadata.ratingKey(item);
  let children: Metadata[];
  switch (Metadata.kind(item)) {
    case "playlist":
      children = (await server.playlistItems(rk, 0, 5000)).Metadata;
      break;
    case "collection":
      children = (await server.collectionItems(rk)).Metadata;
      break;
    default:
      return (await server.allLeaves(rk)).Metadata;
  }
  const out: Metadata[] = [];
  for (const child of children) {
    if (Metadata.isPlayable(child)) out.push(child);
    else out.push(...(await server.allLeaves(Metadata.ratingKey(child))).Metadata);
  }
  return out;
}

/** Split an argument string like a shell would, honouring single and double quotes. */
export function splitArgs(s: string): string[] {
  const out: string[] = [];
  let cur = "";
  let quote: string | null = null;
  let has = false;
  for (const c of s) {
    if (quote !== null) {
      if (c === quote) quote = null;
      else cur += c;
    } else if (c === '"' || c === "'") {
      quote = c;
      has = true;
    } else if (/\s/u.test(c)) {
      if (has || cur !== "") {
        out.push(cur);
        cur = "";
        has = false;
      }
    } else {
      cur += c;
    }
  }
  if (has || cur !== "") out.push(cur);
  return out;
}

function partOf(item: Metadata, mediaIndex: number): Part | null {
  return (item.Media[mediaIndex] ?? item.Media[0])?.Part[0] ?? null;
}

interface Loaded {
  url: string;
  transcodeSession: string | null;
}

/** Whether `item` plays through the transcoder rather than from its file. */
function transcoded(item: Metadata, quality: Quality, live: boolean): boolean {
  return live || (quality.kbps !== null && ["movie", "episode", "clip", "trailer"].includes(Metadata.kind(item)));
}

function mediaUrl(server: PlexServer, item: Metadata, mediaIndex: number, quality: Quality, live: boolean): Loaded {
  if (transcoded(item, quality, live)) {
    const session = randomUUID();
    const key = item.key;
    if (key === null) throw new Error("item has no key");
    const url = server.transcodeUrl(key, {
      session,
      mediaIndex,
      maxVideoBitrate: quality.kbps,
      videoResolution: quality.kbps !== null ? quality.resolution : null,
      subtitles: "burn",
    });
    return { url: url.toString(), transcodeSession: session };
  }
  const part = partOf(item, mediaIndex);
  if (!part) throw new Error(`"${Metadata.title(item)}" has no playable file`);
  if (part.key === null) throw new Error("media part has no key");
  return { url: server.url(part.key).toString(), transcodeSession: null };
}

/** One entry of mpv's playlist, in playlist order. */
interface Entry {
  uid: number;
  item: Metadata;
  mediaIndex: number;
  transcodeSession: string | null;
}

/** Per-item playback state. */
interface Current {
  uid: number;
  item: Metadata;
  mediaIndex: number;
  timeMs: number;
  durationMs: number;
  paused: boolean;
  reachedEnd: boolean;
  transcodeSession: string | null;
  /** Ignore track changes until this instant (we are applying Plex's choice). */
  applyingUntil: number;
  /** Sidecar subtitle URL -> Plex stream id. */
  sidecars: Map<string, number>;
  activeMarker: Marker | null;
  promptedMarker: number | null;
  /** In the closing credits or last seconds, with something queued after. */
  ending: boolean;
  lastUi: number;
  /** mpv's playlist entry id for this item, to match its `end-file`. */
  mpvId: number | null;
  /** mpv finished loading the file. */
  loaded: boolean;
  /** The full item (markers, streams, chapters) is still being fetched. */
  detailPending: boolean;
  /** The file loaded before the full item arrived: apply Plex's streams once it does. */
  streamsPending: boolean;
  /** When the position was last saved locally. */
  checkpointAt: number;
}

const now = (): number => performance.now();

/** The playlist-pos / time / pause / ... properties the loop observes, with ids 1.. in this order. */
const PLAYER_PROPERTIES = ["playlist-pos", "time-pos", "pause", "duration", "aid", "sid", "idle-active", "eof-reached"];

type OptionList = [string, string][];

class Player {
  // Everything below is the Rust `Player` struct; the fields are set by `run`.
  app!: AppState;
  server!: PlexServer;
  /** Machine id of `server` when it is not the session's own; queue items are tagged with it. */
  serverTag: string | null = null;
  scope: Scope | null = null;
  /** The account and connected server when playback started (`scope` is the item's server). */
  origin: Scope | null = null;
  settings!: PlayerSettings;
  quality!: Quality;
  mpv!: Mpv;
  kind = "video";
  live = false;
  playQueueId: number | null = null;
  entries: Entry[] = [];
  cur: Current | null = null;
  /** Entry order before shuffling, to restore it. */
  unshuffled: number[] | null = null;
  stopAfter = false;
  /** "Stop after this" was turned on by the sleep timer's "end of item". */
  sleepItem = false;
  /** mpv's own `keep-open`, restored when "stop after this" is turned off. */
  keepOpen: unknown = "no";
  playedAny = false;
  /** Volume, speed, chapters and diagnostics, published as they change. */
  tracker!: Tracker;
  /** Answers to background requests, back into the loop. */
  internal!: Channel<Inbox>;
  /** The last item ended with nothing after it. */
  finishedQueue = false;
  /** Items that failed one after the other. */
  failuresInARow = 0;
  /** Playback of this rating key restarted from a local checkpoint at this position. */
  recovered: [string, number] | null = null;
  /** Where the first item was told to start, for a failure before it loaded. */
  initialStart: [string, number] | null = null;
  /** More of the Plex queue is being fetched. */
  extending = false;

  private post(msg: Internal): void {
    this.internal.send({ kind: "internal", msg });
  }

  reports(): boolean {
    return this.kind !== "photo" && !this.live;
  }

  indexOf(uid: number): number {
    return this.entries.findIndex((e) => e.uid === uid);
  }

  curIndex(): number | null {
    if (!this.cur) return null;
    const i = this.indexOf(this.cur.uid);
    return i >= 0 ? i : null;
  }

  hasNext(): boolean {
    const i = this.curIndex();
    return i !== null && i + 1 < this.entries.length;
  }

  /** Load an item into mpv at `index` (the end when `null`) and track it. */
  async insert(itemIn: Metadata, mediaIndex: number, index: number | null, optsIn: OptionList): Promise<number> {
    const item = structuredClone(itemIn);
    Metadata.setServerId(item, this.serverTag);
    const loaded = mediaUrl(this.server, item, mediaIndex, this.quality, this.live);
    const opts = [...optsIn];
    if (!opts.some(([k]) => k === "force-media-title")) opts.push(["force-media-title", Metadata.fullTitle(item)]);
    const at = index !== null && index < this.entries.length ? index : null;
    if (at !== null) await this.mpv.loadfileAt(loaded.url, at, opts);
    else await this.mpv.loadfile(loaded.url, "append", opts);
    const entry: Entry = { uid: uid(), item, mediaIndex, transcodeSession: loaded.transcodeSession };
    if (at !== null) this.entries.splice(at, 0, entry);
    else this.entries.push(entry);
    return entry.uid;
  }

  /**
   * Save the automatic track choice of queued items that will be transcoded, in the background:
   * mpv opens (and so starts transcoding) them only when it gets there.
   */
  saveTracksLater(itemsIn: [Metadata, number][]): void {
    const items = itemsIn.filter(([item]) => !this.live && transcoded(item, this.quality, false));
    if (items.length === 0) return;
    const { scope, server } = this;
    void this.app.spawn(async () => {
      // `for_each_concurrent(4, ..)`
      let next = 0;
      const worker = async () => {
        for (;;) {
          const i = next++;
          const entry = items[i];
          if (!entry) return;
          try {
            await prefs.saveAuto(scope, server, entry[0], entry[1]);
          } catch (e) {
            log.debug(`could not save automatic track choice: ${text(e)}`);
          }
        }
      };
      await Promise.all(Array.from({ length: Math.min(4, items.length) }, worker));
    });
  }

  emitQueue(): void {
    const view: QueueView = {
      items: this.entries.map((e) => ({ uid: e.uid, item: { ...e.item } })),
      current: this.cur ? this.cur.uid : null,
      shuffled: this.unshuffled !== null,
      stopAfter: this.stopAfter,
    };
    this.app.setQueue(view);
  }

  publish(): void {
    const pos = this.curIndex() ?? 0;
    const len = this.entries.length;
    const c = this.cur;
    if (!c) return;
    c.lastUi = now();
    const np: NowPlaying = {
      item: { ...c.item },
      positionMs: c.timeMs,
      durationMs: c.durationMs,
      paused: c.paused,
      transcoding: c.transcodeSession !== null,
      queueLen: len,
      queuePos: pos,
      uid: c.uid,
      ending: c.ending,
      stopAfter: this.stopAfter,
      live: this.live,
      local: false,
    };
    this.app.setNowPlaying(np);
  }

  /** Tell Plex about the current state of an item. Never waits for the answer. */
  report(c: Current, state: TimelineState): void {
    const op = timelineOp(c, state);
    if (op) reporter().send(this.server, op);
  }

  /** Views refetch watch state once Plex has the last reports. */
  refreshAfterReports(): void {
    const app = this.app;
    void app.spawn(async () => {
      await report.flush(20_000);
      app.bump();
    });
  }

  /** Save the local position of the playing item and the queue around it. */
  checkpoint(): void {
    if (!this.reports()) return;
    const scope = this.scope;
    const c = this.cur;
    if (!scope || !c) return;
    c.checkpointAt = now();
    if (c.timeMs > 0 && c.item.ratingKey !== null) {
      try {
        recovery.saveCheckpoint(scope, c.item.ratingKey, c.timeMs, c.durationMs);
      } catch (e) {
        log.debug(`could not save the playback position: ${text(e)}`);
      }
    }
  }

  persistQueue(): void {
    if (!this.reports()) return;
    const scope = this.scope;
    const index = this.curIndex();
    if (!scope || index === null) return;
    const c = this.cur;
    if (!c) return;
    const [range, current] = recovery.window(this.entries.length, index);
    const queue: recovery.SavedQueue = {
      account: scope.account,
      server: scope.server,
      saved_at: offline.now(),
      kind: this.kind,
      items: this.entries.slice(range.start, range.end).map((e) => recovery.slim(e.item)),
      current,
      position_ms: c.timeMs,
      quality: this.quality.id,
    };
    try {
      recovery.saveQueue(scope, queue);
    } catch (e) {
      log.debug(`could not save the queue: ${text(e)}`);
    }
  }

  /** Handle one mpv event; false when playback is over. */
  async onEvent(ev: Event): Promise<boolean> {
    switch (ev.kind) {
      case "shutdown":
        return false;
      case "propertyChange": {
        const { name, data } = ev;
        switch (name) {
          case "playlist-pos":
            await this.onPosition(asI64(data) ?? -1);
            return true;
          case "time-pos":
            if (typeof data === "number") await this.onTime(Math.trunc(data * 1000));
            return true;
          case "duration":
            if (this.cur && typeof data === "number") this.cur.durationMs = Math.trunc(data * 1000);
            return true;
          case "pause": {
            const paused = data === true;
            const c = this.cur;
            if (c && paused !== c.paused) {
              c.paused = paused;
              this.publish();
              if (this.reports() && this.cur) this.report(this.cur, paused ? "paused" : "playing");
              this.checkpoint();
              this.persistQueue();
            }
            return true;
          }
          case "eof-reached": {
            // With keep-open mpv parks on the last frame instead of ending the file.
            const c = this.cur;
            if (data === true && c) {
              c.reachedEnd = true;
              if (this.stopAfter) await this.mpv.quit().catch(() => {});
            }
            return true;
          }
          case "aid":
          case "sid": {
            const c = this.cur;
            if (c && this.settings.sync_streams && c.transcodeSession === null && now() > c.applyingUntil) {
              await syncStreamChoice(this.server, this.mpv, c, name, data);
            }
            return true;
          }
          case "idle-active":
            // Playback is over once mpv has gone idle after playing, or after every item failed.
            if (data === true && (this.playedAny || this.failuresInARow > 0)) {
              await this.mpv.quit().catch(() => {});
            }
            return true;
          default:
            if (Tracker.observes(name)) {
              const urgent = this.tracker.onProperty(name, data);
              this.tracker.publish(this.app, urgent);
            }
            return true;
        }
      }
      case "startFile": {
        // A new file: start its diagnostics over. Repeating one item starts a file without
        // changing the playlist position, so pick the item up here as well.
        if (this.cur === null) {
          const pos = await this.mpv.getProperty("playlist-pos").catch(() => undefined);
          if (pos !== undefined) await this.onPosition(asI64(pos) ?? -1);
        }
        this.beginItemState();
        return true;
      }
      case "fileLoaded": {
        this.playedAny = true;
        this.failuresInARow = 0;
        const loadedKey = this.cur?.item.ratingKey ?? null;
        const failed = failure.current();
        if (failed && failed.item.ratingKey !== loadedKey) {
          // Playback moved on to something else that works.
          failure.clear(this.app);
        }
        const c = this.cur;
        if (c) {
          c.loaded = true;
          if (this.settings.sync_streams && c.transcodeSession === null) {
            if (c.detailPending) {
              c.streamsPending = true;
            } else {
              prefs.applyAuto(this.app, this.scope, this.server, c.item, c.mediaIndex);
              await applyPlexStreams(this.server, this.mpv, c);
            }
          }
        }
        return true;
      }
      case "endFile": {
        // Jumping or skipping moves `playlist-pos` before the old file's
        // end-file arrives; that one ended already, not the new item.
        const curId = this.cur?.mpvId ?? null;
        const other = ev.playlistEntryId !== null && curId !== null && ev.playlistEntryId !== curId;
        const c = this.cur;
        if (!other && c) {
          this.cur = null;
          const completed = ev.reason === "eof";
          c.reachedEnd ||= completed;
          if (completed && !this.hasNextAfter(c.uid) && controls.repeat() === "off") this.finishedQueue = true;
          if (ev.reason === "error") await this.onFailure(c, ev.error);
          this.finish(c, completed);
          if (completed && controls.sleepAtItemEnd()) await this.mpv.quit().catch(() => {});
        }
        return true;
      }
      case "clientMessage": {
        if (ev.args[0] === "patina-skip") {
          const m = this.cur?.activeMarker ?? null;
          if (m) await this.skipMarker(m);
        }
        return true;
      }
      default:
        return true;
    }
  }

  hasNextAfter(uid: number): boolean {
    const i = this.indexOf(uid);
    return i >= 0 && i + 1 < this.entries.length;
  }

  /** The playing item's per-file state for the front ends: chapters from Plex and a recovered position. */
  beginItemState(): void {
    this.tracker.newItem(this.cur ? this.cur.item.Chapter : []);
    const c = this.cur;
    const recovered = c && this.recovered && this.recovered[0] === Metadata.ratingKey(c.item) ? this.recovered[1] : null;
    this.tracker.recoveredFrom(recovered);
    this.tracker.publish(this.app, true);
  }

  /** mpv gave up on an item: tell the front end why, and what can be done. */
  async onFailure(c: Current, error: string | null): Promise<void> {
    this.failuresInARow += 1;
    const index = this.indexOf(c.uid);
    const upcoming: Metadata[] = index >= 0 ? this.entries.slice(index).map((e) => ({ ...e.item })) : [];
    const remaining = Math.max(0, upcoming.length - 1);
    // Where trying again picks up: the playhead, or for a file that never started, where it was meant to start.
    const position =
      c.timeMs > 0 ? c.timeMs : this.initialStart && this.initialStart[0] === Metadata.ratingKey(c.item) ? this.initialStart[1] : 0;
    const request = playRequest({
      item: upcoming[0] ?? { ...c.item },
      selection: upcoming.length > 1 ? upcoming : [],
      resume: true,
      mediaIndex: c.mediaIndex,
      live: this.live,
      quality: this.quality.id,
    });
    failure.record(this.app, {
      item: { ...c.item },
      error,
      kind: null,
      phase: c.loaded ? "playing" : "start",
      positionMs: position,
      mediaIndex: c.mediaIndex,
      quality: this.quality.id,
      remaining,
      request,
      scope: this.origin,
    });
    // Everything failing in turn (or one item failing over and over under repeat-all) ends playback.
    if (this.failuresInARow >= Math.max(this.entries.length, 1)) await this.mpv.quit().catch(() => {});
  }

  async onPosition(pos: number): Promise<void> {
    const uidAt = pos >= 0 ? (this.entries[pos]?.uid ?? null) : null;
    if ((this.cur?.uid ?? null) === uidAt) return;
    const old = this.cur;
    this.cur = null;
    if (old) this.finish(old, false);
    if (this.stopAfter) await this.setStopAfter(false);
    const index = uidAt !== null ? this.indexOf(uidAt) : -1;
    if (index < 0) return;
    const e = this.entries[index]!;
    const c = begin(e.item, e.uid, e.mediaIndex, e.transcodeSession);
    const id = await this.mpv.getProperty(`playlist/${pos}/id`).catch(() => null);
    c.mpvId = asI64(id);
    if (c.detailPending) {
      // Queue entries lack markers and sometimes streams; fetch the full item without holding up playback.
      const { server } = this;
      const key = Metadata.ratingKey(c.item);
      const cuid = c.uid;
      void server
        .metadata(key, MetadataOptions.playback())
        .catch(() => null)
        .then((item) => this.post({ kind: "detail", uid: cuid, item: item ?? null }));
    }
    this.cur = c;
    this.beginItemState();
    this.publish();
    this.emitQueue();
    if (this.reports() && this.cur) this.report(this.cur, "playing");
    this.persistQueue();
    if (index + 1 === this.entries.length) this.extendFromPlex();
  }

  async onTime(t: number): Promise<void> {
    const nextEntry = (() => {
      const i = this.curIndex();
      return i === null ? undefined : this.entries[i + 1];
    })();
    const nextTitle = nextEntry ? Metadata.fullTitle(nextEntry.item) : null;
    const video = this.kind === "video" && !this.live;
    this.tracker.onTime(t);
    // Repeating one item restarts the file without ending it: when the position jumps from
    // near the end back to the start, the pass that just finished counts as watched.
    const before = this.cur;
    const wrapped =
      controls.repeat() === "one" && !this.stopAfter && before !== null && before.durationMs > 0 && before.timeMs >= before.durationMs - 10_000 && t < 15_000;
    if (wrapped && before) {
      this.finish(before, true);
      this.cur = { ...before, timeMs: 0, reachedEnd: false, promptedMarker: null, activeMarker: null, ending: false };
      this.report(this.cur, "playing");
    }
    const c = this.cur;
    if (!c) return;
    c.timeMs = t;
    const inCredits = c.item.Marker.some((m) => m.type === "credits" && m.final && m.startTimeOffset !== null && m.startTimeOffset <= t);
    const nearEnd = c.durationMs > 0 && c.durationMs - t < 30_000;
    const ending = nextTitle !== null && video && (inCredits || nearEnd);
    const changed = ending !== c.ending;
    c.ending = ending;
    const checkpointDue = now() - c.checkpointAt >= 5000;
    if (changed && !this.stopAfter && ending && nextTitle !== null) {
      await this.mpv.showText(`Up next: ${nextTitle}`, 4000).catch(() => {});
    }
    await this.handleMarkers();
    const stale = this.cur !== null && now() - this.cur.lastUi >= 900;
    if (changed || stale) this.publish();
    if (checkpointDue) this.checkpoint();
  }

  async tick(): Promise<void> {
    const c = this.cur;
    if (!c) return;
    if (this.reports()) this.report(c, c.paused ? "paused" : "playing");
    if (c.transcodeSession !== null) reporter().send(this.server, { kind: "pingTranscode", session: c.transcodeSession });
    this.persistQueue();
  }

  /** An item ended (or was left). Never waits for Plex: the reports go to the worker. */
  finish(c: Current, completed: boolean): void {
    if (!this.reports()) return;
    const progress = c.durationMs > 0 ? c.timeMs / c.durationMs : 0;
    const watched = completed || c.reachedEnd || progress >= this.settings.watched_threshold;
    const timeMs = watched && c.durationMs > 0 ? c.durationMs : c.timeMs;
    const op = timelineAt(c, "stopped", timeMs);
    if (op) reporter().send(this.server, op);
    if (watched && c.item.ratingKey !== null) reporter().send(this.server, { kind: "scrobble", ratingKey: c.item.ratingKey });
    if (this.scope && c.item.ratingKey !== null) {
      try {
        if (watched) recovery.clearCheckpoint(this.scope, c.item.ratingKey);
        else recovery.saveCheckpoint(this.scope, c.item.ratingKey, c.timeMs, c.durationMs);
      } catch (e) {
        log.debug(`could not update the playback position: ${text(e)}`);
      }
    }
    this.refreshAfterReports();
  }

  async handleMarkers(): Promise<void> {
    const settings = this.settings;
    const c = this.cur;
    if (!c) return;
    const t = c.timeMs;
    const marker =
      c.item.Marker.find(
        (m) =>
          (m.type === "intro" || m.type === "credits") &&
          (m.startTimeOffset ?? Number.MAX_SAFE_INTEGER) <= t &&
          t < (m.endTimeOffset ?? 0) - 1000,
      ) ?? null;
    c.activeMarker = marker;
    if (!marker) return;
    if (c.promptedMarker === marker.id) return;
    c.promptedMarker = marker.id;
    const isIntro = marker.type === "intro";
    const auto = isIntro ? settings.auto_skip_intro : settings.auto_skip_credits;
    if (auto) {
      await this.skipMarker(marker);
    } else {
      const what = isIntro ? "intro" : "credits";
      const key = settings.skip_key.trim();
      await this.mpv.showText(`Press ${key} to skip ${what}`, 5000).catch(() => {});
    }
  }

  async skipMarker(m: Marker): Promise<void> {
    const isCredits = m.type === "credits";
    const hasNext = this.hasNext();
    const c = this.cur;
    if (!c) return;
    c.activeMarker = null;
    if (isCredits && m.final && (hasNext || this.stopAfter)) {
      c.reachedEnd = true;
      if (this.stopAfter) {
        await this.mpv.quit().catch(() => {});
      } else {
        await this.mpv.showText("Next episode", 1500).catch(() => {});
        await this.mpv.playlistNext().catch(() => {});
      }
      return;
    }
    if (m.endTimeOffset !== null) {
      await this.mpv.seekAbsolute(m.endTimeOffset / 1000).catch(() => {});
      await this.mpv.showText(isCredits ? "Skipped credits" : "Skipped intro", 1500).catch(() => {});
    }
  }

  async setStopAfter(on: boolean): Promise<void> {
    this.stopAfter = on;
    controls.setStopAfter(on);
    if (!on) this.sleepItem = false;
    const value = on ? "always" : this.keepOpen;
    await this.mpv.setProperty("keep-open", value).catch(() => {});
    // Stopping after this beats repeating it.
    await controls.applyRepeat(this.mpv, controls.repeat(), on);
    if (on) await this.mpv.showText("Stopping after this", 1500).catch(() => {});
    this.publish();
    this.tracker.publish(this.app, true);
  }

  /** Big and endless queues (radio) arrive in windows; fetch what follows. */
  extendFromPlex(): void {
    const id = this.playQueueId;
    if (id === null) return;
    if (this.extending) return;
    this.extending = true;
    void this.server
      .playQueue(id, 50)
      .then((mc) => mc.Metadata)
      .catch((): Metadata[] => [])
      .then((items) => this.post({ kind: "upcoming", items }));
  }

  async onInternal(msg: Internal): Promise<void> {
    switch (msg.kind) {
      case "detail": {
        const c = this.cur;
        if (!c || c.uid !== msg.uid) return;
        c.detailPending = false;
        const full = msg.item;
        if (full) {
          full.playQueueItemID = c.item.playQueueItemID;
          // The server's answer carries no Patina server tag; keep the one the entry had.
          Metadata.setServerId(full, this.serverTag);
          if (c.durationMs === 0) c.durationMs = full.duration ?? 0;
          this.tracker.setPlexChapters(full.Chapter);
          c.item = full;
        }
        if (c.streamsPending && c.loaded) {
          c.streamsPending = false;
          prefs.applyAuto(this.app, this.scope, this.server, c.item, c.mediaIndex);
          await applyPlexStreams(this.server, this.mpv, c);
        }
        this.publish();
        return;
      }
      case "leaves": {
        let added: Error | null = null;
        if ("ok" in msg.result) {
          try {
            await this.addItems(msg.source, msg.result.ok, msg.next);
          } catch (e) {
            added = e instanceof Error ? e : new Error(String(e));
          }
        } else {
          added = new Error(msg.result.err);
        }
        if (added) this.app.error(`Could not change the queue: ${added.message}`);
        this.emitQueue();
        this.persistQueue();
        return;
      }
      case "added": {
        const mc = msg.queue;
        if (!mc) return;
        const known = this.entries.map((e) => e.item.playQueueItemID).filter((x): x is number => x !== null);
        for (const u of msg.uids) {
          const i = this.indexOf(u);
          if (i < 0) continue;
          const rk = this.entries[i]!.item.ratingKey;
          const found = mc.Metadata.find((m) => m.ratingKey === rk && m.playQueueItemID !== null && !known.includes(m.playQueueItemID));
          if (found && found.playQueueItemID !== null) {
            this.entries[i]!.item.playQueueItemID = found.playQueueItemID;
            known.push(found.playQueueItemID);
          }
        }
        return;
      }
      case "upcoming": {
        this.extending = false;
        const known = this.entries.map((e) => e.item.playQueueItemID).filter((x): x is number => x !== null);
        const last = known[known.length - 1];
        if (last === undefined) return;
        const after = msg.items.findIndex((m) => m.playQueueItemID === last);
        if (after < 0) return;
        const fresh = msg.items.slice(after + 1).filter((m) => m.playQueueItemID !== null && !known.includes(m.playQueueItemID));
        const later: [Metadata, number][] = [];
        for (const item of fresh) {
          const mediaIndex = prefs.mediaIndex(this.scope, item, null);
          later.push([item, mediaIndex]);
          try {
            await this.insert(item, mediaIndex, null, []);
          } catch (e) {
            log.debug(`could not extend queue: ${text(e)}`);
          }
        }
        this.saveTracksLater(later);
        this.emitQueue();
        return;
      }
    }
  }

  async onCommand(cmd: Cmd): Promise<void> {
    switch (cmd.kind) {
      case "queue":
        try {
          await this.onQueueCommand(cmd.cmd);
        } catch (e) {
          this.app.error(`Could not change the queue: ${text(e)}`);
        }
        break;
      case "tune": {
        try {
          await controls.apply(this.app, this.mpv, cmd.cmd);
        } catch (e) {
          this.app.error(text(e));
        }
        // The sleep timer's "end of item" is "stop after this".
        const atEnd = controls.sleepAtItemEnd();
        if (atEnd && !this.stopAfter) await this.setStopAfter(true);
        else if (!atEnd && this.sleepItem && this.stopAfter) await this.setStopAfter(false);
        this.sleepItem = atEnd;
        break;
      }
    }
    this.tracker.publish(this.app, true);
  }

  async onQueueCommand(cmd: QueueCommand): Promise<void> {
    switch (cmd.kind) {
      case "add": {
        // Tags are relative to the connected server, which may have changed since the
        // player started: compare where the item and the player actually are.
        const item = cmd.item;
        if (!Scope.eq(prefs.scopeFor(this.app, item), this.scope)) {
          throw new Error(`A queue plays from one server; “${Metadata.title(item)}” is on another.`);
        }
        if (Metadata.isPlayable(item)) {
          await this.addItems(item, [item], cmd.next);
        } else {
          // A container's items come from the server; wait for them off the loop.
          const { server } = this;
          void leaves(server, item).then(
            (ok) => this.post({ kind: "leaves", source: item, next: cmd.next, result: { ok } }),
            (e) => this.post({ kind: "leaves", source: item, next: cmd.next, result: { err: text(e) } }),
          );
        }
        return;
      }
      case "remove": {
        const i = this.indexOf(cmd.uid);
        if (i < 0) return;
        await this.mpv.playlistRemove(i);
        const [e] = this.entries.splice(i, 1);
        if (!e) return;
        if (e.transcodeSession !== null) reporter().send(this.server, { kind: "stopTranscode", session: e.transcodeSession });
        if (this.playQueueId !== null && e.item.playQueueItemID !== null) {
          reporter().send(this.server, { kind: "playQueue", op: { kind: "remove", queue: this.playQueueId, item: e.item.playQueueItemID } });
        }
        if (this.unshuffled) this.unshuffled = this.unshuffled.filter((u) => u !== cmd.uid);
        return;
      }
      case "move": {
        const from = this.indexOf(cmd.uid);
        if (from < 0) return;
        const beforeIndex = cmd.before != null ? this.indexOf(cmd.before) : -1;
        const to = beforeIndex >= 0 ? beforeIndex : this.entries.length;
        await this.moveEntry(from, to);
        const nowAt = Math.max(0, this.indexOf(cmd.uid));
        const after = nowAt > 0 ? this.entries[nowAt - 1]!.item.playQueueItemID : null;
        const item = this.entries[nowAt]?.item.playQueueItemID ?? null;
        if (this.playQueueId !== null && item !== null) {
          const op: QueueOp = { kind: "move", queue: this.playQueueId, item, after };
          reporter().send(this.server, { kind: "playQueue", op });
        }
        return;
      }
      case "jump": {
        const i = this.indexOf(cmd.uid);
        if (i >= 0) await this.mpv.setProperty("playlist-pos", i);
        return;
      }
      case "shuffle":
        await this.shuffle(cmd.on);
        return;
      case "stopAfter":
        await this.setStopAfter(cmd.on);
        return;
    }
  }

  /** Queue `items` (what `source` stands for) in mpv, and mirror them to the Plex queue in the background. */
  async addItems(source: Metadata, items: Metadata[], next: boolean): Promise<void> {
    if (items.length === 0) throw new Error(`nothing playable in “${Metadata.title(source)}”`);
    const ci = this.curIndex();
    const first = next && ci !== null ? ci + 1 : null;
    const added: number[] = [];
    const later: [Metadata, number][] = [];
    let n = 0;
    for (const it of items) {
      const mediaIndex = prefs.mediaIndex(this.scope, it, null);
      later.push([it, mediaIndex]);
      try {
        added.push(await this.insert(it, mediaIndex, first !== null ? first + n : null, []));
      } catch (e) {
        log.warn(`could not queue item: ${text(e)}`);
      }
      n += 1;
    }
    this.saveTracksLater(later);
    // Keep the Plex queue in step so other apps (and later edits) agree; its answer
    // tells us the new entries' ids, which arrives as an `added` message.
    const id = this.playQueueId;
    if (id !== null) {
      const uri =
        Metadata.kind(source) === "playlist"
          ? this.server.libraryUri(`/playlists/${Metadata.ratingKey(source)}/items`)
          : this.server.itemUri([Metadata.ratingKey(source)]);
      const { server } = this;
      const uids = [...added];
      void server
        .addToPlayQueue(id, uri, next)
        .catch((): MediaContainer | null => null)
        .then((queue) => this.post({ kind: "added", uids, queue }));
    }
    if (this.unshuffled) this.unshuffled.push(...added);
    const what = added.length === 1 ? "1 item" : `${added.length} items`;
    this.app.toast(next ? `Playing ${what} next` : `Added ${what} to the queue`);
  }

  /** Move entry `from` before entry `to` (or to the end), in mpv and locally. */
  async moveEntry(from: number, to: number): Promise<void> {
    if (from === to || from + 1 === to) return;
    await this.mpv.playlistMove(from, to);
    const [e] = this.entries.splice(from, 1);
    if (!e) return;
    const dest = from < to ? to - 1 : to;
    this.entries.splice(Math.min(dest, this.entries.length), 0, e);
  }

  /** Put the entries after the current one into `order`. */
  async reorderUpcoming(order: number[]): Promise<void> {
    const ci = this.curIndex();
    const base = ci !== null ? ci + 1 : 0;
    for (let k = 0; k < order.length; k++) {
      const i = this.indexOf(order[k]!);
      if (i < 0) continue;
      if (i >= base && i !== base + k) await this.moveEntry(i, base + k);
    }
  }

  async shuffle(on: boolean): Promise<void> {
    const ci = this.curIndex();
    const base = ci !== null ? ci + 1 : 0;
    if (on) {
      this.unshuffled ??= this.entries.map((e) => e.uid);
      const upcoming = this.entries.slice(Math.min(base, this.entries.length)).map((e) => e.uid);
      // Fisher-Yates
      for (let i = upcoming.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1));
        [upcoming[i], upcoming[j]] = [upcoming[j]!, upcoming[i]!];
      }
      await this.reorderUpcoming(upcoming);
      if (this.playQueueId !== null) reporter().send(this.server, { kind: "playQueue", op: { kind: "shuffle", queue: this.playQueueId } });
    } else if (this.unshuffled) {
      const order = this.unshuffled;
      this.unshuffled = null;
      const upcoming = this.entries.slice(Math.min(base, this.entries.length)).map((e) => e.uid);
      const restored = order.filter((u) => upcoming.includes(u));
      await this.reorderUpcoming(restored);
      if (this.playQueueId !== null) reporter().send(this.server, { kind: "playQueue", op: { kind: "unshuffle", queue: this.playQueueId } });
    }
  }
}

function asI64(v: unknown): number | null {
  return typeof v === "number" && Number.isInteger(v) ? v : null;
}

/** The state of an item as it starts: from the queue entry alone, since the full item is fetched afterwards. */
function begin(item: Metadata, uid: number, mediaIndex: number, transcodeSession: string | null): Current {
  return {
    uid,
    durationMs: item.duration ?? 0,
    detailPending: item.ratingKey !== null && Metadata.kind(item) !== "photo",
    item: structuredClone(item),
    mediaIndex,
    timeMs: 0,
    paused: false,
    reachedEnd: false,
    transcodeSession,
    applyingUntil: now() + 3000,
    sidecars: new Map(),
    activeMarker: null,
    promptedMarker: null,
    ending: false,
    lastUi: now() - 5000,
    mpvId: null,
    loaded: false,
    streamsPending: false,
    checkpointAt: now(),
  };
}

function timelineOp(c: Current, state: TimelineState): report.Op | null {
  return timelineAt(c, state, c.timeMs);
}

function timelineAt(c: Current, state: TimelineState, timeMs: number): report.Op | null {
  const rk = c.item.ratingKey;
  const key = c.item.key;
  if (rk === null || key === null) return null;
  return {
    kind: "timeline",
    update: {
      ratingKey: rk,
      key,
      state,
      timeMs,
      durationMs: c.durationMs,
      playQueueItemId: c.item.playQueueItemID,
      session: c.transcodeSession,
    },
  };
}

type Track = Record<string, unknown>;

function tracks(list: unknown): Track[] {
  return Array.isArray(list) ? list.filter((t): t is Track => typeof t === "object" && t !== null) : [];
}

/** Select mpv tracks matching the streams chosen in Plex, and add sidecar subtitles. */
async function applyPlexStreams(server: PlexServer, mpv: Mpv, c: Current): Promise<void> {
  c.applyingUntil = now() + 3000;
  const part = partOf(c.item, c.mediaIndex);
  if (!part) return;
  let list: unknown;
  try {
    list = await mpv.getProperty("track-list");
  } catch {
    return;
  }
  const find = (kind: string, ffIndex: number): number | null => {
    const t = tracks(list).find((t) => t.type === kind && asI64(t["ff-index"]) === ffIndex && t.external !== true);
    return t ? asI64(t.id) : null;
  };

  const audio = Part.audioStreams(part).find((s) => s.selected);
  if (audio && audio.index !== null) {
    const id = find("audio", audio.index);
    if (id !== null) await mpv.setProperty("aid", id).catch(() => {});
  }

  const selectedSub = Part.subtitleStreams(part).find((s) => s.selected) ?? null;
  for (const s of Part.subtitleStreams(part).filter((s) => Stream.isExternal(s))) {
    if (s.key === null || s.id === null) continue;
    let url: string;
    try {
      url = server.url(s.key).toString();
    } catch {
      continue;
    }
    const flag = selectedSub !== null && selectedSub.id === s.id ? "select" : "auto";
    const lang = s.languageCode ?? "";
    try {
      await mpv.subAdd(url, flag, Stream.shortLabel(s), lang);
      c.sidecars.set(url, s.id);
    } catch {
      // not added
    }
  }
  if (selectedSub && !Stream.isExternal(selectedSub)) {
    if (selectedSub.index !== null) {
      const id = find("sub", selectedSub.index);
      if (id !== null) await mpv.setProperty("sid", id).catch(() => {});
    }
  } else if (!selectedSub) {
    await mpv.setProperty("sid", "no").catch(() => {});
  }
  c.applyingUntil = now() + 1500;
}

/** Save a track choice made in mpv back to Plex so other apps agree. */
async function syncStreamChoice(server: PlexServer, mpv: Mpv, c: Current, prop: string, data: unknown): Promise<void> {
  const part = partOf(c.item, c.mediaIndex);
  if (!part) return;
  const partId = part.id;
  if (partId === null) return;
  const disabled = data === false || data === "no";
  let streamId: number | null;
  if (disabled) {
    streamId = prop === "sid" ? 0 : null;
  } else {
    const trackId = asI64(data);
    if (trackId === null) return;
    let list: unknown;
    try {
      list = await mpv.getProperty("track-list");
    } catch {
      return;
    }
    const kind = prop === "aid" ? "audio" : "sub";
    const track = tracks(list).find((t) => t.type === kind && asI64(t.id) === trackId);
    if (!track) return;
    if (track.external === true) {
      const file = track["external-filename"];
      streamId = typeof file === "string" ? (c.sidecars.get(file) ?? null) : null;
    } else {
      const wanted = prop === "aid" ? streamType.AUDIO : streamType.SUBTITLE;
      const ff = asI64(track["ff-index"]);
      streamId = part.Stream.find((s) => s.streamType === wanted && s.index === ff && ff !== null)?.id ?? null;
    }
  }
  if (streamId === null) return;
  const [audio, subtitle] = prop === "aid" ? [streamId, null] : [null, streamId];
  reporter().send(server, { kind: "streams", partId, audio, subtitle });
}

async function spawnMpv(settings: PlayerSettings, headers: [string, string][]): Promise<[Mpv, Channel<Event>]> {
  const previous = ACTIVE;
  ACTIVE = null;
  if (previous) {
    await previous.quit().catch(() => {});
    await previous.wait(3000);
  }
  let spawned: [Mpv, Channel<Event>];
  try {
    spawned = await Mpv.spawn({ ...defaultSpawnOptions(), program: mpvProgram(settings), extraArgs: splitArgs(settings.extra_args), httpHeaders: headers });
  } catch (e) {
    throw context(e, "starting mpv (set its path in Settings → Player if it is not on PATH)");
  }
  ACTIVE = spawned[0];
  return spawned;
}

/** Forget the player if it is still the active one; true when it was. */
function release(mpv: Mpv): boolean {
  if (ACTIVE === mpv) {
    ACTIVE = null;
    return true;
  }
  return false;
}

/** Feed mpv's events into the loop's inbox; ends with `eventsEnded`. */
function pumpEvents(events: Channel<Event>, inbox: Channel<Inbox>): void {
  void (async () => {
    for (;;) {
      const event = await events.recv();
      if (event === null) break;
      inbox.send({ kind: "event", event });
    }
    inbox.send({ kind: "eventsEnded" });
  })();
}

/** One message of the playback loop; true when playback is over. */
async function handle(p: Player, msg: Inbox, flags: { tickDone: () => void; stateTickDone: () => void }): Promise<boolean> {
  switch (msg.kind) {
    case "event":
      return !(await p.onEvent(msg.event));
    case "eventsEnded":
      return true;
    case "cmd":
      await p.onCommand(msg.cmd);
      p.emitQueue();
      p.persistQueue();
      return false;
    case "internal":
      await p.onInternal(msg.msg);
      return false;
    case "tick":
      flags.tickDone();
      await p.tick();
      return false;
    case "stateTick":
      flags.stateTickDone();
      p.tracker.publish(p.app, false);
      return false;
  }
}


async function run(app: AppState, req: PlayRequest, origin: Scope | null): Promise<void> {
  RUNS += 1;
  try {
    await runInner(app, req, origin);
  } finally {
    RUNS -= 1;
  }
}

async function runInner(app: AppState, req: PlayRequest, origin: Scope | null): Promise<void> {
  const session = app.session();
  if (!session) throw new Error("Not connected to a server.");
  // An item found on another server of the account plays, and reports, through that server.
  const server = await servers.serverOf(app, req.item);
  const tagId = Metadata.serverId(req.item);
  const serverTag = tagId !== null && !servers.isCurrent(app, tagId) ? tagId : null;
  // Checkpoints and saved queues are kept per server, so the other server's rating keys never mix with ours.
  const scope = Scope.of(session);
  if (serverTag !== null) scope.server = serverTag;
  const settings = app.settings().player;
  const forced = START_OFFSET;
  START_OFFSET = null;
  const queue = await buildQueue(server, req, settings);

  const info = server.clientInfo();
  const [mpv, events] = await spawnMpv(settings, [
    ["X-Plex-Token", server.token()],
    ["X-Plex-Client-Identifier", info.identifier],
    ["X-Plex-Product", info.product],
  ]);
  const inbox = new Channel<Inbox>();
  COMMANDS = inbox;

  for (let i = 0; i < PLAYER_PROPERTIES.length; i++) await mpv.observeProperty(i + 1, PLAYER_PROPERTIES[i]!);
  await Tracker.observe(mpv);
  await controls.applySaved(mpv);
  if (settings.skip_key.trim() !== "") await mpv.bindMessage(settings.skip_key.trim(), "patina-skip").catch(() => {});
  const keepOpen = await mpv.getProperty("keep-open").catch(() => "no");
  if (settings.media_controls) {
    // Patina publishes Now Playing itself; mpv's own entry would compete (older mpv lacks the property).
    await mpv.setProperty("media-controls", "no").catch(() => {});
  }

  const p = new Player();
  p.app = app;
  p.server = server;
  p.serverTag = serverTag;
  p.scope = scope;
  p.origin = origin;
  p.settings = settings;
  p.quality = (req.quality !== null ? qualityByName(req.quality) : undefined) ?? playerQuality(settings);
  p.mpv = mpv;
  p.kind = queue.kind;
  p.live = req.live;
  p.playQueueId = queue.id;
  p.keepOpen = keepOpen;
  p.tracker = new Tracker(controls.connectionInfoOf(session, server, scope.server));
  p.internal = inbox;

  // Resolve stream URLs up front; items that cannot be played are dropped so
  // mpv playlist positions line up with `entries`.
  let start = 0;
  const later: [Metadata, number][] = [];
  for (let i = 0; i < queue.items.length; i++) {
    const item = queue.items[i]!;
    const mediaIndex = prefs.mediaIndex(scope, item, i === queue.start ? req.mediaIndex : null);
    if (!p.live && transcoded(item, p.quality, false)) {
      if (i === queue.start) {
        // The transcoder starts as soon as mpv opens the file.
        try {
          await prefs.saveAuto(scope, server, item, mediaIndex);
        } catch (e) {
          log.debug(`could not save automatic track choice: ${text(e)}`);
        }
      } else {
        later.push([item, mediaIndex]);
      }
    }
    const opts: OptionList = [["force-media-title", Metadata.fullTitle(item)]];
    if (i === queue.start) p.initialStart = [Metadata.ratingKey(item), 0];
    if (i === queue.start && req.resume) {
      // A position asked for, else a local checkpoint newer than what the server knows (a
      // connection that dropped before it was told), else the server's.
      const asked = forced && forced[0] === Metadata.ratingKey(item) ? forced[1] : null;
      const local = recovery.resumePosition(scope, item);
      const chosen = asked ?? local ?? item.viewOffset;
      if (chosen !== null && chosen > 0) {
        p.initialStart = [Metadata.ratingKey(item), chosen];
        opts.push(["start", (chosen / 1000).toFixed(3)]);
        const own = asked ?? local;
        if (own !== null && (item.viewOffset === null || Math.abs(own - item.viewOffset) > 2000)) {
          p.recovered = [Metadata.ratingKey(item), chosen];
        }
      }
    }
    if (i === queue.start) start = p.entries.length;
    try {
      await p.insert(item, mediaIndex, null, opts);
    } catch (e) {
      log.warn(`skipping queue item: ${text(e)}`);
    }
  }
  if (p.entries.length === 0) {
    await mpv.quit().catch(() => {});
    throw new Error("nothing in the queue could be played");
  }
  p.saveTracksLater(later);
  await mpv.setProperty("playlist-pos", start);
  p.emitQueue();

  pumpEvents(events, inbox);
  // `ticker.tick().await` consumed the first tick of the 10s timer; `setInterval` waits a period anyway.
  let tickQueued = false;
  let stateTickQueued = false;
  const ticker = setInterval(() => {
    if (!tickQueued) {
      tickQueued = true;
      inbox.send({ kind: "tick" });
    }
  }, 10_000);
  const stateTicker = setInterval(() => {
    if (!stateTickQueued) {
      stateTickQueued = true;
      inbox.send({ kind: "stateTick" });
    }
  }, 1000);

  try {
    for (;;) {
      const msg = await inbox.recv();
      if (msg === null) break;
      let stop = false;
      try {
        stop = await handle(p, msg, { tickDone: () => (tickQueued = false), stateTickDone: () => (stateTickQueued = false) });
      } catch (e) {
        // A handler failing must not end playback (in Rust these paths cannot fail).
        log.warn(`playback loop: ${text(e)}`);
      }
      if (stop) break;
    }
  } finally {
    clearInterval(ticker);
    clearInterval(stateTicker);
  }

  try {
    // A queue played to its end is done; anything else can be offered again after a restart.
    const atEnd = p.finishedQueue || (p.cur !== null && p.cur.reachedEnd && !p.hasNext());
    if (atEnd && controls.repeat() === "off") {
      try {
        recovery.clearQueue(scope);
      } catch {
        // `let _ =`
      }
    } else {
      p.persistQueue();
    }
    const last = p.cur;
    p.cur = null;
    if (last) p.finish(last, last.reachedEnd);
    for (const e of p.entries) {
      if (e.transcodeSession !== null) reporter().send(server, { kind: "stopTranscode", session: e.transcodeSession });
    }
    const ours = release(mpv);
    await mpv.wait(2000);
    if (ours && ACTIVE === null) {
      COMMANDS = null;
      app.setNowPlaying(null);
      app.setQueue(null);
      controls.finished(app);
    }
    p.refreshAfterReports();
  } finally {
    // No more commands reach this loop (`rx` is dropped when `run` returns, in Rust).
    inbox.close();
  }
}

// Offline playback -----------------------------------------------------------

/** Play downloaded files with local checkpoints; the outbox reconciles them when connected. */
export function playFiles(app: AppState, files: Download[]): void {
  void app.spawn(async () => {
    try {
      await runFiles(app, files);
    } catch (e) {
      app.error(`Playback failed: ${text(e)}`);
      app.setNowPlaying(null);
    }
  });
}

async function runFiles(app: AppState, files: Download[]): Promise<void> {
  const settings = app.settings().player;
  const [mpv, events] = await spawnMpv(settings, []);
  COMMANDS = null;
  const props = ["playlist-pos", "time-pos", "pause", "duration", "idle-active"];
  for (let i = 0; i < props.length; i++) await mpv.observeProperty(i + 1, props[i]!);
  await Tracker.observe(mpv);
  await controls.applySaved(mpv);
  const tracker = new Tracker(controls.offlineConnection());
  for (const download of files) {
    if (download.file === null) throw new Error("Downloaded file is missing");
    await mpv.loadfile(download.file, "append", [
      ["force-media-title", Metadata.fullTitle(download.item)],
      ["start", (offline.position(download) / 1000).toFixed(3)],
    ]);
  }
  await mpv.setProperty("playlist-pos", 0);

  let np: NowPlaying | null = null;
  let lastUi = now();
  let playedAny = false;
  let checkpoint = now();
  let currentEntryId: number | null = null;
  const save = (n: NowPlaying | null, completed: boolean) => {
    const d = n ? files[n.queuePos] : undefined;
    if (n && d) {
      try {
        offline.record(d, n.positionMs, n.durationMs, completed, settings.watched_threshold);
      } catch (e) {
        app.error(`Could not save offline progress: ${text(e)}`);
      }
    }
  };
  for (;;) {
    const ev = await events.recv();
    if (ev === null) break;
    if (ev.kind === "shutdown") {
      save(np, false);
      break;
    }
    switch (ev.kind) {
      case "fileLoaded":
        playedAny = true;
        break;
      case "startFile":
        tracker.newItem([]);
        tracker.publish(app, true);
        break;
      case "endFile": {
        // A skip can publish the new playlist position before the old entry ends.
        const stale = ev.playlistEntryId !== null && currentEntryId !== null && ev.playlistEntryId !== currentEntryId;
        if (!stale) {
          save(np, ev.reason === "eof");
          np = null;
          currentEntryId = null;
          if (ev.reason === "eof" && controls.sleepAtItemEnd()) await mpv.quit().catch(() => {});
        }
        break;
      }
      case "propertyChange": {
        const { name, data } = ev;
        switch (name) {
          case "playlist-pos": {
            save(np, false);
            const pos = asI64(data) ?? -1;
            currentEntryId = pos >= 0 ? asI64(await mpv.getProperty(`playlist/${pos}/id`).catch(() => null)) : null;
            const d = pos >= 0 ? files[pos] : undefined;
            np = d
              ? {
                  item: structuredClone(d.item),
                  positionMs: offline.position(d),
                  durationMs: d.item.duration ?? 0,
                  paused: false,
                  transcoding: false,
                  queueLen: files.length,
                  queuePos: pos,
                  uid: 0,
                  ending: false,
                  stopAfter: false,
                  live: false,
                  local: true,
                }
              : null;
            break;
          }
          case "time-pos":
            if (np && typeof data === "number") {
              np.positionMs = Math.trunc(data * 1000);
              tracker.onTime(np.positionMs);
            }
            tracker.publish(app, false);
            if (now() - lastUi < 900) continue;
            break;
          case "duration":
            if (np && typeof data === "number") np.durationMs = Math.trunc(data * 1000);
            break;
          case "pause":
            if (np) np.paused = data === true;
            break;
          case "idle-active":
            if (data === true && playedAny) await mpv.quit().catch(() => {});
            continue;
          default:
            if (Tracker.observes(name)) {
              const urgent = tracker.onProperty(name, data);
              tracker.publish(app, urgent);
            }
            continue;
        }
        if (now() - checkpoint >= 5000 || name === "pause") {
          save(np, false);
          checkpoint = now();
        }
        lastUi = now();
        app.setNowPlaying(np ? { ...np, item: { ...np.item } } : null);
        break;
      }
      default:
        break;
    }
  }
  save(np, false);
  const ours = release(mpv);
  await mpv.wait(2000);
  if (ours && ACTIVE === null) {
    app.setNowPlaying(null);
    controls.finished(app);
  }
}
