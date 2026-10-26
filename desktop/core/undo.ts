// Undo for reversible actions: watch-state changes (single and bulk), removing a
// playlist entry and removing something from Continue Watching.
//
// Each action goes through a function here, which records what it needs to put
// things back *before* changing anything, does the change, and announces it with a
// toast carrying the undo entry's id (see `Toast.undo` in state.ts). Front ends
// then offer "Undo" on that toast and run `undo` with the id, or with `null` for
// the most recent entry (Cmd/Ctrl+Z, the TUI's `Z`).
//
// The stack belongs to one account and server: entries of another scope are dropped
// when the session changes (`Stack.retainScope`) and refused if one slips through.

import type { Metadata, PlexServer } from "../plex";
import * as offline from "./offline";
import * as servers from "./servers";
import { type AppState, Changes, type Session } from "./state";
import { mapLimit } from "./ops/util";

/** How many undoable actions are remembered. */
export const DEPTH = 25;
/** Calls made at once when restoring many items. */
const CONCURRENCY = 6;

/** The account and server an undo entry belongs to. */
export function scopeOf(session: Session): string {
  return `${offline.account(session)}:${session.serverId}`;
}

function currentScope(app: AppState): string | null {
  const s = app.session();
  return s ? scopeOf(s) : null;
}

/** What an item's watch state was. */
export interface WatchState {
  ratingKey: string;
  watched: boolean;
  /** Resume position in milliseconds. */
  offset: number;
}

function watchStateOf(item: Metadata): WatchState {
  return {
    ratingKey: item.ratingKey ?? "",
    watched: (item.viewCount ?? 0) > 0,
    offset: Math.max(item.viewOffset ?? 0, 0),
  };
}

/** How to reverse one change. */
export type Action =
  /** Put these items back to their earlier watch state. */
  | { kind: "watch"; server: string | null; restore: WatchState[] }
  /** Put a removed entry back into a playlist, after the entry it followed. */
  | { kind: "playlistEntry"; playlist: string; ratingKey: string; after: number | null }
  /** Bring back a resume position (what removing from Continue Watching and clearing progress take away). */
  | { kind: "progress"; server: string | null; ratingKey: string; offset: number };

export interface Entry {
  id: number;
  scope: string;
  /** What the action was, as in the toast ("Marked Heat watched"). */
  label: string;
  action: Action;
}

/** The undo history, newest last. */
export class Stack {
  entries: Entry[] = [];
  private next = 0;

  /** Remember an action; returns its id. Entries of other scopes are dropped first. */
  push(scope: string, label: string, action: Action): number {
    this.retainScope(scope);
    this.next += 1;
    this.entries.push({ id: this.next, scope, label, action });
    if (this.entries.length > DEPTH) this.entries.shift();
    return this.next;
  }

  /** Drop everything that does not belong to `scope`. */
  retainScope(scope: string): void {
    this.entries = this.entries.filter((e) => e.scope === scope);
  }

  clear(): void {
    this.entries = [];
  }

  /** The newest entry of `scope`. */
  last(scope: string): Entry | null {
    for (let i = this.entries.length - 1; i >= 0; i--) {
      if (this.entries[i]!.scope === scope) return this.entries[i]!;
    }
    return null;
  }

  /** Remove and return entry `id`, or the newest one of `scope`. */
  take(scope: string, id: number | null = null): Entry | null {
    this.retainScope(scope);
    let at: number;
    if (id !== null) {
      at = this.entries.findIndex((e) => e.id === id);
      if (at < 0) return null;
    } else {
      at = this.entries.length - 1;
      if (at < 0) return null;
    }
    return this.entries.splice(at, 1)[0]!;
  }

  /** Put an entry that could not be undone back where it was (the newest slot). */
  restore(entry: Entry): void {
    this.entries.push(entry);
  }

  get length(): number {
    return this.entries.length;
  }

  isEmpty(): boolean {
    return this.entries.length === 0;
  }
}

/** Forget every entry (the session ended). */
export function clear(app: AppState): void {
  app.undo.clear();
}

/** Drop entries that belong to another account or server. */
export function sessionChanged(app: AppState, session: Session | null): void {
  if (session) app.undo.retainScope(scopeOf(session));
  else app.undo.clear();
}

/** The id and description of what `undo` would reverse. */
export function last(app: AppState): [number, string] | null {
  const scope = currentScope(app);
  if (scope === null) return null;
  const e = app.undo.last(scope);
  return e ? [e.id, e.label] : null;
}

/**
 * Remember an action begun in `scope` (taken before its first request). Nothing is
 * recorded if the account or server changed meanwhile: the action's rating keys and
 * "current server" would then be read against the wrong one.
 */
function record(app: AppState, scope: string | null, label: string, action: Action): number | null {
  if (scope === null) return null;
  if (currentScope(app) !== scope) return null;
  return app.undo.push(scope, label, action);
}

/** Toast `text`, offering undo when the action was recorded. */
function announce(app: AppState, text: string, undoId: number | null): void {
  if (undoId !== null) app.toastUndo(text, undoId);
  else app.toast(text);
}

