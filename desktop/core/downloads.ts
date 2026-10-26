// Durable downloads shared by both front ends. A process lease owns the worker;
// manifest transactions coordinate pause, remove and progress across processes.
// Port of crates/core/src/downloads.rs.

import { closeSync, copyFileSync, existsSync, fsyncSync, mkdirSync, openSync, renameSync, rmSync, statSync, statfsSync, writeSync } from "node:fs";
import { dirname, join } from "node:path";

import { Metadata, PlexError, type PlexServer, normalizeMetadata } from "../plex";
import { finishResponse } from "../plex/client";
import * as config from "./config";
import * as images from "./images";
import { logger } from "./log";
import * as offline from "./offline";
import { errorText } from "./ops/util";
import type { AppState, Session } from "./state";
import * as storage from "./storage";

const log = logger("downloads");

export type State = "queued" | "downloading" | "paused" | "done" | "failed";

export interface Download {
  id: string;
  serverId: string;
  serverName: string;
  item: Metadata;
  file: string | null;
  poster: string | null;
  size: number;
  received: number;
  state: State;
  error: string | null;
  addedAt: number;
  /** Account that queued this file. Legacy downloads remain playable but never sync to an inferred user. */
  account: string;
  validator: string | null;
  sourceKey: string | null;
  /**
   * Smart-download policy that fetched this file and may remove it again
   * (see smart.ts); `null` for downloads someone asked for by hand.
   */
  policy: string | null;
}

const STATES: readonly string[] = ["queued", "downloading", "paused", "done", "failed"];

const str = (v: unknown): string | null => (typeof v === "string" ? v : null);
const int = (v: unknown): number => (typeof v === "number" && Number.isFinite(v) ? v : 0);

/** A `Download` as serde would read it: `#[serde(default)]` fields filled, the item normalised. */
export function normalizeDownload(v: unknown): Download {
  const o = (typeof v === "object" && v !== null ? v : {}) as Record<string, unknown>;
  return {
    id: String(o.id ?? ""),
    serverId: String(o.serverId ?? ""),
    serverName: String(o.serverName ?? ""),
    item: normalizeMetadata(o.item ?? {}),
    file: str(o.file),
    poster: str(o.poster),
    size: int(o.size),
    received: int(o.received),
    state: STATES.includes(o.state as string) ? (o.state as State) : "failed",
    error: str(o.error),
    addedAt: int(o.addedAt),
    account: str(o.account) ?? "",
    validator: str(o.validator),
    sourceKey: str(o.sourceKey),
    policy: str(o.policy),
  };
}

let running = false;

function manifest(): string {
  return join(config.configDir(), "downloads.json");
}

function artworkDir(): string {
  return join(config.dataDir(), "artwork");
}

function update<R>(f: (list: Download[]) => R): R {
  return storage.updateJson<Download[], R>(manifest(), () => [], f);
}

/** The manifest as stored, without normalising items (cheap enough to poll). */
function readRaw(): Download[] {
  const raw = storage.readJson<unknown>(manifest(), () => []);
  return Array.isArray(raw) ? (raw as Download[]) : [];
}

export function list(): Download[] {
  try {
    return readRaw().map(normalizeDownload);
  } catch (e) {
    log.error(`downloads: ${errorText(e)}`);
    return [];
  }
}

function entry(id: string): Download | undefined {
  try {
    return readRaw().find((d) => d.id === id);
  } catch (e) {
    log.error(`downloads: ${errorText(e)}`);
    return undefined;
  }
}

function emit(app: AppState): void {
  app.emit({ kind: "downloads", downloads: list() });
}

function change(app: AppState, f: (list: Download[]) => void): void {
  try {
    update(f);
  } catch (e) {
    app.error(`Could not save downloads: ${errorText(e)}`);
  }
  emit(app);
}

export function poster(id: string): string | null {
  return entry(id)?.poster ?? null;
}

