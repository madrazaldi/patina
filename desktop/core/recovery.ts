// What survives a connection failure or a restart: a local position per item
// (checkpoints) and the play queue with its position.
//
// Both are written to the platform data directory and scoped by account and
// server, so another profile or server never sees them. Plex stays the source
// of truth for watch state; a checkpoint only matters when the server did not
// hear about the last minutes of playback (the connection dropped), which is
// why one is ignored once the server has a newer view of the item.

import { join } from "node:path";
import { defaultMetadata, normalizeMetadata } from "../plex";
import type { Metadata } from "../plex";
import { dataDir } from "./config";
import * as offline from "./offline";
import type { Session } from "./state";
import * as storage from "./storage";

/** The account and server something belongs to. */
export class Scope {
  constructor(
    public account: string,
    public server: string,
  ) {}

  static of(session: Session): Scope {
    return new Scope(offline.account(session), session.serverId);
  }

  /** `Option<Scope>` equality. */
  static eq(a: Scope | null | undefined, b: Scope | null | undefined): boolean {
    if (!a || !b) return !a && !b;
    return a.account === b.account && a.server === b.server;
  }

  clone(): Scope {
    return new Scope(this.account, this.server);
  }
}

function checkpointsPath(): string {
  return join(dataDir(), "playback-checkpoints.json");
}

function queuesPath(): string {
  return join(dataDir(), "saved-queues.json");
}

const MONTH = 30 * 24 * 3600;
/**
 * A local checkpoint is only used to start playback again for this long; after that
 * the server's position (or an explicit resume of a saved queue) decides.
 */
export const RECOVERY_WINDOW = 6 * 3600;
const MAX_CHECKPOINTS = 200;
const MAX_QUEUES = 16;
/** Longest queue kept; around the playing item when there are more. */
export const MAX_QUEUE_ITEMS = 300;

/** Keep the elements of `list` that satisfy `keep`, in place. */
function retain<T>(list: T[], keep: (x: T) => boolean): void {
  const kept = list.filter(keep);
  list.splice(0, list.length, ...kept);
}

// Checkpoints ------------------------------------------------------------------

// Serialized as is (no rename): snake_case, shared with the Rust core.
export interface Checkpoint {
  account: string;
  server: string;
  rating_key: string;
  position: number;
  duration: number;
  updated: number;
}

function isCheckpoint(c: Checkpoint, scope: Scope, ratingKey: string): boolean {
  return c.account === scope.account && c.server === scope.server && c.rating_key === ratingKey;
}

/** Remember where an item is being played. */
export function saveCheckpoint(scope: Scope, ratingKey: string, position: number, duration: number): void {
  const now = offline.now();
  storage.updateJson<Checkpoint[], void>(checkpointsPath(), () => [], (list) => {
    retain(list, (c) => !isCheckpoint(c, scope, ratingKey) && now - c.updated < MONTH);
    list.push({
      account: scope.account,
      server: scope.server,
      rating_key: ratingKey,
      position: Math.max(0, position),
      duration,
      updated: now,
    });
    if (list.length > MAX_CHECKPOINTS) list.splice(0, list.length - MAX_CHECKPOINTS);
  });
}

/** Forget an item's checkpoint (it was finished). */
export function clearCheckpoint(scope: Scope, ratingKey: string): void {
  storage.updateJson<Checkpoint[], void>(checkpointsPath(), () => [], (list) => {
    retain(list, (c) => !isCheckpoint(c, scope, ratingKey));
  });
}

export function checkpoint(scope: Scope, ratingKey: string): Checkpoint | null {
  try {
    return storage.readJson<Checkpoint[]>(checkpointsPath(), () => []).find((c) => isCheckpoint(c, scope, ratingKey)) ?? null;
  } catch {
    return null;
  }
}

/** The position to resume `item` from, if a local checkpoint is newer than what the server knows. */
export function resumePosition(scope: Scope, item: Metadata): number | null {
  if (item.ratingKey === null) return null;
  const cp = checkpoint(scope, item.ratingKey);
  if (!cp) return null;
  return usable(cp, item.lastViewedAt, offline.now());
}