// ---- watch state ---------------------------------------------------------

/**
 * Every playable thing below the given items, with its current watch state, for the
 * ones whose state `watched` would change.
 */
export function changing(items: Iterable<Metadata>, watched: boolean): WatchState[] {
  return Array.from(items, watchStateOf).filter((s) => (watched ? !s.watched : s.watched || s.offset > 0));
}

/** The watch state of `keys` and, for shows, seasons, artists and albums, of everything inside. */
async function leafStates(server: PlexServer, keys: string[]): Promise<[Metadata[], Metadata[]]> {
  const found: Metadata[] = [];
  for (let i = 0; i < keys.length; i += 40) {
    found.push(...(await server.metadataMany(keys.slice(i, i + 40))));
  }
  const leaves: Metadata[] = [];
  const nested: string[] = [];
  for (const item of found) {
    if (["show", "season", "artist", "album"].includes(item.type ?? "")) nested.push(item.ratingKey ?? "");
    else leaves.push(item);
  }
  const inside = await mapLimit(nested, CONCURRENCY, async (rk) => (await server.allLeaves(rk)).Metadata);
  for (const list of inside) leaves.push(...list);
  return [found, leaves];
}

export function watchLabel(found: Metadata[], keys: number, watched: boolean): string {
  const state = watched ? "watched" : "unwatched";
  if (found.length === 1 && keys === 1) return `Marked ${found[0]!.title ?? ""} ${state}`;
  if (keys === 1) return `Marked as ${state}`;
  return `Marked ${keys} items ${state}`;
}

/**
 * Mark items (or whole shows, seasons, albums) watched or unwatched on `serverId`
 * (`null` for the current one), remembering how to put them back.
 */
export async function setWatched(app: AppState, serverId: string | null, keys: string[], watched: boolean): Promise<void> {
  const scope = currentScope(app);
  const foreign = serverId != null && !servers.isCurrent(app, serverId) ? serverId : null;
  const server = await servers.serverFor(app, serverId);
  // The earlier state is best effort: failing to read it must not stop the change.
  const before = await leafStates(server, keys).catch(() => null);
  for (const key of keys) {
    if (watched) await server.scrobble(key);
    else await server.unscrobble(key);
  }
  let undoEntry: [string, Action] | null = null;
  if (before) {
    const [found, leaves] = before;
    const restore = changing(leaves, watched);
    if (restore.length > 0) undoEntry = [watchLabel(found, keys.length, watched), { kind: "watch", server: foreign, restore }];
  }
  const text = undoEntry
    ? undoEntry[0]
    : keys.length === 1
      ? `Marked as ${watched ? "watched" : "unwatched"}`
      : `Marked ${keys.length} items ${watched ? "watched" : "unwatched"}`;
  const id = undoEntry ? record(app, scope, undoEntry[0], undoEntry[1]) : null;
  announce(app, text, id);
  app.bumpWith(Changes.everything());
}

/** Forget the resume position without changing the watched state. */
export async function clearProgress(app: AppState, ratingKey: string): Promise<void> {
  const scope = currentScope(app);
  const server = app.server();
  const before = (await server.metadata(ratingKey).catch(() => null)) ?? null;
  await server.setProgress(ratingKey, 0, "stopped");
  const offset = before ? (before.viewOffset ?? 0) : 0;
  let id: number | null = null;
  if (offset > 0) {
    const label = `Cleared progress of ${before?.title ?? "the item"}`;
    id = record(app, scope, label, { kind: "progress", server: null, ratingKey, offset });
  }
  announce(app, "Progress cleared", id);
  app.bumpWith(Changes.everything());
}

/**
 * Hide an item from Continue Watching. Undo is offered when it had a resume position
 * to bring back.
 */
export async function removeFromContinueWatching(app: AppState, ratingKey: string): Promise<void> {
  const scope = currentScope(app);
  const server = app.server();
  const before = (await server.metadata(ratingKey).catch(() => null)) ?? null;
  await server.removeFromContinueWatching(ratingKey);
  const offset = before ? (before.viewOffset ?? 0) : 0;
  let id: number | null = null;
  if (offset > 0) {
    const label = `Removed ${before?.title ?? "the item"} from Continue Watching`;
    id = record(app, scope, label, { kind: "progress", server: null, ratingKey, offset });
  }
  announce(app, "Removed from Continue Watching", id);
  app.bumpWith(Changes.everything());
}

// ---- playlists -----------------------------------------------------------

/** A playlist entry as far as position matters. */
export interface Slot {
  entryId: number;
  ratingKey: string;
  title: string;
}

async function playlistSlots(server: PlexServer, id: string): Promise<Slot[]> {
  const PAGE = 500;
  const out: Slot[] = [];
  while (true) {
    const mc = await server.playlistItems(id, out.length, PAGE);
    const got = mc.Metadata.length;
    const total = mc.totalSize != null ? Math.max(mc.totalSize, 0) : null;
    for (const m of mc.Metadata) {
      if (m.playlistItemID == null) continue;
      out.push({ entryId: m.playlistItemID, ratingKey: m.ratingKey ?? "", title: m.title ?? "" });
    }
    if (got < PAGE || (total !== null && out.length >= total) || out.length >= 50_000) break;
  }
  return out;
}

