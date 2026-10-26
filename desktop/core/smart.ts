// Smart downloads: keep the next few unwatched episodes of a show, season or
// playlist downloaded, and clean up after watching.
//
// A `Policy` belongs to one account on one server. `reconcile` compares
// what each enabled policy wants with what is downloaded and queues or
// removes files to match. It only ever looks at the connected server and
// account, and it only removes downloads a policy fetched itself
// (`Download.policy`); downloads someone asked for by hand are never
// touched. An optional storage budget limits how much a policy may add.
//
// Reconciling runs on sign-in, whenever watch state may have changed (after
// playback, a scrobble, an offline sync or a live server update, through
// `AppState.bump`) and every few minutes.
// Port of crates/core/src/smart.rs.

import { readFileSync } from "node:fs";
import { join } from "node:path";

import { Metadata, type PlexServer } from "../plex";
import * as config from "./config";
import * as downloads from "./downloads";
import type { Download } from "./downloads";
import { logger } from "./log";
import * as offline from "./offline";
import { errorText } from "./ops/util";
import * as prefs from "./prefs";
import { Scope } from "./recovery";
import type { AppState } from "./state";
import * as storage from "./storage";

const log = logger("smart");

/** What a policy keeps downloaded. */
export type Target = "show" | "season" | "playlist";

export function targetLabel(t: Target): string {
  switch (t) {
    case "show":
      return "show";
    case "season":
      return "season";
    case "playlist":
      return "playlist";
  }
}

export interface Policy {
  id: string;
  /** Account and server the policy belongs to; it is inert anywhere else. */
  account: string;
  serverId: string;
  serverName: string;
  /** The show, season or playlist. */
  ratingKey: string;
  target: Target;
  title: string;
  /** How many unwatched items to keep downloaded. */
  keep: number;
  /** Remove downloads this policy fetched once they are watched. */
  deleteWatched: boolean;
  enabled: boolean;
}

export const MAX_KEEP = 50;

interface Saved {
  /** Most bytes all downloads together may use before policies stop adding. */
  budget: number | null;
  policies: Policy[];
}

const emptySaved = (): Saved => ({ budget: null, policies: [] });

function path(): string {
  return join(config.configDir(), "smart-downloads.json");
}

/** `#[serde(default)]` on the whole struct: missing fields take their defaults. */
function readSaved(): Saved {
  const raw = storage.readJson<Partial<Saved> | null>(path(), emptySaved);
  return {
    budget: typeof raw?.budget === "number" ? raw.budget : null,
    policies: Array.isArray(raw?.policies) ? raw.policies : [],
  };
}

function load(): Saved {
  try {
    return readSaved();
  } catch (e) {
    log.error(`smart downloads: ${errorText(e)}`);
    return emptySaved();
  }
}

function update<R>(f: (saved: Saved) => R): R {
  return storage.updateJson<Saved, R>(path(), emptySaved, (saved) => {
    // Complete a partial file the way serde's defaults would, so the update sees every field.
    saved.budget ??= null;
    saved.policies ??= [];
    return f(saved);
  });
}

/** The saved policies as text, to notice another process changing them. */
export function fingerprint(): string {
  try {
    return readFileSync(path(), "utf8");
  } catch {
    return "";
  }
}

function notify(app: AppState): void {
  app.emit({ kind: "downloads", downloads: downloads.list() });
}

// Evaluation -----------------------------------------------------------------

/** One item a policy could keep, in the order it would be watched. */
export interface Candidate {
  ratingKey: string;
  /** Id of its download. */
  id: string;
  watched: boolean;
  size: number;
}

/** A policy together with its items, as fetched from the server. */
export interface Fetched {
  policy: Policy;
  candidates: Candidate[];
}

/** A download of the connected account and server. */
export interface Held {
  id: string;
  policy: string | null;
  size: number;
}

/** What to change. */
export interface Plan {
  /** `[policy id, rating key]` to download, in order. */
  add: [string, string][];
  /** Download ids to remove. */
  remove: string[];
  /** Downloads whose owner is gone: adopted by a policy that wants them, or released. */
  assign: [string, string | null][];
  /** Items left out because the budget is full. */
  overBudget: number;
}

export function emptyPlan(): Plan {
  return { add: [], remove: [], assign: [], overBudget: 0 };
}