export function usable(cp: Checkpoint, lastViewedAt: number | null, now: number): number | null {
  const finished = cp.duration > 0 && cp.position >= cp.duration * 0.95;
  // The server saw the item after the checkpoint (here or on another device): it knows better.
  const serverNewer = lastViewedAt !== null && lastViewedAt > cp.updated;
  return cp.position > 0 && !finished && !serverNewer && now - cp.updated < RECOVERY_WINDOW ? cp.position : null;
}

// Saved queue ------------------------------------------------------------------

/** The queue that was playing, to offer again after a restart. */
export interface SavedQueue {
  account: string;
  server: string;
  saved_at: number;
  /** `video` or `audio`. */
  kind: string;
  /** Slim copies of the queue items, in play order. */
  items: Metadata[];
  current: number;
  position_ms: number;
  quality: string | null;
}

function isQueueOf(q: SavedQueue, scope: Scope): boolean {
  return q.account === scope.account && q.server === scope.server;
}

/** What a front end shows in its "Resume" offer. */
export interface SavedQueueView {
  current: Metadata;
  positionMs: number;
  index: number;
  count: number;
  savedAt: number;
}

export function savedQueueView(q: SavedQueue): SavedQueueView | null {
  const current = q.items[q.current];
  if (!current) return null;
  return { current, positionMs: q.position_ms, index: q.current, count: q.items.length, savedAt: q.saved_at };
}

/** An item reduced to what is needed to list and re-queue it. */
export function slim(m: Metadata): Metadata {
  return defaultMetadata({
    ratingKey: m.ratingKey,
    key: m.key,
    type: m.type,
    title: m.title,
    year: m.year,
    index: m.index,
    parentIndex: m.parentIndex,
    duration: m.duration,
    thumb: m.thumb,
    parentThumb: m.parentThumb,
    grandparentThumb: m.grandparentThumb,
    parentTitle: m.parentTitle,
    grandparentTitle: m.grandparentTitle,
    parentRatingKey: m.parentRatingKey,
    grandparentRatingKey: m.grandparentRatingKey,
    librarySectionID: m.librarySectionID,
    lastViewedAt: m.lastViewedAt,
  });
}

/** A half-open range of indexes (`start..end`). */
export interface Range {
  start: number;
  end: number;
}

/**
 * At most `MAX_QUEUE_ITEMS` items keeping `current` in view; returns the window and the new index of `current`.
 */
export function window(len: number, current: number): [Range, number] {
  if (len <= MAX_QUEUE_ITEMS) return [{ start: 0, end: len }, current];
  const start = Math.min(Math.max(0, current - Math.floor(MAX_QUEUE_ITEMS / 10)), len - MAX_QUEUE_ITEMS);
  return [{ start, end: start + MAX_QUEUE_ITEMS }, current - start];
}

export function saveQueue(scope: Scope, queue: SavedQueue): void {
  const now = offline.now();
  storage.updateJson<SavedQueue[], void>(queuesPath(), () => [], (list) => {
    retain(list, (q) => !isQueueOf(q, scope) && now - q.saved_at < MONTH);
    list.push(queue);
    if (list.length > MAX_QUEUES) list.splice(0, list.length - MAX_QUEUES);
  });
}

export function loadQueue(scope: Scope): SavedQueue | null {
  const now = offline.now();
  let list: SavedQueue[];
  try {
    list = storage.readJson<SavedQueue[]>(queuesPath(), () => []);
  } catch {
    return null;
  }
  const found = list.find((q) => isQueueOf(q, scope) && now - q.saved_at < MONTH && q.current < q.items.length);
  if (!found) return null;
  // The Rust core writes every Metadata field; make sure all of them exist here too.
  return { ...found, items: found.items.map((m) => normalizeMetadata(m)), quality: found.quality ?? null };
}

export function clearQueue(scope: Scope): void {
  storage.updateJson<SavedQueue[], void>(queuesPath(), () => [], (list) => {
    retain(list, (q) => !isQueueOf(q, scope));
  });
}