/** Identity of a download: one file per server, account and item. */
export function idFor(serverId: string, account: string, ratingKey: string): string {
  return `${serverId}:${account}:${ratingKey}`;
}

export async function enqueue(app: AppState, ratingKey: string): Promise<number> {
  const session = app.session();
  if (!session) throw new Error("Not connected.");
  const server = session.server;
  const item = await server.metadata(ratingKey);
  if (!item) throw new Error("Item not found.");
  const items = Metadata.isPlayable(item) ? [item] : (await server.allLeaves(ratingKey)).Metadata;
  return add(app, session, items, null);
}

/**
 * Queue the playable items for the session's server and account; those
 * already downloaded are skipped. `policy` marks them as managed by a smart
 * download policy.
 */
export function add(app: AppState, session: Session, items: Metadata[], policy: string | null = null): number {
  const account = offline.account(session);
  const added = update((list) => {
    let added = 0;
    for (const item of items.filter((m) => Metadata.isPlayable(m) && m.Media.length > 0)) {
      const id = idFor(session.serverId, account, Metadata.ratingKey(item));
      if (list.some((d) => d.id === id)) continue;
      list.push({
        id,
        serverId: session.serverId,
        serverName: session.serverName,
        size: item.Media[0]?.Part[0]?.size ?? 0,
        item,
        file: null,
        poster: null,
        received: 0,
        state: "queued",
        error: null,
        addedAt: offline.now(),
        account,
        validator: null,
        sourceKey: null,
        policy,
      });
      added += 1;
    }
    return added;
  });
  emit(app);
  resume(app);
  return added;
}

/** Change which policy owns downloads: `[download id, policy id or null]`. */
export function assign(changes: readonly (readonly [string, string | null])[]): void {
  update((list) => {
    for (const [id, policy] of changes) {
      const d = list.find((d) => d.id === id);
      if (d) d.policy = policy;
    }
  });
}

export function remove(app: AppState, id: string): void {
  try {
    const removed = update((list) => {
      const i = list.findIndex((d) => d.id === id);
      return i >= 0 ? list.splice(i, 1)[0] : undefined;
    });
    if (removed) cleanup(removed);
  } catch (e) {
    app.error(`Could not remove download: ${errorText(e)}`);
  }
  emit(app);
}

function cleanup(d: Download): void {
  for (const f of [d.file, d.poster]) {
    if (!f) continue;
    rmSync(f, { force: true });
    rmSync(`${f}.part`, { force: true });
  }
}

export function pause(app: AppState, id: string): void {
  change(app, (list) => {
    const d = list.find((d) => d.id === id && (d.state === "queued" || d.state === "downloading"));
    if (d) d.state = "paused";
  });
}

export function retry(app: AppState, id: string): void {
  change(app, (list) => {
    const d = list.find((d) => d.id === id && (d.state === "failed" || d.state === "paused"));
    if (d) {
      d.state = "queued";
      d.error = null;
    }
  });
  resume(app);
}

/** Preserve server/account identity all the way into offline playback. */
export function files(ids: readonly string[]): Download[] {
  const all = list();
  const out: Download[] = [];
  for (const id of ids) {
    const d = all.find((d) => d.id === id && d.state === "done");
    if (d && d.file !== null && existsSync(d.file)) out.push(d);
  }
  return out;
}

export function resume(app: AppState): void {
  if (running) return;
  running = true;
  app.spawn(
    (async () => {
      try {
        await worker(app);
      } catch (e) {
        app.error(`Downloads: ${errorText(e)}`);
      }
      running = false;
    })(),
  );
}