/**
 * Work out what the policies need.
 *
 * - Each policy wants the first `keep` unwatched items.
 * - A download a policy fetched is removed when no policy wants it any more
 *   and it is not a watched item that policy is told to keep. Downloads of
 *   policies that are disabled, or whose items could not be fetched, are left alone.
 * - New items are added in order until `budget` (counting `outsideBytes`, the
 *   downloads of other servers and accounts, and removals just freed) is reached.
 */
export function evaluate(
  fetched: readonly Fetched[],
  held: readonly Held[],
  known: ReadonlySet<string>,
  outsideBytes: number,
  budget: number | null,
): Plan {
  const plan = emptyPlan();
  const evaluated = new Set(fetched.map((f) => f.policy.id));

  const wanted = new Map<string, string>();
  const keepWatched = new Set<string>();
  const windows: Candidate[][] = [];
  for (const f of fetched) {
    const window = f.candidates.filter((c) => !c.watched).slice(0, f.policy.keep);
    for (const c of window) {
      if (!wanted.has(c.id)) wanted.set(c.id, f.policy.id);
    }
    if (!f.policy.deleteWatched) {
      for (const c of f.candidates) if (c.watched) keepWatched.add(c.id);
    }
    windows.push(window);
  }

  let used = outsideBytes;
  const present = new Set<string>();
  for (const h of held) {
    const size = Math.max(h.size, 0);
    const owner = h.policy;
    if (owner !== null && evaluated.has(owner) && !wanted.has(h.id) && !keepWatched.has(h.id)) {
      plan.remove.push(h.id);
      continue;
    }
    if (owner !== null && !known.has(owner)) {
      plan.assign.push([h.id, wanted.get(h.id) ?? null]);
    }
    used += size;
    present.add(h.id);
  }

  fetched.forEach((f, i) => {
    const window = windows[i]!;
    for (const [at, c] of window.entries()) {
      if (present.has(c.id)) continue;
      const size = Math.max(c.size, 0);
      if (budget !== null && used + size > budget) {
        // Later items must not jump the queue.
        plan.overBudget += window.slice(at).filter((w) => !present.has(w.id)).length;
        break;
      }
      used += size;
      present.add(c.id);
      plan.add.push([f.policy.id, c.ratingKey]);
    }
  });
  return plan;
}

/**
 * Put the items of a policy in the order they are watched: a show's
 * episodes by season and number (specials excluded), a season's by number, a
 * playlist as it is.
 */
export function watchOrder(target: Target, items: Metadata[]): Metadata[] {
  switch (target) {
    case "show":
      // Array.prototype.sort is stable, like `sort_by_key`.
      return items
        .filter((m) => m.parentIndex !== 0)
        .sort((a, b) => (a.parentIndex ?? 0) - (b.parentIndex ?? 0) || (a.index ?? 0) - (b.index ?? 0));
    case "season":
      return items.sort((a, b) => (a.index ?? 0) - (b.index ?? 0));
    case "playlist":
      return items;
  }
}

function partSize(m: Metadata): number {
  return m.Media[0]?.Part[0]?.size ?? 0;
}

export function candidates(items: readonly Metadata[], scope: Scope, offlineWatched: ReadonlySet<string>): Candidate[] {
  return items
    .filter((m) => Metadata.isPlayable(m) && m.Media.length > 0)
    .map((m) => {
      const id = downloads.idFor(scope.server, scope.account, Metadata.ratingKey(m));
      return {
        ratingKey: Metadata.ratingKey(m),
        watched: Metadata.isWatched(m) || offlineWatched.has(id),
        size: partSize(m),
        id,
      };
    });
}

/** Downloads of this account and server, and the bytes of all the others. */
export function split(all: readonly Download[], scope: Scope): [Held[], number] {
  const held: Held[] = [];
  let outside = 0;
  for (const d of all) {
    if (d.serverId === scope.server && d.account === scope.account) {
      held.push({ id: d.id, policy: d.policy ?? null, size: d.size });
    } else {
      outside += Math.max(d.size, 0);
    }
  }
  return [held, outside];
}

export function inScope(p: Policy, scope: Scope): boolean {
  return p.account === scope.account && p.serverId === scope.server;
}

// Reconciling ----------------------------------------------------------------

