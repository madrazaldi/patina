// Local resume points and an account/server-scoped outbox for offline watches.
// Port of crates/core/src/offline.rs.

import { createHash, randomUUID } from "node:crypto";
import { join } from "node:path";

import { PlexError } from "../plex";
import { Metadata } from "../plex";
import * as config from "./config";
import type { Download } from "./downloads";
import { sameServer } from "./state";
import type { AppState, Session } from "./state";
import * as storage from "./storage";

/** One entry of `offline-progress.json` (snake_case on disk, like the Rust struct). */
export interface Progress {
  id: string;
  account: string;
  server: string;
  rating_key: string;
  position: number;
  duration: number;
  watched: boolean;
  updated: number;
  revision: string;
  pending: boolean;
}

/** Unix time in seconds. */
export function now(): number {
  return Math.floor(Date.now() / 1000);
}

export function digest(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

export function account(session: Session): string {
  return session.user?.uuid ?? digest(session.server.token());
}

function path(): string {
  return join(config.configDir(), "offline-progress.json");
}

const emptyList = (): Progress[] => [];

/** The resume position of a download: the local one when there is one, else the server's. */
export function position(download: Download): number {
  const p = readList().find((p) => p.id === download.id);
  if (p) return p.watched ? 0 : p.position;
  return download.item.viewOffset ?? 0;
}

function readList(): Progress[] {
  try {
    return storage.readJson<Progress[]>(path(), emptyList);
  } catch {
    return [];
  }
}

/** Downloads finished offline whose watch isn't on the server yet. */
export function watchedOffline(): Set<string> {
  return new Set(readList().filter((p) => p.watched && p.pending).map((p) => p.id));
}

export function record(download: Download, positionMs: number, duration: number, completed: boolean, threshold: number): void {
  if (duration <= 0) return;
  const watched = completed || positionMs / duration >= threshold;
  const progress: Progress = {
    id: download.id,
    account: download.account,
    server: download.serverId,
    rating_key: Metadata.ratingKey(download.item),
    position: watched ? 0 : Math.max(positionMs, 0),
    duration,
    watched,
    updated: now(),
    revision: randomUUID(),
    pending: download.account !== "",
  };
  storage.updateJson<Progress[], void>(path(), emptyList, (list) => {
    const kept = list.filter((p) => p.id !== progress.id);
    list.splice(0, list.length, ...kept, progress);
  });
}

export function belongs(p: Progress, server: string, account: string): boolean {
  return p.pending && p.server === server && p.account === account;
}

export function acknowledge(list: Progress[], sent: Progress): void {
  // A playback checkpoint written while the request was in flight stays pending.
  const p = list.find((p) => p.id === sent.id && p.revision === sent.revision);
  if (p) p.pending = false;
}

export async function sync(app: AppState): Promise<void> {
  const session = app.session();
  if (!session) return;
  // Wait until local playback ends so partial checkpoints don't race its final watch state.
  if (app.nowPlaying()?.local) return;
  const lease = storage.tryLock(join(config.configDir(), "offline-sync"));
  if (!lease) return;
  try {
    const acct = account(session);
    const list = storage.readJson<Progress[]>(path(), emptyList);
    let firstError: Error | undefined;
    const stillCurrent = () => {
      const s = app.session();
      return s !== null && sameServer(s.server, session.server);
    };
    for (const p of list.filter((p) => belongs(p, session.serverId, acct))) {
      if (!stillCurrent()) break;
      let item: Metadata | null;
      try {
        item = await session.server.metadata(p.rating_key, {});
      } catch (e) {
        if (e instanceof PlexError && (e.status === 404 || e.status === 410)) {
          item = null;
        } else {
          firstError ??= e as Error;
          continue;
        }
      }
      if (!item) {
        // Deleted titles keep their local resume point, but must not block the outbox.
        storage.updateJson<Progress[], void>(path(), emptyList, (l) => acknowledge(l, p));
        continue;
      }
      // A newer online watch wins; replay must never rewind another device's newer progress.
      const serverIsNewer = (item.lastViewedAt ?? 0) > p.updated;
      const serverPosition = item.viewOffset ?? 0;
      const serverWatched = Metadata.isWatched(item);
      if (!serverIsNewer) {
        try {
          if (p.watched) {
            await session.server.scrobble(p.rating_key);
          } else {
            await session.server.timeline({
              ratingKey: p.rating_key,
              key: item.key ?? `/library/metadata/${p.rating_key}`,
              state: "stopped",
              timeMs: p.position,
              durationMs: p.duration,
              playQueueItemId: null,
              session: null,
            });
          }
        } catch (e) {
          firstError ??= e as Error;
          continue;
        }
      }
      storage.updateJson<Progress[], void>(path(), emptyList, (l) => {
        acknowledge(l, p);
        if (serverIsNewer) {
          const local = l.find((x) => x.id === p.id && x.revision === p.revision);
          if (local) {
            local.position = serverPosition;
            local.watched = serverWatched;
          }
        }
      });
      app.bump();
    }
    if (firstError) throw firstError;
  } finally {
    lease.release();
  }
}