/** The slot of `entryId` and the entry id it follows. */
export function locate(slots: Slot[], entryId: number): [Slot, number | null] | null {
  const at = slots.findIndex((s) => s.entryId === entryId);
  if (at < 0) return null;
  return [slots[at]!, at > 0 ? slots[at - 1]!.entryId : null];
}

/** Remove one entry from a playlist, remembering where it was. */
export async function removeFromPlaylist(app: AppState, playlist: string, entryId: number): Promise<void> {
  const scope = currentScope(app);
  const server = app.server();
  const slots = await playlistSlots(server, playlist).catch(() => null);
  const found = slots ? locate(slots, entryId) : null;
  await server.removeFromPlaylist(playlist, entryId);
  let text: string;
  let id: number | null;
  if (found) {
    const [slot, after] = found;
    const label = `Removed ${slot.title} from the playlist`;
    id = record(app, scope, label, { kind: "playlistEntry", playlist, ratingKey: slot.ratingKey, after });
    text = label;
  } else {
    text = "Removed from the playlist";
    id = null;
  }
  announce(app, text, id);
  app.bumpWith(Changes.everything());
}

/**
 * Where to put a restored entry, given the playlist as it is now (with the restored
 * entry last): after `after` if that entry still exists, at the top when it was first,
 * else it stays at the end. The outer `null` is "stay at the end"; `{ after }` carries
 * the target (itself `null` for the top).
 */
export function place(slots: Slot[], after: number | null): { after: number | null } | null {
  if (after === null) return { after: null };
  // `slots.iter().rev().skip(1)`: everything but the last (the restored entry itself).
  if (slots.slice(0, -1).some((s) => s.entryId === after)) return { after };
  return null;
}

async function restorePlaylistEntry(server: PlexServer, playlist: string, ratingKey: string, after: number | null): Promise<void> {
  await server.addToPlaylist(playlist, server.itemUri([ratingKey]));
  const slots = await playlistSlots(server, playlist);
  const added = slots[slots.length - 1];
  if (!added) throw new Error("The playlist is empty.");
  if (added.ratingKey === ratingKey) {
    const target = place(slots, after);
    if (target) await server.movePlaylistItem(playlist, added.entryId, target.after);
  }
}

// ---- undoing -------------------------------------------------------------

/** Restore calls for watch states: scrobble or unscrobble, then the resume position. */
export type Call =
  | { kind: "scrobble"; ratingKey: string }
  | { kind: "unscrobble"; ratingKey: string }
  | { kind: "progress"; ratingKey: string; ms: number };

export function callsFor(restore: WatchState[]): Call[] {
  const out: Call[] = [];
  for (const s of restore) {
    out.push(s.watched ? { kind: "scrobble", ratingKey: s.ratingKey } : { kind: "unscrobble", ratingKey: s.ratingKey });
    if (!s.watched && s.offset > 0) out.push({ kind: "progress", ratingKey: s.ratingKey, ms: s.offset });
  }
  return out;
}

async function runCalls(server: PlexServer, calls: Call[]): Promise<void> {
  // One item's calls stay in order; different items go in parallel.
  const perItem = new Map<string, Call[]>();
  for (const c of calls) {
    const list = perItem.get(c.ratingKey);
    if (list) list.push(c);
    else perItem.set(c.ratingKey, [c]);
  }
  await mapLimit([...perItem.values()], CONCURRENCY, async (list) => {
    for (const c of list) {
      if (c.kind === "scrobble") await server.scrobble(c.ratingKey);
      else if (c.kind === "unscrobble") await server.unscrobble(c.ratingKey);
      else await server.setProgress(c.ratingKey, c.ms, "stopped");
    }
  });
}

/**
 * Reverse an action. `id` names an entry (an Undo button), `null` the most recent one.
 * Returns what was undone; the front ends are told with a toast and a revision.
 */
export async function undo(app: AppState, id: number | null = null): Promise<string> {
  const scope = currentScope(app);
  if (scope === null) throw new Error("Not connected to a server.");
  const entry = app.undo.take(scope, id);
  if (!entry) throw new Error("Nothing to undo.");
  try {
    const a = entry.action;
    if (a.kind === "watch") {
      await runCalls(await servers.serverFor(app, a.server), callsFor(a.restore));
    } else if (a.kind === "playlistEntry") {
      await restorePlaylistEntry(app.server(), a.playlist, a.ratingKey, a.after);
    } else {
      await (await servers.serverFor(app, a.server)).setProgress(a.ratingKey, a.offset, "stopped");
    }
  } catch (e) {
    // Keep it so a retry is possible once the server is reachable.
    if (currentScope(app) === entry.scope) app.undo.restore(entry);
    throw e;
  }
  app.toast(`Undid: ${entry.label}`);
  app.bumpWith(Changes.everything());
  return entry.label;
}
