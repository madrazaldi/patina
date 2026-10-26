// Live updates from the server's notification stream (SSE).
// Port of crates/core/src/live.rs.
//
// Library changes and playback on other devices bump the revision so views
// refetch; background activities (scans, analysis) are mirrored for the
// sidebar and the Activity page; session changes nudge the Activity page.
// The listener reconnects on its own and stops when the session changes.

import { normalizeActivity } from "../plex";
import type { Activity } from "../plex";
import { logger } from "./log";
import * as session from "./session";
import { Changes } from "./state";
import type { AppState, LiveTask, ServerActivity } from "./state";

const log = logger("live");

const ABORTED = Symbol("aborted");

/** Wait for `p`, or return `ABORTED` as soon as `signal` fires (the task is dropped at its next await, as in tokio). */
function until<T>(p: Promise<T>, signal: AbortSignal): Promise<T | typeof ABORTED> {
  if (signal.aborted) {
    p.catch(() => {});
    return Promise.resolve(ABORTED);
  }
  return new Promise((resolve, reject) => {
    const onAbort = () => resolve(ABORTED);
    signal.addEventListener("abort", onAbort, { once: true });
    p.then(
      (v) => {
        signal.removeEventListener("abort", onAbort);
        resolve(v);
      },
      (e) => {
        signal.removeEventListener("abort", onAbort);
        // An error that races an abort is not interesting.
        if (signal.aborted) resolve(ABORTED);
        else reject(e);
      },
    );
  });
}

/** Sleep that ends early when aborted. */
function sleep(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    if (signal.aborted) return resolve();
    const t = setTimeout(done, ms);
    function done() {
      signal.removeEventListener("abort", done);
      clearTimeout(t);
      resolve();
    }
    signal.addEventListener("abort", done, { once: true });
  });
}

/**
 * Start listening to the current server, replacing any previous listener.
 *
 * The previous listener is aborted, not just told to stop: it may be parked
 * waiting for the old server's next event, which can take arbitrarily long
 * (and holds its connection open meanwhile).
 */
export function watch(app: AppState): void {
  app.liveGeneration += 1;
  const generation = app.liveGeneration;
  // Replacing the slot is synchronous, so concurrent calls cannot leave two running.
  app.liveTask?.abort.abort();
  const task: LiveTask = { abort: new AbortController(), finished: false };
  app.liveTask = task;
  app.spawn(
    run(app, generation, task).finally(() => {
      task.finished = true;
    }),
  );
}

async function run(app: AppState, generation: number, task: LiveTask): Promise<void> {
  const signal = task.abort.signal;
  const current = () => !signal.aborted && app.liveGeneration === generation;
  const pending: Pending = { scheduled: false, changes: new Changes() };
  let backoff = 2000;
  while (current()) {
    let server;
    try {
      server = app.server();
    } catch {
      return;
    }
    const list = await until(server.activities().catch(() => null), signal);
    if (list === ABORTED || !current()) return;
    if (list) {
      app.setConnected(true);
      app.setActivities(list.map(activityView).filter((a): a is ServerActivity => a !== null));
    }
    let opened;
    try {
      opened = await until(server.notifications([], signal), signal);
      if (opened === ABORTED || !current()) return;
      app.setConnected(true);
      backoff = 2000;
      const stream = opened[Symbol.asyncIterator]();
      let lastSessions = Date.now() - 5000;
      try {
        for (;;) {
          let n;
          try {
            n = await until(stream.next(), signal);
          } catch {
            break;
          }
          if (n === ABORTED || !current()) return;
          if (n.done) break;
          lastSessions = handle(app, n.value.event, n.value.data, pending, lastSessions);
        }
      } finally {
        // Closes the connection of a stream that ended or was abandoned.
        void Promise.resolve(stream.return?.()).catch(() => {});
      }
    } catch (e) {
      if (!current()) return;
      log.debug(`notifications unavailable: ${(e as Error).message}`);
    }
    if (!current()) return;
    const reachable = await until(
      server.capabilities().then(
        () => true,
        () => false,
      ),
      signal,
    );
    if (reachable === ABORTED || !current()) return;
    app.setConnected(reachable);
    await sleep(backoff, signal);
    backoff = Math.min(backoff * 2, 60_000);
  }
}

/** Stop listening (signing out). */
export function stop(app: AppState): void {
  app.liveGeneration += 1;
  const old = app.liveTask;
  app.liveTask = null;
  old?.abort.abort();
  app.setActivities([]);
}

