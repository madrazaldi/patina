// The webview's whole API: one handler per command, named and shaped like the
// Tauri commands this replaced (`web/src/lib/api.ts` is the caller). Plex models
// are returned as they serialize (Plex's own JSON field names); tokens never
// cross the boundary. Handlers throw to reject; the message reaches the UI.

import { mkdirSync } from "node:fs";
import * as config from "../core/config";
import * as controls from "../core/controls";
import * as downloads from "../core/downloads";
import * as failure from "../core/failure";
import * as livetv from "../core/livetv";
import * as lyrics from "../core/lyrics";
import * as edit from "../core/ops/edit";
import * as library from "../core/ops/library";
import * as lists from "../core/ops/lists";
import * as pick from "../core/ops/pick";
import * as playback from "../core/ops/playback";
import * as server from "../core/ops/server";
import * as setup from "../core/ops/setup";
import * as xsearch from "../core/ops/xsearch";
import * as player from "../core/player";
import * as session from "../core/session";
import * as servers from "../core/servers";
import * as tracks from "../core/tracks";
import * as undo from "../core/undo";
import type { AppState } from "../core/state";
import { LibraryQuery, MetadataOptions, Params, normalizeMetadata } from "../plex";
import { autoCommands } from "./auto";
import type { Ctx, Handler } from "./types";

export type { Ctx, Handler, Host } from "./types";

const clean = <T>(v: T | null | undefined): T | undefined => (v == null ? undefined : v);

