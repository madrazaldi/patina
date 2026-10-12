// Typed wrappers around the Rust commands in crates/app/src/commands.rs.
// Errors reject with the message string.

import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  AllResults,
  Artwork,
  PickOptions,
  PickQuery,
  PickResult,
  Download,
  FilterView,
  FolderView,
  LibraryChoices,
  LibraryForm,
  LiveChannel,
  Lyrics,
  Maintenance,
  MatchResult,
  Recordings,
  ServerStats,
  Stream,
  Directory,
  HistoryPage,
  HomeUser,
  Hub,
  Info,
  MediaContainer,
  Metadata,
  Overview,
  PartChange,
  PinView,
  PlaybackFailure,
  PlaybackState,
  PlayerSettings,
  PlaylistView,
  PlaybackPrefs,
  PlaybackRules,
  Pref,
  PrefGroup,
  Repeat,
  SavedQueueView,
  Settings,
  ShowRules,
  SmartOverview,
  SmartPolicy,
  SmartReport,
  Snapshot,
  Tasks,
  TrackView,
  UiSettings,
  UsersDevices,
} from "./types";

const call = <T = void>(cmd: string, args?: Record<string, unknown>) => invoke<T>(cmd, args);

export interface PlayOptions {
  resume?: boolean;
  shuffle?: boolean;
  mediaIndex?: number;
  /** Play the item's radio station (music). */
  radio?: boolean;
}

export type Control =
  | { kind: "togglePause" }
  | { kind: "play" }
  | { kind: "pause" }
  | { kind: "next" }
  | { kind: "previous" }
  | { kind: "stop" }
  | { kind: "seekRelative"; seconds: number }
  | { kind: "seekTo"; seconds: number };

/** Changes to the running player beyond transport. */
export type Tune =
  | { kind: "volume"; value: number }
  | { kind: "volumeBy"; delta: number }
  | { kind: "mute"; on?: boolean }
  | { kind: "speed"; value: number }
  | { kind: "speedBy"; delta: number }
  | { kind: "repeat"; mode: Repeat }
  | { kind: "cycleRepeat" }
  | { kind: "chapter"; index: number }
  | { kind: "chapterBy"; delta: number }
  | { kind: "sleepOff" }
  | { kind: "sleepMinutes"; minutes: number }
  | { kind: "sleepEndOfItem" }
  | { kind: "cycleSleep" };

/** What to do about a playback failure. */
export type Recover = { kind: "retry" } | { kind: "version"; index: number } | { kind: "lowerQuality" };

/** Edits to the running queue; entries are addressed by `QueueItem.uid`. */
export type QueueCommand =
  | { kind: "add"; item: Metadata; next: boolean }
  | { kind: "remove"; uid: number }
  | { kind: "move"; uid: number; before: number | null }
  | { kind: "jump"; uid: number }
  | { kind: "shuffle"; on: boolean }
  | { kind: "stopAfter"; on: boolean };

export interface TagEdit {
  /** "genre", "label", "collection", "mood", ... */
  tag: string;
  add: string[];
  remove: string[];
}