async function worker(app: AppState): Promise<void> {
  // This lease is held across awaits; another GUI/TUI process cannot start a second writer.
  const lease = storage.tryLock(join(config.configDir(), "download-worker"));
  if (!lease) return;
  try {
    // With the lease held, any downloading entries were interrupted by a previous process.
    update((list) => {
      for (const d of list) {
        if (d.state === "downloading") d.state = "queued";
      }
    });
    for (;;) {
      const session = app.session();
      if (!session) break;
      const account = offline.account(session);
      const next = update((list) => {
        const d = list.find((d) => d.state === "queued" && d.serverId === session.serverId && (!d.account || d.account === account));
        if (!d) return undefined;
        d.state = "downloading";
        d.error = null;
        return structuredClone(d);
      });
      if (!next) break;
      const d = next;
      emit(app);
      let result: { ok: boolean } | { err: unknown } = { ok: false };
      for (let attempt = 0; attempt < 3; attempt++) {
        try {
          result = { ok: await fetchOne(app, session.server, d.id) };
        } catch (err) {
          result = { err };
        }
        if ("ok" in result || !isActive(d.id) || !retryable(result.err)) break;
        if (attempt < 2) await Bun.sleep(1000 * 2 ** attempt);
      }
      const outcome = result;
      update((list) => {
        const current = list.find((x) => x.id === d.id && x.state === "downloading");
        if (!current) return;
        if ("ok" in outcome) {
          if (outcome.ok) {
            current.state = "done";
            current.error = null;
          } else {
            current.state = "paused";
          }
        } else {
          current.state = "failed";
          current.error = errorText(outcome.err);
        }
      });
      if (!entry(d.id)) cleanup(d);
      emit(app);
    }
  } finally {
    lease.release();
  }
}

/** A failure while reading or writing the transfer; the same kinds reqwest retried (timeout, connect, body). */
class TransferError extends Error {
  constructor(
    message: string,
    readonly retry: boolean,
  ) {
    super(message);
    this.name = "TransferError";
  }
}

function retryable(error: unknown): boolean {
  if (error instanceof TransferError) return error.retry;
  if (error instanceof PlexError) {
    // `Http` wraps reqwest errors (timeouts, connection failures); `Status` retries only transient codes.
    if (error.kind === "http") return true;
    return error.kind === "status" && [429, 500, 502, 503, 504].includes(error.status ?? 0);
  }
  return false;
}

function isActive(id: string): boolean {
  return entry(id)?.state === "downloading";
}

/** Rust `char::is_control`. */
const CONTROL = /\p{Cc}/u;

export function clean(name: string): string {
  const s = Array.from(name)
    .map((c) => (":/\\*?\"<>|".includes(c) || CONTROL.test(c) ? " " : c))
    .join("");
  const words = s.split(/\s+/).filter((w) => w !== "").join(" ");
  const trimmed = words.replace(/^\.+|\.+$/g, "");
  return trimmed === "" ? "Untitled" : trimmed;
}

const pad2 = (n: number): string => String(n).padStart(2, "0");

export function target(dir: string, item: Metadata, ext: string): string {
  let folder: string;
  let name: string;
  switch (Metadata.kind(item)) {
    case "episode":
      folder = join(dir, clean(item.grandparentTitle ?? "Shows"), clean(item.parentTitle ?? "Season"));
      name = `${Metadata.episodeCode(item)} ${Metadata.title(item)}`;
      break;
    case "track":
      folder = join(dir, clean(item.grandparentTitle ?? "Music"), clean(item.parentTitle ?? "Album"));
      name = `${pad2(item.index ?? 0)} ${Metadata.title(item)}`;
      break;
    default:
      folder = dir;
      name = Metadata.fullTitle(item);
  }
  return join(folder, `${clean(name)} [${clean(Metadata.ratingKey(item))}].${ext}`);
}

/** Rust `str::parse::<u64>()` with its error texts. */
function parseU64(s: string): number {
  if (s === "") throw new Error("cannot parse integer from empty string");
  if (!/^\+?\d+$/.test(s)) throw new Error("invalid digit found in string");
  if (BigInt(s) > 0xffffffffffffffffn) throw new Error("number too large to fit in target type");
  return Number(s);
}