export interface Report {
  added: number;
  removed: number;
  /** Items not queued because the storage budget is full. */
  overBudget: number;
  errors: string[];
}

let running = false;
let pending = false;
let last: number | null = null;

async function fetchItems(server: PlexServer, p: Policy): Promise<Metadata[]> {
  switch (p.target) {
    case "show":
      return (await server.allLeaves(p.ratingKey)).Metadata;
    case "season":
      return (await server.children(p.ratingKey)).Metadata;
    case "playlist":
      return (await server.playlistItems(p.ratingKey, 0, 5000)).Metadata;
  }
}

/** Make the downloads match the policies of the connected account and server. */
export async function reconcile(app: AppState): Promise<Report> {
  const report: Report = { added: 0, removed: 0, overBudget: 0, errors: [] };
  const session = app.session();
  if (!session) return report;
  if (running) return report;
  running = true;
  try {
    // Another Patina process may be reconciling the same files.
    const lease = storage.tryLock(join(config.configDir(), "smart-downloads-run"));
    if (!lease) return report;
    try {
      last = Date.now();

      const scope = Scope.of(session);
      const saved = load();
      const mine = saved.policies.filter((p) => inScope(p, scope));
      const known = new Set(mine.map((p) => p.id));
      const offlineWatched = offline.watchedOffline();

      const items: [Policy, Metadata[]][] = [];
      for (const p of mine.filter((p) => p.enabled)) {
        try {
          items.push([p, watchOrder(p.target, await fetchItems(session.server, p))]);
        } catch (e) {
          // Never remove anything on a guess: this policy sits the round out.
          report.errors.push(`${p.title}: ${errorText(e)}`);
        }
      }
      // The session may have changed while the server was asked.
      const now = app.session();
      if (!now || now.server !== session.server) return report;

      const all = downloads.list();
      const [held, outside] = split(all, scope);
      const fetched: Fetched[] = items.map(([p, list]) => ({ policy: p, candidates: candidates(list, scope, offlineWatched) }));
      const plan = evaluate(fetched, held, known, outside, saved.budget);
      report.overBudget = plan.overBudget;

      if (plan.assign.length > 0) downloads.assign(plan.assign);
      const np = app.nowPlaying();
      const playing = np && np.local ? (np.item.ratingKey ?? null) : null;
      for (const id of plan.remove) {
        // A file being played is cleaned up on a later round.
        if (all.some((d) => d.id === id && playing === (d.item.ratingKey ?? null))) continue;
        downloads.remove(app, id);
        report.removed += 1;
      }
      const byPolicy: [string, Metadata[]][] = [];
      for (const [policy, ratingKey] of plan.add) {
        const item = items.flatMap(([, l]) => l).find((m) => Metadata.ratingKey(m) === ratingKey);
        if (!item) continue;
        const entry = byPolicy.find(([p]) => p === policy);
        if (entry) entry[1].push(item);
        else byPolicy.push([policy, [item]]);
      }
      for (const [policy, list] of byPolicy) {
        report.added += downloads.add(app, session, list, policy);
      }
      if (report.added + report.removed + plan.assign.length > 0) notify(app);
      return report;
    } finally {
      lease.release();
    }
  } finally {
    running = false;
  }
}

/** Reconcile shortly, coalescing requests; no more than once every 20 seconds. */
export function reconcileSoon(app: AppState): void {
  if (pending) return;
  pending = true;
  const since = last !== null ? Date.now() - last : 3600 * 1000;
  const wait = Math.max(Math.max(20_000 - since, 0), 2_000);
  app.spawn(
    (async () => {
      await Bun.sleep(wait);
      pending = false;
      try {
        const r = await reconcile(app);
        for (const e of r.errors) log.debug(`smart downloads: ${e}`);
      } catch (e) {
        log.debug(`smart downloads: ${errorText(e)}`);
      }
    })(),
  );
}

// Managing policies ----------------------------------------------------------

/** A policy as front ends show it. */
export interface PolicyView extends Policy {
  /** Belongs to the connected account and server (others wait until you switch to them). */
  active: boolean;
  /** Files this policy fetched, and their size. */
  downloads: number;
  bytes: number;
}

export interface Overview {
  policies: PolicyView[];
  /** Storage budget in bytes; `null` when unlimited. */
  budget: number | null;
  /** Bytes of every download on this computer. */
  used: number;
}