/** Changes collected while a refetch is scheduled. */
interface Pending {
  scheduled: boolean;
  changes: Changes;
}

/** Whether a notification listener is running. */
export function listening(app: AppState): boolean {
  return app.liveTask !== null && !app.liveTask.finished;
}

type Json = Record<string, unknown>;

const isObj = (v: unknown): v is Json => typeof v === "object" && v !== null && !Array.isArray(v);

/** Handles one notification; returns the (possibly updated) time sessions were last announced. */
export function handle(app: AppState, event: string, data: unknown, pending: Pending, lastSessions: number): number {
  switch (event) {
    case "timeline": {
      // Items added, updated or deleted in a library. State 5 is "done", 9 "deleted".
      for (const e of entries(data, "TimelineEntry")) {
        const state = typeof e.state === "number" ? e.state : null;
        const library = e.identifier === "com.plexapp.plugins.library";
        const changed = library && (state === 5 || state === 9 || e.metadataState === "deleted");
        if (changed) bumpSoon(app, pending, text(e, "sectionID"), text(e, "itemID"));
      }
      break;
    }
    case "activity": {
      for (const n of entries(data, "ActivityNotification")) {
        if (!isObj(n.Activity)) continue;
        let activity: Activity;
        try {
          activity = normalizeActivity(n.Activity);
        } catch {
          continue;
        }
        const view = activityView(activity);
        if (!view) continue;
        const ended = n.event === "ended";
        const library = view.kind.startsWith("library.") || view.kind.startsWith("media.");
        const section = view.sectionId;
        app.updateActivities((list) => {
          const kept = list.filter((a) => a.uuid !== view.uuid);
          if (!ended) kept.push(view);
          list.splice(0, list.length, ...kept);
        });
        if (ended && library) {
          bumpSoon(app, pending, section, null);
          app.spawn(session.reloadSections(app));
        }
      }
      break;
    }
    case "playing":
    case "transcodeSession.start":
    case "transcodeSession.end":
    case "transcodeSession.update": {
      if (Date.now() - lastSessions >= 1000) {
        lastSessions = Date.now();
        app.emit({ kind: "sessionsChanged" });
      }
      // Something stopped playing (here or on another device): watch state may have changed.
      for (const n of entries(data, "PlaySessionStateNotification")) {
        if (n.state === "stopped") bumpSoon(app, pending, text(n, "librarySectionID"), text(n, "ratingKey"));
      }
      break;
    }
    default:
      break;
  }
  return lastSessions;
}

/** A string or number field as a string. */
export function text(v: unknown, key: string): string | null {
  if (!isObj(v)) return null;
  const x = v[key];
  if (typeof x === "string" && x !== "") return x;
  if (typeof x === "number") return String(x);
  return null;
}

/**
 * Coalesce bursts of changes (a scan adds many items) into one refetch that
 * names every section and item touched. A change with no section means "anywhere".
 */
function bumpSoon(app: AppState, pending: Pending, section: string | null, item: string | null): void {
  pending.changes.add(section, item);
  if (pending.scheduled) return;
  pending.scheduled = true;
  app.spawn(
    new Promise<void>((resolve) => {
      setTimeout(() => {
        pending.scheduled = false;
        const changes = pending.changes;
        pending.changes = new Changes();
        app.bumpWith(changes);
        resolve();
      }, 1500);
    }),
  );
}

/**
 * Notification payloads arrive as `{Key: [...]}` or `{Key: {...}}`, sometimes
 * wrapped in a `NotificationContainer`.
 */
export function entries(data: unknown, key: string): Json[] {
  if (!isObj(data)) return [];
  const inner = data.NotificationContainer ?? data;
  if (!isObj(inner)) return [];
  const v = inner[key];
  if (Array.isArray(v)) return v as Json[];
  if (isObj(v)) return [v];
  return [];
}

export function activityView(a: Activity): ServerActivity | null {
  if (a.uuid === null) return null;
  const context = isObj(a.Context) ? a.Context : null;
  const section = context ? context.librarySectionID : undefined;
  return {
    uuid: a.uuid,
    kind: a.type ?? "",
    title: a.title ?? "",
    subtitle: a.subtitle !== null && a.subtitle !== "" ? a.subtitle : null,
    progress: a.progress ?? 0,
    cancellable: a.cancellable,
    sectionId: section === undefined || section === null ? null : typeof section === "string" ? section : JSON.stringify(section),
  };
}
