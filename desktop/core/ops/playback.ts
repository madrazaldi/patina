// Starting playback from things that aren't library items.

import * as downloads from "../downloads";
import * as livetv from "../livetv";
import * as offline from "../offline";
import * as player from "../player";
import { Scope, type SavedQueueView, clearQueue, loadQueue, savedQueueView } from "../recovery";
import type { AppState } from "../state";
import { logger } from "../log";
import { errorText } from "./util";

const log = logger("playback");

/** Play downloads offline, in the order given. */
export function playDownloads(app: AppState, ids: string[]): void {
  const files = downloads.files(ids);
  const s = app.session();
  const account = s ? offline.account(s) : app.settings().auth.account_id;
  if (files.some((d) => d.account !== "" && d.account !== account)) {
    throw new Error("These downloads belong to another Plex profile. Switch to that profile to play them.");
  }
  if (files.length === 0) throw new Error("The downloaded file is missing.");
  player.playFiles(app, files);
}

/** Tune a Live TV channel and play it; `title` names it if the tuner doesn't. */
export async function livetvWatch(app: AppState, dvr: string, channel: string, title: string): Promise<void> {
  const item = await livetv.tune(app.server(), dvr, channel);
  if (item.title === null) item.title = title;
  player.play(app, player.playRequest({ item, live: true, resume: false }));
}

/** The queue that was playing when Patina last stopped (for this account and server), to offer a Resume. */
export function savedQueue(app: AppState): SavedQueueView | null {
  const s = app.session();
  if (!s) return null;
  const saved = loadQueue(Scope.of(s));
  return saved ? savedQueueView(saved) : null;
}

/** Play the saved queue again from where it stopped. */
export function resumeSaved(app: AppState): void {
  const s = app.session();
  if (!s) throw new Error("Not connected to a server.");
  const saved = loadQueue(Scope.of(s));
  if (!saved) throw new Error("There is no saved queue to resume.");
  const item = saved.items[saved.current]!;
  const selection = saved.items.length > 1 ? [...saved.items] : [];
  player.playFrom(app, player.playRequest({ item, selection, resume: true, quality: saved.quality }), saved.position_ms);
}

/** Forget the saved queue. */
export function forgetSaved(app: AppState): void {
  const s = app.session();
  if (!s) return;
  try {
    clearQueue(Scope.of(s));
  } catch (e) {
    log.debug(`could not forget the saved queue: ${errorText(e)}`);
  }
}