export function overview(app: AppState): Overview {
  const saved = load();
  const all = downloads.list();
  const scope = prefs.currentScope(app);
  const policies = saved.policies.map((policy): PolicyView => {
    let n = 0;
    let bytes = 0;
    for (const d of all) {
      if (d.policy === policy.id) {
        n += 1;
        bytes += d.size;
      }
    }
    return { ...policy, active: scope !== null && inScope(policy, scope), downloads: n, bytes };
  });
  return { policies, budget: saved.budget, used: all.reduce((sum, d) => sum + Math.max(d.size, 0), 0) };
}

function targetOf(item: Metadata): Target {
  switch (Metadata.kind(item)) {
    case "show":
      return "show";
    case "season":
      return "season";
    case "playlist":
      return "playlist";
    default:
      throw new Error("Smart downloads work on shows, seasons and playlists.");
  }
}

function titleOf(item: Metadata): string {
  return Metadata.kind(item) === "season" ? `${item.parentTitle ?? "Show"} · ${Metadata.title(item)}` : Metadata.title(item);
}

/**
 * Keep the next `keep` unwatched items of a show, season or playlist
 * downloaded. Replaces the settings of an existing policy for it.
 */
export async function setPolicy(app: AppState, ratingKey: string, keep: number, deleteWatched: boolean): Promise<Policy> {
  if (!Number.isInteger(keep) || keep < 1 || keep > MAX_KEEP) throw new Error(`Keep between 1 and ${MAX_KEEP} items.`);
  const session = app.session();
  if (!session) throw new Error("Not connected.");
  const item = await session.server.metadata(ratingKey);
  if (!item) throw new Error("Item not found.");
  const target = targetOf(item);
  const scope = Scope.of(session);
  const fresh: Policy = {
    id: crypto.randomUUID(),
    account: scope.account,
    serverId: scope.server,
    serverName: session.serverName,
    ratingKey,
    target,
    title: titleOf(item),
    keep,
    deleteWatched,
    enabled: true,
  };
  const policy = update((saved) => {
    const existing = saved.policies.find((p) => inScope(p, scope) && p.ratingKey === ratingKey);
    if (existing) {
      existing.keep = keep;
      existing.deleteWatched = deleteWatched;
      existing.enabled = true;
      existing.title = fresh.title;
      return { ...existing };
    }
    saved.policies.push(fresh);
    return fresh;
  });
  notify(app);
  reconcileSoon(app);
  return policy;
}

/** What to change in a policy; unset fields stay. */
export interface Patch {
  keep?: number | null;
  deleteWatched?: boolean | null;
  enabled?: boolean | null;
}

export function updatePolicy(app: AppState, id: string, patch: Patch): void {
  if (patch.keep != null) {
    if (!Number.isInteger(patch.keep) || patch.keep < 1 || patch.keep > MAX_KEEP) throw new Error(`Keep between 1 and ${MAX_KEEP} items.`);
  }
  const found = update((saved) => {
    const p = saved.policies.find((p) => p.id === id);
    if (!p) return false;
    p.keep = patch.keep ?? p.keep;
    p.deleteWatched = patch.deleteWatched ?? p.deleteWatched;
    p.enabled = patch.enabled ?? p.enabled;
    return true;
  });
  if (!found) throw new Error("That policy no longer exists.");
  notify(app);
  reconcileSoon(app);
}

/**
 * Delete a policy. Its downloads stay as ordinary downloads, or are removed
 * too with `deleteDownloads`.
 */
export function removePolicy(app: AppState, id: string, deleteDownloads: boolean): void {
  update((saved) => {
    saved.policies = saved.policies.filter((p) => p.id !== id);
  });
  const owned = downloads.list().filter((d) => d.policy === id).map((d) => d.id);
  if (deleteDownloads) {
    for (const download of owned) downloads.remove(app, download);
  } else {
    downloads.assign(owned.map((d): [string, string | null] => [d, null]));
  }
  notify(app);
}

/** Limit all downloads together to `bytes`; `null` removes the limit. */
export function setBudget(app: AppState, bytes: number | null): void {
  update((saved) => {
    saved.budget = bytes !== null && bytes > 0 ? bytes : null;
  });
  notify(app);
  reconcileSoon(app);
}
