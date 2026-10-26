// Commands for automatic playback preferences and smart downloads. Both are
// scoped by account and server inside the core.

import * as prefs from "../core/prefs";
import { normalizeMetadata } from "../plex";
import * as smart from "../core/smart";
import type { Handler } from "./types";

export const autoCommands: Record<string, Handler> = {
  // Playback preferences -----------------------------------------------------
  /** The profile's rules, and a show's when `show` is given, with what applies. */
  playback_prefs: (app, a) => prefs.view(app, a.show ?? null),
  /** Replace the rules of the profile, or of a show; empty rules clear them. `show: null` is the profile, as stored. */
  set_playback_prefs: (app, a) => prefs.save(app, a.show ?? null, a.title ?? null, a.rules),
  /** Shows that have rules of their own. */
  playback_prefs_shows: (app) => prefs.shows(app),
  /** Index of the version of an item the rules prefer. */
  preferred_version: (app, a) => prefs.preferredVersion(app, normalizeMetadata(a.item)),
  /** Remember the track dialog's choices as the show's rules. */
  tracks_remember: (app, a) => prefs.rememberTracks(app, a.query),

  // Smart downloads ------------------------------------------------------------
  smart_downloads: (app) => smart.overview(app),
  /** Keep the next `keep` unwatched items of a show, season or playlist downloaded. */
  set_smart_download: (app, a) => smart.setPolicy(app, a.ratingKey, a.keep, a.deleteWatched),
  update_smart_download: (app, a) => smart.updatePolicy(app, a.id, a.patch),
  remove_smart_download: (app, a) => smart.removePolicy(app, a.id, a.deleteDownloads),
  /** Limit all downloads together to this many bytes; none removes the limit. */
  set_download_budget: (app, a) => smart.setBudget(app, a.bytes ?? undefined),
  /** Bring downloads in line with the policies now. */
  run_smart_downloads: (app) => smart.reconcile(app),
};