const handlers: Record<string, Handler> = {
  // App --------------------------------------------------------------------
  snapshot: (app) => app.snapshot(),
  info: () => setup.info(),
  settings: (app) => {
    const s = app.settings();
    return { player: s.player, ui: s.ui };
  },
  set_player_settings: (app, a) => app.tryUpdateSettings((s) => config.patch(s.player, a.player)),
  set_ui_settings: (app, a) => app.tryUpdateSettings((s) => config.patch(s.ui, a.ui)),
  probe_mpv: (app) => setup.probeMpv(app),
  open_url: (_app, a, ctx) => ctx.host.openUrl(a.url),
  open_mpv_config: (_app, _a, ctx) => {
    const dir = config.mpvConfigDir();
    mkdirSync(dir, { recursive: true });
    return ctx.host.openPath(dir);
  },

  // Session ----------------------------------------------------------------
  retry: async (app) => void (await session.retry(app)),
  /** Start a PIN sign-in; strong PINs open the browser straight away. */
  start_pin: async (app, a, ctx) => {
    const pin = await session.createPin(app, a.strong);
    if (a.strong) void ctx.host.openUrl(pin.url).catch(() => {});
    return { id: pin.id, code: pin.code, url: pin.url, attempt: pin.attempt };
  },
  /** Wait for a PIN to be approved and sign in. Resolves early if cancelled. */
  complete_pin: (app, a) => session.completePin(app, a.id, a.attempt),
  cancel_pin: (app) => session.cancelPin(app),
  password_sign_in: (app, a) => session.passwordSignIn(app, a.login, a.password, clean(a.code)),
  sign_in_manual: (app, a) => session.signInManual(app, a.url, a.token),
  pick_server: async (app, a) => void (await session.run(app, session.pickServer(app, a.id))),
  switch_server: (app, a) => session.switchServer(app, a.id),
  switch_user: (app, a) => session.switchUser(app, a.uuid, clean(a.pin)),
  sign_out: (app) => session.signOut(app),
  reload_sections: (app) => session.reloadSections(app),
  home_users: async (app) => {
    const tv = app.tv();
    return tv ? await tv.homeUsers() : [];
  },

  // Browsing ---------------------------------------------------------------
  home_hubs: (app) => app.server().homeHubs(20),
  section_hubs: (app, a) => app.server().sectionHubs(a.id, 18),
  section_sorts: (app, a) => app.server().sectionSorts(a.id),
  section_filter_values: (app, a) => app.server().sectionFilterValues(a.id, a.filter, clean(a.kind)),
  section_items: (app, a) =>
    app.server().sectionItems(
      a.id,
      new LibraryQuery({
        kind: clean(a.kind),
        sort: clean(a.sort),
        unwatched: !!a.unwatched,
        filters: Params.from(a.filters ?? []),
        start: a.start,
        size: a.size,
      }),
    ),
  collections: (app, a) => app.server().collections(a.id),
  /** Any server listing by key: "See all" for hubs, tag filters (genre, actor, ...). */
  listing: (app, a) =>
    app.server().fetch(a.path, new Params().set("X-Plex-Container-Start", a.start).set("X-Plex-Container-Size", a.size)),
  search: (app, a) => app.server().searchHubs(a.query, 24, undefined),
  /** `server` is the machine id of another server of the account (see `search_all_servers`). */
  metadata: async (app, a) => {
    const on = clean<string>(a.server);
    const found = await (await servers.serverFor(app, on)).metadata(a.ratingKey, MetadataOptions.full());
    if (!found) return null;
    servers.tag(app, [found], on);
    return found;
  },
  /** Seasons, episodes, albums, tracks or collection items of an item. */
  children: async (app, a) => {
    const on = clean<string>(a.server);
    const s = await servers.serverFor(app, on);
    const mc = a.collection ? await s.collectionItems(a.ratingKey) : await s.children(a.ratingKey);
    const items = mc.Metadata;
    servers.tag(app, items, on);
    return items;
  },
  /** Rating key of the local copy of a plex.tv item, if the server has one. */
  find_by_guid: async (app, a) => {
    const found = await app.server().findByGuid(a.guid);
    return found[0]?.ratingKey ?? null;
  },

  // Item actions -----------------------------------------------------------
  /** Mark an item (or a whole show, season or album) watched or unwatched; announces itself with a toast that offers Undo. */
  set_watched: (app, a) => undo.setWatched(app, clean(a.server), [a.ratingKey], a.watched),
  set_watched_many: (app, a) => undo.setWatched(app, null, a.ratingKeys, a.watched),
  rate: (app, a) => app.server().rate(a.ratingKey, a.rating ?? null),
  /** Choose the audio and/or subtitle stream of a part (0 turns subtitles off). */
  select_streams: async (app, a) =>
    (await servers.serverFor(app, clean(a.server))).selectStreams(a.partId, clean(a.audio), clean(a.subtitle), true),
  refresh_metadata: (app, a) => app.server().refreshMetadata(a.ratingKey, true),
  analyze_metadata: (app, a) => app.server().analyzeMetadata(a.ratingKey),
  remove_from_continue_watching: (app, a) => undo.removeFromContinueWatching(app, a.ratingKey),
  /** Forget the resume position without changing the watched state. */
  clear_progress: (app, a) => undo.clearProgress(app, a.ratingKey),
  delete_item: (app, a) => app.server().deleteMetadata(a.ratingKey),
  /** Edit fields (locking them so refreshes keep the change) and tags. */
  edit_metadata: (app, a) => edit.editMetadata(app, a.ratingKey, a.fields, a.tags),
  /** Poster or background choices (`kind` is `poster` or `art`). */
  artwork: (app, a) => (a.kind === "art" ? app.server().arts(a.ratingKey) : app.server().posters(a.ratingKey)),
  set_artwork: (app, a) =>
    a.kind === "art" ? app.server().setArt(a.ratingKey, a.url) : app.server().setPoster(a.ratingKey, a.url),
  matches: (app, a) => {
    const title = typeof a.title === "string" && a.title.trim() !== "" ? a.title : undefined;
    return app.server().matches(a.ratingKey, title, clean(a.year), undefined);
  },
  apply_match: (app, a) => app.server().applyMatch(a.ratingKey, a.guid, clean(a.name)),
  unmatch: (app, a) => app.server().unmatch(a.ratingKey),
  search_subtitles: (app, a) =>
    app.server().searchSubtitles(a.ratingKey, a.language, !!a.hearingImpaired, !!a.forced),
  download_subtitle: (app, a) => app.server().downloadSubtitle(a.ratingKey, a.key),
  delete_stream: (app, a) => app.server().deleteStream(a.id),

  // Watchlist --------------------------------------------------------------
  watchlist: (app) => lists.watchlist(app),
  watchlisted: (app, a) => lists.watchlisted(app, a.discoverId),
  set_watchlisted: async (app, a) => {
    const tv = app.plexTv();
    if (a.on) await tv.addToWatchlist(a.discoverId);
    else await tv.removeFromWatchlist(a.discoverId);
  },

  // Playlists --------------------------------------------------------------
  /** All playlists, or the non-smart ones of one kind (`video`, `audio`, `photo`). */
  playlists: (app, a) => app.server().playlists(clean(a.kind), a.kind != null ? false : undefined),
  playlist: (app, a) => lists.playlist(app, a.id),
  /** Add an item to an existing playlist, or to a new one called `title`. */
  add_to_playlist: (app, a) => lists.addToPlaylist(app, a.ratingKey, clean(a.playlist), clean(a.title), a.kind),
  remove_from_playlist: (app, a) => undo.removeFromPlaylist(app, a.id, a.itemId),
  delete_playlist: (app, a) => app.server().deletePlaylist(a.id),
  move_playlist_item: (app, a) => app.server().movePlaylistItem(a.id, a.itemId, clean(a.after)),
  update_playlist: (app, a) => app.server().updatePlaylist(a.id, String(a.title).trim(), String(a.summary).trim()),
  clear_playlist: (app, a) => app.server().clearPlaylist(a.id),

  // Playback ---------------------------------------------------------------
  play: (app, a) => void player.play(app, player.parsePlayRequest(a.request)),
  control: (_app, a) => void player.control(a.command),
  /** Volume, speed, chapters, repeat and the sleep timer. */
  tune: (app, a) => void player.tune(app, a.command),
  /** The playing item's controls and diagnostics; also pushed as `playback-state`. */
  playback_state: () => controls.playbackState() ?? null,
  /** The playback failure waiting for a decision; also pushed as `playback-failure`. */
  playback_failure: () => failure.current() ?? null,
  recover_playback: (app, a) => failure.recover(app, a.action),
  dismiss_failure: (app) => failure.clear(app),
  /** The queue saved by the last playback, to offer a Resume. */
  saved_queue: (app) => playback.savedQueue(app) ?? null,
  resume_saved: (app) => playback.resumeSaved(app),
  forget_saved: (app) => playback.forgetSaved(app),
  queue: (app, a) =>
    void player.queue(app, a.command?.kind === "add" ? { ...a.command, item: normalizeMetadata(a.command.item) } : a.command),
  queue_items: (app, a) => player.queueItems(app, (a.items as unknown[]).map((m) => normalizeMetadata(m))),
  lyrics: async (app, a) => (await lyrics.load(app.server(), a.ratingKey)) ?? null,

  // Bulk audio & subtitles ---------------------------------------------------
  tracks_load: (app, a) => tracks.load(app, a.show),
  tracks_view: (app, a) => tracks.view(app, a.query),
  tracks_apply: (app, a, ctx) => {
    const progress = ctx.channel<[number, number]>(a.progress);
    return tracks.apply(app, a.changes, (done: number, total: number) => progress.send([done, total]));
  },

  // Activity -----------------------------------------------------------------
  sessions: (app) => app.server().sessions(),
  terminate_session: (app, a) => app.server().terminateSession(a.id, "Stopped by the server owner."),
  history: (app, a) => lists.history(app, a.page),
  /** Delete a history entry by its `historyKey`. */
  delete_history: (app, a) => lists.deleteHistory(app, a.key),
  tasks: (app) => server.tasks(app),
  cancel_activity: (app, a) => app.server().cancelActivity(a.uuid),
  kill_transcode: (app, a) => server.killTranscode(app, a.key),

  // Server administration ----------------------------------------------------
  server_overview: (app) => server.serverOverview(app),
  check_for_updates: (app) => app.server().checkForUpdates(true),
  apply_update: (app, a) => app.server().applyUpdate(!!a.tonight, false),
  /** Library maintenance: `scan`, `refresh` (re-download metadata), `analyze` or `emptyTrash`. */
  section_action: async (app, a) => {
    const s = app.server();
    switch (a.action) {
      case "scan": return void (await s.refreshSection(a.id, false, undefined));
      case "refresh": return void (await s.refreshSection(a.id, true, undefined));
      case "analyze": return void (await s.analyzeSection(a.id));
      case "emptyTrash": return void (await s.emptyTrash(a.id));
      default: throw new Error(`Unknown library action ${a.action}.`);
    }
  },
  /** Server maintenance: `scanAll`, `cleanBundles` or `optimizeDatabase`. */
  server_action: async (app, a) => {
    const s = app.server();
    switch (a.action) {
      case "scanAll": return void (await s.refreshAllSections(false));
      case "cleanBundles": return void (await s.cleanBundles());
      case "optimizeDatabase": return void (await s.optimizeDatabase());
      default: throw new Error(`Unknown server action ${a.action}.`);
    }
  },
  maintenance: (app) => server.maintenance(app),
  butler_task: async (app, a) => {
    const s = app.server();
    if (a.start) await s.startButlerTask(a.name);
    else await s.stopButlerTask(a.name);
  },
  prefs: (app) => server.settings(app),
  set_pref: (app, a) => app.server().setPref(a.id, a.value),
  users_devices: (app) => server.usersDevices(app),

  // Library browsing: filters, jump bar, folders, related -------------------
  /** Filters for a section and media type (genre, year, resolution, unwatched, ...). */
  section_filters: (app, a) => library.sectionFilters(app, a.id, clean(a.kind)),
  first_characters: (app, a) => app.server().sectionFirstCharacters(a.id, clean(a.kind)),
  section_folders: (app, a) => library.sectionFolders(app, a.id, clean(a.key)),
  /** Extra hubs for a detail page, plus sonically similar music. */
  item_hubs: async (app, a) => {
    const on = clean<string>(a.server);
    if (servers.isCurrent(app, on)) return library.itemHubs(app, a.ratingKey, a.kind);
    const hubs = await library.itemHubsOn(await servers.serverFor(app, on), a.ratingKey, a.kind);
    for (const hub of hubs) servers.tag(app, hub.Metadata, on);
    return hubs;
  },

  // Undo -------------------------------------------------------------------
  /** What Cmd/Ctrl+Z would undo, if anything. */
  undo_last: (app) => {
    const last = undo.last(app);
    return last ? { id: last[0], label: last[1] } : null;
  },
  /** Undo entry `id` (a toast's Undo button), or the most recent one. */
  undo_action: (app, a) => undo.undo(app, clean(a.id)),

  // Pick something to watch --------------------------------------------------
  pick_options: (app) => pick.options(app),
  pick_something: (app, a) => pick.pick(app, pick.pickQueryFrom(a.query)),

  // Search across servers ------------------------------------------------------
  /** Search every server of the account; `progress` gets the results so far after each server answers. */
  search_all_servers: (app, a, ctx) => {
    const progress = ctx.channel<xsearch.Results>(a.progress);
    return xsearch.searchAll(app, a.query, (r: xsearch.Results) => progress.send(r));
  },

  // Collections --------------------------------------------------------------
  /** Add an item to a collection, or to a new one called `title`. */
  add_to_collection: (app, a) => edit.addToCollection(app, a.ratingKey, clean(a.collection), clean(a.title)),
  remove_from_collection: (app, a) => app.server().removeFromCollection(a.id, a.ratingKey),
  move_collection_item: (app, a) => app.server().moveCollectionItem(a.id, a.ratingKey, clean(a.after)),
  delete_collection: (app, a) => app.server().deleteCollection(a.id),
  /** Collection display settings: `mode` (-1..2) and/or `sort` (0..2). */
  collection_settings: async (app, a) => {
    const s = app.server();
    if (a.mode != null) await s.setCollectionMode(a.id, a.mode);
    if (a.sort != null) await s.setCollectionSort(a.id, a.sort);
  },

  // Downloads ----------------------------------------------------------------
  downloads: () => downloads.list(),
  download: (app, a) => downloads.enqueue(app, a.ratingKey),
  remove_download: (app, a) => downloads.remove(app, a.id),
  pause_download: (app, a) => downloads.pause(app, a.id),
  retry_download: (app, a) => downloads.retry(app, a.id),
  play_downloads: (app, a) => playback.playDownloads(app, a.ids),
  open_downloads_folder: (app, _a, ctx) => {
    const dir = config.downloadsDir(app.settings().player);
    mkdirSync(dir, { recursive: true });
    return ctx.host.openPath(dir);
  },

  // Live TV ------------------------------------------------------------------
  livetv_available: async (app) => livetv.available(app.server()),
  livetv_guide: (app, a) => livetv.guide(app.server(), Math.min(24, Math.max(1, Number(a.hours)))),
  livetv_watch: (app, a) => playback.livetvWatch(app, a.dvr, a.channel, a.title),
  recordings: (app) => livetv.recordings(app.server()),
  record: (app, a) => livetv.record(app.server(), a.guid),
  delete_recording_rule: (app, a) => livetv.deleteRule(app.server(), a.id),

  // Server administration: libraries, stats, diagnostics ---------------------
  /** Agents and scanners for a new library of `kind` (`movie`, `show`, `artist`, `photo`). */
  library_choices: (app, a) => server.libraryChoices(app, a.kind),
  create_library: (app, a) => server.createLibrary(app, a.form),
  /** Rename a library and/or change its folders. */
  edit_library: (app, a) => server.editLibrary(app, a.id, a.name, a.locations),
  delete_library: (app, a) => server.deleteLibrary(app, a.id),
  cancel_scan: (app, a) => app.server().cancelRefreshSection(a.id),
  library_prefs: (app, a) => server.libraryPrefs(app, a.id),
  set_library_pref: (app, a) => app.server().setSectionPrefs(a.id, new Params().set(a.pref, a.value)),
  butler_all: async (app, a) => {
    const s = app.server();
    if (a.start) await s.startAllButlerTasks();
    else await s.stopAllButlerTasks();
  },
  /** Bandwidth and plays since `since` (unix seconds). `timespan` is Plex's bucket size code: 6 for hours, 4 for days. */
  server_stats: (app, a) => server.serverStats(app, a.timespan, a.since),
  /** Save the server's logs (`logs`) or database (`database`) to the downloads folder and reveal it. Returns the file path. */
  save_diagnostics: async (app, a, ctx) => {
    const path = await server.saveDiagnostics(app, a.what);
    const dir = path.replace(/[\\/][^\\/]*$/, "");
    if (dir && dir !== path) void ctx.host.openPath(dir).catch(() => {});
    return path;
  },

  ...autoCommands,
};

/** Every command name the webview may call. */
export const commandNames = Object.keys(handlers);

/** Run one command. Rejects with the handler's error. */
export async function dispatch(app: AppState, cmd: string, args: Record<string, unknown> | undefined, ctx: Ctx): Promise<unknown> {
  const handler = handlers[cmd];
  if (!handler) throw new Error(`Unknown command ${cmd}.`);
  const out = await handler(app, args ?? {}, ctx);
  return out === undefined ? null : out;
}