/** Validate a resumed response before appending even one byte. Returns `[offset, total]`. */
export function responseOffset(status: number, range: string | null, requested: number): [number, number | null] {
  if (status === 200) return [0, null];
  if (status !== 206) throw new Error(`Unexpected download response: ${status}`);
  if (range === null || !range.startsWith("bytes ")) throw new Error("Missing Content-Range");
  const spec = range.slice("bytes ".length);
  const slash = spec.indexOf("/");
  if (slash < 0) throw new Error("Invalid Content-Range");
  const span = spec.slice(0, slash);
  const totalText = spec.slice(slash + 1);
  const dash = span.indexOf("-");
  if (dash < 0) throw new Error("Invalid byte range");
  const start = parseU64(span.slice(0, dash));
  const end = parseU64(span.slice(dash + 1));
  const total = parseU64(totalText);
  if (!(start === requested && end >= start && end < total)) throw new Error("Server returned the wrong byte range");
  return [start, total];
}

/** Only a strong ETag or modification date can identify bytes in a saved partial file. */
export function responseValidator(resp: Response): string | null {
  const etag = resp.headers.get("ETag");
  if (etag !== null && !etag.startsWith("W/")) return etag;
  return resp.headers.get("Last-Modified");
}

/** Cancel a response body that will not be read. */
async function discard(resp: Response): Promise<void> {
  await resp.body?.cancel().catch(() => undefined);
  finishResponse(resp);
}

type ReadResult = { done: false; value: Uint8Array } | { done: true; value?: undefined };

const never = Symbol("timeout");