export const api = {
  // App
  snapshot: () => call<Snapshot>("snapshot"),
  info: () => call<Info>("info"),
  settings: () => call<Settings>("settings"),
  setPlayerSettings: (player: Partial<PlayerSettings>) => call("set_player_settings", { player }),
  setUiSettings: (ui: Partial<UiSettings>) => call("set_ui_settings", { ui }),
  probeMpv: () => call<string>("probe_mpv"),
  openUrl: (url: string) => call("open_url", { url }),
  openMpvConfig: () => call("open_mpv_config"),

  // Session
  retry: () => call("retry"),
  startPin: (strong: boolean) => call<PinView>("start_pin", { strong }),
  completePin: (id: number, attempt: number) => call("complete_pin", { id, attempt }),
  cancelPin: () => call("cancel_pin"),
  passwordSignIn: (login: string, password: string, code: string) =>
    call("password_sign_in", { login, password, code }),
  signInManual: (url: string, token: string) => call("sign_in_manual", { url, token }),
  pickServer: (id: string) => call("pick_server", { id }),
  switchServer: (id: string) => call("switch_server", { id }),
  switchUser: (uuid: string, pin?: string) => call("switch_user", { uuid, pin }),
  signOut: () => call("sign_out"),
  reloadSections: () => call("reload_sections"),
  homeUsers: () => call<HomeUser[]>("home_users"),

  // Browsing
  homeHubs: () => call<Hub[]>("home_hubs"),
  sectionHubs: (id: string) => call<Hub[]>("section_hubs", { id }),
  sectionSorts: (id: string) => call<Directory[]>("section_sorts", { id }),
  sectionFilterValues: (id: string, filter: string, kind?: number) =>
    call<Directory[]>("section_filter_values", { id, filter, kind }),
  /** `filters` are [filter, value] pairs, e.g. ["genre", "12"], ["year", "1999"], ["hdr", "1"]. */
  sectionItems: (
    id: string,
    q: { kind?: number; sort?: string; unwatched: boolean; filters: [string, string][]; start: number; size: number },
  ) => call<MediaContainer>("section_items", { id, ...q }),
  sectionFilters: (id: string, kind?: number) => call<FilterView[]>("section_filters", { id, kind }),
  firstCharacters: (id: string, kind?: number) => call<Directory[]>("first_characters", { id, kind }),
  /** Folder view; `key` is a directory key from a previous call, or none for the top. */
  sectionFolders: (id: string, key?: string) => call<FolderView>("section_folders", { id, key }),
  /** More hubs for a detail page (related, sonically similar, similar artists). */
  itemHubs: (ratingKey: string, kind: string, server?: string) => call<Hub[]>("item_hubs", { ratingKey, kind, server }),
  collections: (id: string) => call<Metadata[]>("collections", { id }),
  listing: (path: string, start: number, size: number) => call<MediaContainer>("listing", { path, start, size }),
  search: (query: string) => call<Hub[]>("search", { query }),
  /** `server` is the machine id of another account server the item was found on (Metadata.patinaServer). */
  metadata: (ratingKey: string, server?: string) => call<Metadata | null>("metadata", { ratingKey, server }),
  children: (ratingKey: string, collection = false, server?: string) => call<Metadata[]>("children", { ratingKey, collection, server }),
  findByGuid: (guid: string) => call<string | null>("find_by_guid", { guid }),

  // Item actions
  /** Core announces it with a toast that offers Undo. */
  setWatched: (ratingKey: string, watched: boolean, server?: string) => call("set_watched", { ratingKey, watched, server }),
  /** A selection; undone together. */
  setWatchedMany: (ratingKeys: string[], watched: boolean) => call("set_watched_many", { ratingKeys, watched }),
  /** What Cmd/Ctrl+Z would undo, if anything. */
  undoLast: () => call<{ id: number; label: string } | null>("undo_last"),
  /** Undo entry `id` (a toast's button), or the most recent one; resolves to what was undone. */
  undo: (id?: number) => call<string>("undo_action", { id }),

  // Pick something to watch
  pickOptions: () => call<PickOptions>("pick_options"),
  pick: (query: PickQuery) => call<PickResult>("pick_something", { query }),

  // Search across every server of the account; `onProgress` sees the results after each server answers.
  searchAllServers: (query: string, onProgress: (r: AllResults) => void) => {
    const progress = new Channel<AllResults>();
    progress.onmessage = onProgress;
    return call<AllResults>("search_all_servers", { query, progress });
  },
  rate: (ratingKey: string, rating: number | null) => call("rate", { ratingKey, rating }),
  selectStreams: (partId: number, audio?: number, subtitle?: number) =>
    call("select_streams", { partId, audio, subtitle }),
  refreshMetadata: (ratingKey: string) => call("refresh_metadata", { ratingKey }),
  analyzeMetadata: (ratingKey: string) => call("analyze_metadata", { ratingKey }),
  removeFromContinueWatching: (ratingKey: string) => call("remove_from_continue_watching", { ratingKey }),
  clearProgress: (ratingKey: string) => call("clear_progress", { ratingKey }),
  /** Deletes the item and its files (the server must allow media deletion). */
  deleteItem: (ratingKey: string) => call("delete_item", { ratingKey }),
  /** `fields` are [field, value] pairs: title, titleSort, originalTitle, summary, tagline, studio,
   *  contentRating, year, originallyAvailableAt. Edited fields are locked. */
  editMetadata: (ratingKey: string, fields: [string, string][], tags: TagEdit[] = []) =>
    call("edit_metadata", { ratingKey, fields, tags }),
  artwork: (ratingKey: string, kind: "poster" | "art") => call<Artwork[]>("artwork", { ratingKey, kind }),
  setArtwork: (ratingKey: string, kind: "poster" | "art", url: string) => call("set_artwork", { ratingKey, kind, url }),
  matches: (ratingKey: string, title?: string, year?: number) => call<MatchResult[]>("matches", { ratingKey, title, year }),
  applyMatch: (ratingKey: string, guid: string, name?: string) => call("apply_match", { ratingKey, guid, name }),
  unmatch: (ratingKey: string) => call("unmatch", { ratingKey }),
  searchSubtitles: (ratingKey: string, language: string, hearingImpaired = false, forced = false) =>
    call<Stream[]>("search_subtitles", { ratingKey, language, hearingImpaired, forced }),
  downloadSubtitle: (ratingKey: string, key: string) => call("download_subtitle", { ratingKey, key }),
  deleteStream: (id: number) => call("delete_stream", { id }),

  // Collections
  addToCollection: (ratingKey: string, target: { collection?: string; title?: string }) =>
    call("add_to_collection", { ratingKey, ...target }),
  removeFromCollection: (id: string, ratingKey: string) => call("remove_from_collection", { id, ratingKey }),
  /** Custom-sorted collections only; `after` null moves to the top. */
  moveCollectionItem: (id: string, ratingKey: string, after: string | null) =>
    call("move_collection_item", { id, ratingKey, after }),
  deleteCollection: (id: string) => call("delete_collection", { id }),
  /** mode: -1 library default, 0 hide collection, 1 hide items, 2 show both. sort: 0 release date, 1 title, 2 custom. */
  collectionSettings: (id: string, s: { mode?: number; sort?: number }) => call("collection_settings", { id, ...s }),

  // Watchlist
  watchlist: () => call<Metadata[]>("watchlist"),
  watchlisted: (discoverId: string) => call<boolean>("watchlisted", { discoverId }),
  setWatchlisted: (discoverId: string, on: boolean) => call("set_watchlisted", { discoverId, on }),

  // Playlists
  playlists: (kind?: string) => call<Metadata[]>("playlists", { kind }),
  playlist: (id: string) => call<PlaylistView | null>("playlist", { id }),
  addToPlaylist: (ratingKey: string, kind: string, target: { playlist?: string; title?: string }) =>
    call("add_to_playlist", { ratingKey, kind, ...target }),
  removeFromPlaylist: (id: string, itemId: number) => call("remove_from_playlist", { id, itemId }),
  deletePlaylist: (id: string) => call("delete_playlist", { id }),
  /** `after` null moves to the top. */
  movePlaylistItem: (id: string, itemId: number, after: number | null) => call("move_playlist_item", { id, itemId, after }),
  updatePlaylist: (id: string, title: string, summary: string) => call("update_playlist", { id, title, summary }),
  clearPlaylist: (id: string) => call("clear_playlist", { id }),

  // Playback
  play: (item: Metadata, opts: PlayOptions = {}) =>
    call("play", {
      request: {
        item,
        resume: opts.resume ?? true,
        shuffle: opts.shuffle ?? false,
        mediaIndex: opts.mediaIndex ?? null,
        radio: opts.radio ?? false,
      },
    }),
  control: (command: Control) => call("control", { command }),
  /** Volume, speed, chapters, repeat and the sleep timer. */
  tune: (command: Tune) => call("tune", { command }),
  playbackState: () => call<PlaybackState | null>("playback_state"),
  playbackFailure: () => call<PlaybackFailure | null>("playback_failure"),
  recoverPlayback: (action: Recover) => call("recover_playback", { action }),
  dismissFailure: () => call("dismiss_failure"),
  /** The queue saved by the last playback, to offer a Resume. */
  savedQueue: () => call<SavedQueueView | null>("saved_queue"),
  resumeSaved: () => call("resume_saved"),
  forgetSaved: () => call("forget_saved"),
  /** Adding while nothing plays starts playback. */
  queueItems: (items: Metadata[]) => call("queue_items", { items }),
  queue: (command: QueueCommand) => call("queue", { command }),
  lyrics: (ratingKey: string) => call<Lyrics | null>("lyrics", { ratingKey }),

  // Downloads
  downloads: () => call<Download[]>("downloads"),
  /** Queue an item (or every episode/track below it); resolves to how many were added. */
  download: (ratingKey: string) => call<number>("download", { ratingKey }),
  removeDownload: (id: string) => call("remove_download", { id }),
  pauseDownload: (id: string) => call("pause_download", { id }),
  retryDownload: (id: string) => call("retry_download", { id }),
  playDownloads: (ids: string[]) => call("play_downloads", { ids }),
  openDownloadsFolder: () => call("open_downloads_folder"),

  // Live TV
  livetvAvailable: () => call<boolean>("livetv_available"),
  livetvGuide: (hours: number) => call<LiveChannel[]>("livetv_guide", { hours }),
  livetvWatch: (dvr: string, channel: string, title: string) => call("livetv_watch", { dvr, channel, title }),
  recordings: () => call<Recordings>("recordings"),
  record: (guid: string) => call("record", { guid }),
  deleteRecordingRule: (id: string) => call("delete_recording_rule", { id }),

  // Bulk audio & subtitles
  tracksLoad: (show: string) => call("tracks_load", { show }),
  tracksView: (query: { show: string; season: number | null; reference: string | null; audio: number | null; subtitle: string }) =>
    call<TrackView>("tracks_view", { query }),
  tracksApply: (changes: PartChange[], onProgress: (done: number, total: number) => void) => {
    const progress = new Channel<[number, number]>();
    progress.onmessage = ([done, total]) => onProgress(done, total);
    return call("tracks_apply", { changes, progress });
  },

  // Automatic playback preferences (per profile, and per show)
  playbackPrefs: (show: string | null) => call<PlaybackPrefs>("playback_prefs", { show }),
  setPlaybackPrefs: (show: string | null, title: string | null, rules: PlaybackRules) =>
    call("set_playback_prefs", { show, title, rules }),
  playbackPrefsShows: () => call<ShowRules[]>("playback_prefs_shows"),
  /** Index of the version (Media) the rules prefer. */
  preferredVersion: (item: Metadata) => call<number>("preferred_version", { item }),
  /** Save what the track dialog chose as the show's rules. */
  tracksRemember: (query: { show: string; season: number | null; reference: string | null; audio: number | null; subtitle: string }) =>
    call("tracks_remember", { query }),

  // Smart downloads
  smartDownloads: () => call<SmartOverview>("smart_downloads"),
  setSmartDownload: (ratingKey: string, keep: number, deleteWatched: boolean) =>
    call<SmartPolicy>("set_smart_download", { ratingKey, keep, deleteWatched }),
  updateSmartDownload: (id: string, patch: { keep?: number; deleteWatched?: boolean; enabled?: boolean }) =>
    call("update_smart_download", { id, patch }),
  removeSmartDownload: (id: string, deleteDownloads: boolean) => call("remove_smart_download", { id, deleteDownloads }),
  /** Bytes all downloads together may use; null removes the limit. */
  setDownloadBudget: (bytes: number | null) => call("set_download_budget", { bytes }),
  runSmartDownloads: () => call<SmartReport>("run_smart_downloads"),

  // Activity
  sessions: () => call<Metadata[]>("sessions"),
  terminateSession: (id: string) => call("terminate_session", { id }),
  history: (page: number) => call<HistoryPage>("history", { page }),
  deleteHistory: (key: string) => call("delete_history", { key }),
  tasks: () => call<Tasks>("tasks"),
  cancelActivity: (uuid: string) => call("cancel_activity", { uuid }),
  killTranscode: (key: string) => call("kill_transcode", { key }),

  // Server administration
  serverOverview: () => call<Overview>("server_overview"),
  checkForUpdates: () => call("check_for_updates"),
  applyUpdate: (tonight: boolean) => call("apply_update", { tonight }),
  sectionAction: (id: string, action: "scan" | "refresh" | "analyze" | "emptyTrash") =>
    call("section_action", { id, action }),
  serverAction: (action: "scanAll" | "cleanBundles" | "optimizeDatabase") => call("server_action", { action }),
  maintenance: () => call<Maintenance>("maintenance"),
  butlerTask: (name: string, start: boolean) => call("butler_task", { name, start }),
  prefs: () => call<PrefGroup[]>("prefs"),
  setPref: (id: string, value: string) => call("set_pref", { id, value }),
  usersDevices: () => call<UsersDevices>("users_devices"),
  libraryChoices: (kind: string) => call<LibraryChoices>("library_choices", { kind }),
  createLibrary: (form: LibraryForm) => call("create_library", { form }),
  editLibrary: (id: string, name: string, locations: string[]) => call("edit_library", { id, name, locations }),
  deleteLibrary: (id: string) => call("delete_library", { id }),
  cancelScan: (id: string) => call("cancel_scan", { id }),
  libraryPrefs: (id: string) => call<Pref[]>("library_prefs", { id }),
  setLibraryPref: (id: string, pref: string, value: string) => call("set_library_pref", { id, pref, value }),
  butlerAll: (start: boolean) => call("butler_all", { start }),
  /** timespan: 6 = hourly buckets, 4 = daily. `since` in unix seconds. */
  serverStats: (timespan: number, since: number) => call<ServerStats>("server_stats", { timespan, since }),
  /** Saves a zip to the Downloads folder and reveals it; resolves to the path. */
  saveDiagnostics: (what: "logs" | "database") => call<string>("save_diagnostics", { what }),
};
