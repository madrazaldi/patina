// Artwork: images are resolved through the current server's photo
// transcoder (so front ends never see the token) and the resized result is
// cached on disk. Port of crates/core/src/images.rs.
//
// The cache is built for a grid of hundreds of posters scrolling past at
// once: simultaneous requests for one image share a single fetch, fetches
// are limited to MAX_FETCHES at a time, entries are written atomically
// (a reader never sees a half-written file), and the folder is kept under a
// size budget as well as an age limit.

import { createHash, randomUUID } from "node:crypto";
import { closeSync, mkdirSync, openSync, readFileSync, readSync, readdirSync, rmSync, statSync } from "node:fs";
import { mkdir, readFile, rename, rm, stat, utimes, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";

import type { PlexServer } from "../plex";
import * as config from "./config";
import * as downloads from "./downloads";

/** Images fetched from the server or the web at the same time. */
export const MAX_FETCHES = 8;
/** The most disk the cache keeps; the least recently used entries go first. */
export const BUDGET_BYTES = 512 * 1024 * 1024;
/** Entries unused for this long are dropped (30 days, in milliseconds). */
export const MAX_AGE_MS = 1000 * 60 * 60 * 24 * 30;
/** Written bytes after which the budget is checked again. */
const PRUNE_EVERY_BYTES = 32 * 1024 * 1024;
/** A hit older than this has its timestamp refreshed, so use keeps entries alive. */
const TOUCH_AFTER_MS = 60 * 60 * 1000;
/** Direct (non-Plex) fetches give up after this long. */
const HTTP_TIMEOUT_MS = 30_000;
/** Temporary files carry this between the entry's name and a random suffix. */
export const TMP_MARK = ".tmp-";

/** An image and its content type. */
export type Fetched = [Uint8Array, string];

let server: PlexServer | null = null;
let written = 0;
let pruning = false;
let cacheMade: string | null = null;

/** The cache folder (created on first use). */
function cacheRoot(): string {
  const dir = join(config.cacheDir(), "images");
  if (cacheMade !== dir) {
    try {
      mkdirSync(dir, { recursive: true });
    } catch {
      // Created on demand when writing.
    }
    cacheMade = dir;
  }
  return dir;
}

/** Counting semaphore (tokio's `Semaphore`). */
export class Semaphore {
  private free: number;
  private readonly waiters: Array<() => void> = [];

  constructor(permits: number) {
    this.free = permits;
  }

  /** Wait for a permit; the returned function gives it back. */
  async acquire(): Promise<() => void> {
    if (this.free > 0) this.free -= 1;
    else await new Promise<void>((resolve) => this.waiters.push(resolve));
    let released = false;
    return () => {
      if (released) return;
      released = true;
      const next = this.waiters.shift();
      if (next) next();
      else this.free += 1;
    };
  }
}

const PERMITS = new Semaphore(MAX_FETCHES);

/**
 * Runs one task per key and lets every caller for that key wait on it.
 *
 * The task is started, not driven by its callers: if all of them give up the
 * work still finishes (and releases whatever it holds), and the key is
 * forgotten the moment it does.
 */
export class SingleFlight<V> {
  private readonly flights = new Map<string, Promise<V | null>>();

  /**
   * The result of the task for `key`, starting it with `make` unless one is
   * already running. `null` if the task failed (panicked).
   */
  run(key: string, make: () => Promise<V>): Promise<V | null> {
    const existing = this.flights.get(key);
    if (existing) return existing;
    let task: Promise<V>;
    try {
      task = make();
    } catch (e) {
      task = Promise.reject(e);
    }
    const flight: Promise<V | null> = task
      .then(
        (v) => v as V | null,
        () => null,
      )
      .finally(() => {
        this.flights.delete(key);
      });
    this.flights.set(key, flight);
    return flight;
  }

  running(): number {
    return this.flights.size;
  }
}

const FLIGHTS = new SingleFlight<Fetched | null>();

/** The server images are fetched through; `null` when signed out. */
export function setServer(s: PlexServer | null): void {
  server = s;
}

/** The current image server (tests). */
export function currentServer(): PlexServer | null {
  return server;
}

/** Saved artwork of a download and its content type; works offline. */
export function downloadPoster(id: string): Fetched | null {
  const path = downloads.poster(id);
  if (!path) return null;
  try {
    return [readFileSync(path), sniff(path)];
  } catch {
    return null;
  }
}

export function cachePath(s: PlexServer | null, source: string, w: number, h: number): string {
  const scope = s?.machineIdentifier() ?? "";
  const key = createHash("sha256").update(`${scope}|${source}|${w}|${h}`).digest("hex");
  return join(cacheRoot(), key.slice(0, 2), key);
}

/**
 * A resized image as a file on disk, for things outside the front end
 * (OS media controls, downloads).
 */
export async function cachedFile(source: string, w: number, h: number): Promise<string | null> {
  if ((await fetch(source, w, h)) === null) return null;
  return cachePath(server, source, w, h);
}

/** A server path or absolute URL resized to fit `w` by `h`, with its content type. */
export async function fetch(source: string, w: number, h: number): Promise<Fetched | null> {
  if (source === "") return null;
  const s = server;
  const path = cachePath(s, source, w, h);
  const hit = await readCached(path);
  if (hit) return hit;
  const shared = await FLIGHTS.run(path, async () => {
    // Another flight may have stored it between the miss and now.
    const again = await readCached(path);
    if (again) return again;
    const release = await PERMITS.acquire();
    try {
      const got = await download(s, source, w, h);
      if (!got) return null;
      // The content type is re-sniffed from the bytes on cache hits.
      try {
        await writeAtomic(path, got[0]);
        noteWritten(got[0].length);
      } catch {
        // A cache that can't be written still serves this request.
      }
      return got;
    } finally {
      release();
    }
  });
  return shared ?? null;
}

/** The cached bytes of `path`, if any (an empty file counts as a miss). */
export async function readCached(path: string): Promise<Fetched | null> {
  let bytes: Uint8Array;
  try {
    bytes = await readFile(path);
  } catch {
    return null;
  }
  if (bytes.length === 0) return null;
  touchIfStale(path);
  return [bytes, sniff(path)];
}

/** Mark an old entry as used just now, off the request path. */
function touchIfStale(path: string): void {
  stat(path).then(
    (m) => {
      if (Date.now() - m.mtimeMs > TOUCH_AFTER_MS) {
        const now = new Date();
        utimes(path, now, now).catch(() => {});
      }
    },
    () => {},
  );
}

/** Ask the server (or the web) for the image. */
async function download(s: PlexServer | null, source: string, w: number, h: number): Promise<Fetched | null> {
  const isAbsolute = source.startsWith("http://") || source.startsWith("https://");
  let got: [Uint8Array, string] | null;
  if (s && !isAbsolute) {
    got = await s.image(source, w, h).then(
      ([b, c]) => [b, c ?? ""],
      () => null,
    );
  } else if (s && isAbsolute && (source.includes("plex.tv") || source.includes("plex.direct"))) {
    // Let the server resize external Plex artwork too, falling back to a direct fetch.
    got = await s.image(source, w, h).then(
      ([b, c]) => [b, c ?? ""] as [Uint8Array, string],
      () => direct(source),
    );
  } else if (isAbsolute) {
    got = await direct(source);
  } else {
    got = null;
  }
  if (!got) return null;
  const [bytes, ctype] = got;
  if (bytes.length === 0) return null;
  return [bytes, ctype.startsWith("image/") ? ctype : "image/jpeg"];
}

async function direct(url: string): Promise<[Uint8Array, string] | null> {
  try {
    const resp = await globalThis.fetch(url, { signal: AbortSignal.timeout(HTTP_TIMEOUT_MS) });
    if (!resp.ok) return null;
    return [new Uint8Array(await resp.arrayBuffer()), resp.headers.get("content-type") ?? ""];
  } catch {
    return null;
  }
}

/**
 * Write to a temporary sibling, then rename over `path`, so readers see
 * either the whole file or none of it.
 */
export async function writeAtomic(path: string, bytes: Uint8Array): Promise<void> {
  const dir = dirname(path);
  await mkdir(dir, { recursive: true });
  const tmp = join(dir, `${basename(path)}${TMP_MARK}${randomUUID().replaceAll("-", "")}`);
  try {
    await writeFile(tmp, bytes);
    await rename(tmp, path);
  } catch (e) {
    await rm(tmp, { force: true }).catch(() => {});
    throw e;
  }
}

/** Count written bytes and check the budget in the background once enough accumulate. */
function noteWritten(bytes: number): void {
  written += bytes;
  if (written < PRUNE_EVERY_BYTES || pruning) return;
  pruning = true;
  written = 0;
  setImmediate(() => {
    try {
      pruneDir(cacheRoot(), Date.now(), MAX_AGE_MS, BUDGET_BYTES);
    } finally {
      pruning = false;
    }
  });
}

/** The content type of an image file, from its first bytes. */
export function sniff(path: string): string {
  const head = Buffer.alloc(8);
  try {
    const fd = openSync(path, "r");
    try {
      readSync(fd, head, 0, 8, 0);
    } finally {
      closeSync(fd);
    }
  } catch {
    // An unreadable file sniffs as the default.
  }
  if (head[0] === 0x89 && head[1] === 0x50 && head[2] === 0x4e && head[3] === 0x47) return "image/png";
  if (head.subarray(0, 4).toString("latin1") === "RIFF") return "image/webp";
  if (head.subarray(0, 3).toString("latin1") === "GIF") return "image/gif";
  return "image/jpeg";
}

/**
 * Drop cached images not used for 30 days, then the least recently used
 * ones until the cache fits its size budget.
 */
export function pruneCache(): Pruned {
  return pruneDir(cacheRoot(), Date.now(), MAX_AGE_MS, BUDGET_BYTES);
}

/** What a prune removed. */
export interface Pruned {
  files: number;
  bytes: number;
}

/**
 * Prune the two-level cache folder `dir`: leftover temporary files, entries
 * last used before `now - maxAge`, then oldest-used entries until the total
 * is at most 90% of `budget` (so a full cache is not trimmed on every write).
 * An entry's age is the later of its modification and access times.
 * Times are milliseconds.
 */
export function pruneDir(dir: string, now: number, maxAge: number, budget: number): Pruned {
  const pruned: Pruned = { files: 0, bytes: 0 };
  const live: Array<[number, number, string]> = [];
  let total = 0;
  let subs: string[];
  try {
    subs = readdirSync(dir);
  } catch {
    return pruned;
  }
  for (const sub of subs) {
    let files: string[];
    try {
      files = readdirSync(join(dir, sub));
    } catch {
      continue;
    }
    for (const name of files) {
      const path = join(dir, sub, name);
      let meta;
      try {
        meta = statSync(path);
      } catch {
        continue;
      }
      if (!meta.isFile()) continue;
      const used = Math.max(meta.atimeMs, meta.mtimeMs);
      const age = Math.max(now - used, 0);
      const temporary = name.includes(TMP_MARK);
      // A temporary file this old belongs to a write that died.
      const expired = temporary ? age > 3_600_000 : age > maxAge;
      if (expired) {
        try {
          rmSync(path);
          pruned.files += 1;
          pruned.bytes += meta.size;
        } catch {
          // Still there; try again next time.
        }
      } else if (!temporary) {
        total += meta.size;
        live.push([used, meta.size, path]);
      }
    }
  }
  if (total > budget) {
    const target = Math.floor(budget / 10) * 9;
    live.sort((a, b) => a[0] - b[0]);
    for (const [, len, path] of live) {
      if (total <= target) break;
      try {
        rmSync(path);
        total -= len;
        pruned.files += 1;
        pruned.bytes += len;
      } catch {
        // Skip entries that can't be removed.
      }
    }
  }
  return pruned;
}