async function fetchOne(app: AppState, server: PlexServer, id: string): Promise<boolean> {
  const d = entry(id);
  if (!d) return false;
  const item = await server.metadata(d.item.ratingKey ?? "");
  if (!item) throw new Error("The item no longer exists on the server.");
  const part = item.Media[0]?.Part[0];
  if (!part) throw new Error("No file to download.");
  const key = part.key;
  if (key === null) throw new Error("No file to download.");
  const ext = part.container !== null && part.container.length <= 12 && /^[A-Za-z0-9]*$/.test(part.container) ? part.container : "mkv";
  // Account/server ids separate files with identical titles and versions.
  const scope = offline.digest(`${d.serverId}:${d.account ?? ""}`);
  const dir = join(config.downloadsDir(app.settings().player), `${clean(d.serverName)} - ${scope.slice(0, 8)}`);
  const path = d.file ? d.file : target(dir, item, ext);
  const parent = dirname(path);
  mkdirSync(parent, { recursive: true });
  const partial = `${path}.part`;
  let existing = 0;
  try {
    existing = statSync(partial).size;
  } catch {
    existing = 0;
  }
  const savedValidator = d.validator ?? null;
  // Without a validator a partial file cannot safely be matched to a changed source.
  let requested = d.sourceKey === key && savedValidator !== null ? existing : 0;
  let resp: Response;
  try {
    resp = await server.downloadRange(key, requested, savedValidator);
  } catch (e) {
    if (e instanceof PlexError && e.kind === "status" && e.status === 416) {
      requested = 0;
      resp = await server.downloadRange(key, 0, null);
    } else {
      throw e;
    }
  }
  // A correctly aligned range alone cannot prove that the source is unchanged.
  if (requested > 0 && resp.status === 206 && responseValidator(resp) !== savedValidator) {
    await discard(resp);
    requested = 0;
    resp = await server.downloadRange(key, 0, null);
  }
  try {
    const [offset, rangeTotal] = responseOffset(resp.status, resp.headers.get("Content-Range"), requested);
    const contentLength = resp.headers.get("Content-Length");
    const length = contentLength !== null && /^\d+$/.test(contentLength) ? Number(contentLength) : null;
    const total = rangeTotal ?? (length !== null ? offset + length : null) ?? (part.size !== null && part.size >= 0 ? part.size : null);
    const validator = responseValidator(resp);
    if (total !== null) {
      // fs2::available_space: blocks available to unprivileged users times the fragment size.
      const fs = statfsSync(parent) as { bavail: number | bigint; bsize: number | bigint; frsize?: number | bigint };
      const available = Number(fs.bavail) * Number(fs.frsize ?? fs.bsize);
      if (!(available >= Math.max(total - offset, 0))) throw new Error("Not enough free disk space for this download.");
    }
    const thumb = item.thumb ?? item.parentThumb ?? item.grandparentThumb;
    let posterPath = d.poster ?? null;
    if (posterPath === null && thumb !== null) {
      const cached = await images.cachedFile(thumb, 480, 720);
      if (cached) {
        const art = artworkDir();
        mkdirSync(art, { recursive: true });
        const dest = join(art, `${offline.digest(id)}.img`);
        try {
          copyFileSync(cached, dest);
          posterPath = dest;
        } catch {
          // The artwork is optional.
        }
      }
    }
    update((list) => {
      const x = list.find((x) => x.id === id);
      if (x) {
        x.item = item;
        x.file = path;
        x.poster = posterPath;
        x.size = total ?? 0;
        x.received = offset;
        x.validator = validator;
        x.sourceKey = key;
      }
    });
    if (!isActive(id)) return false;
    // create + write + truncate when starting over, append when resuming.
    const fd = openSync(partial, offset > 0 ? "a" : "w");
    let received = offset;
    let last = Date.now();
    let lastChunk = Date.now();
    let lastControl = Date.now() - 1000;
    const reader = resp.body ? resp.body.getReader() : null;
    let pending: Promise<ReadResult> | null = null;
    try {
      for (;;) {
        if (Date.now() - lastChunk >= 60_000) throw new TransferError("The download stalled. Resume to try again.", false);
        const checkControl = Date.now() - lastControl >= 250;
        if (checkControl) lastControl = Date.now();
        if (checkControl && !isActive(id)) {
          closeSync(fd);
          await reader?.cancel().catch(() => undefined);
          if (!entry(id)) {
            rmSync(partial, { force: true });
            rmSync(path, { force: true });
          }
          return false;
        }
        if (!reader) break;
        pending ??= reader.read();
        let timer: ReturnType<typeof setTimeout> | undefined;
        let chunk: ReadResult | typeof never;
        try {
          chunk = await Promise.race([
            pending,
            new Promise<typeof never>((resolve) => {
              timer = setTimeout(() => resolve(never), 250);
            }),
          ]);
        } catch (e) {
          pending = null;
          throw new TransferError(`downloading: error decoding response body: ${errorText(e)}`, true);
        } finally {
          clearTimeout(timer);
        }
        if (chunk === never) continue;
        pending = null;
        if (chunk.done) break;
        lastChunk = Date.now();
        try {
          let at = 0;
          while (at < chunk.value.length) at += writeSync(fd, chunk.value, at, chunk.value.length - at);
        } catch (e) {
          throw new TransferError(`writing the file: ${errorText(e)}`, false);
        }
        received += chunk.value.length;
        if (Date.now() - last >= 500) {
          last = Date.now();
          update((list) => {
            const x = list.find((x) => x.id === id);
            if (x) x.received = received;
          });
          emit(app);
        }
      }
      if (total !== null && total !== received) throw new Error("Download ended before the complete file arrived.");
      // fsync the data before it becomes the finished file.
      fsyncSync(fd);
    } catch (e) {
      try {
        closeSync(fd);
      } catch {
        // already closed
      }
      await reader?.cancel().catch(() => undefined);
      throw e;
    }
    closeSync(fd);
    const lock = storage.lock(manifest());
    try {
      if (!isActive(id)) return false;
      renameSync(partial, path);
      const all = readRaw();
      const x = all.find((x) => x.id === id);
      if (x) {
        x.received = received;
        x.size = received;
        x.state = "done";
      }
      storage.write(manifest(), new TextEncoder().encode(JSON.stringify(all, null, 2)));
    } finally {
      lock.release();
    }
    return true;
  } finally {
    finishResponse(resp);
  }
}
