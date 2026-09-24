// Typed wrappers around the Rust commands in crates/app/src/commands.rs.
// Errors reject with the message string.

import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  ButlerTask,
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
  PlayerSettings,
  PlaylistView,
  Pref,
  Settings,
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
}

export type Control =
  | { kind: "togglePause" }
  | { kind: "next" }
  | { kind: "previous" }
  | { kind: "stop" }
  | { kind: "seekRelative"; seconds: number };

export const api = {
  // App
  snapshot: () => call<Snapshot>("snapshot"),
  info: () => call<Info>("info"),
  settings: () => call<Settings>("settings"),
  setPlayerSettings: (player: PlayerSettings) => call("set_player_settings", { player }),
  setUiSettings: (ui: UiSettings) => call("set_ui_settings", { ui }),
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
  sectionItems: (
    id: string,
    q: { kind?: number; sort?: string; unwatched: boolean; genre?: string; start: number; size: number },
  ) => call<MediaContainer>("section_items", { id, ...q }),
  collections: (id: string) => call<Metadata[]>("collections", { id }),
  listing: (path: string, start: number, size: number) => call<MediaContainer>("listing", { path, start, size }),
  search: (query: string) => call<Hub[]>("search", { query }),
  metadata: (ratingKey: string) => call<Metadata | null>("metadata", { ratingKey }),
  children: (ratingKey: string, collection = false) => call<Metadata[]>("children", { ratingKey, collection }),
  findByGuid: (guid: string) => call<string | null>("find_by_guid", { guid }),

  // Item actions
  setWatched: (ratingKey: string, watched: boolean) => call("set_watched", { ratingKey, watched }),
  rate: (ratingKey: string, rating: number | null) => call("rate", { ratingKey, rating }),
  selectStreams: (partId: number, audio?: number, subtitle?: number) =>
    call("select_streams", { partId, audio, subtitle }),
  refreshMetadata: (ratingKey: string) => call("refresh_metadata", { ratingKey }),
  analyzeMetadata: (ratingKey: string) => call("analyze_metadata", { ratingKey }),

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

  // Playback
  play: (item: Metadata, opts: PlayOptions = {}) =>
    call("play", { request: { item, resume: opts.resume ?? true, shuffle: opts.shuffle ?? false, mediaIndex: opts.mediaIndex ?? 0 } }),
  control: (command: Control) => call("control", { command }),

  // Bulk audio & subtitles
  tracksLoad: (show: string) => call("tracks_load", { show }),
  tracksView: (query: { show: string; season: number | null; reference: string | null; audio: number | null; subtitle: string }) =>
    call<TrackView>("tracks_view", { query }),
  tracksApply: (changes: PartChange[], onProgress: (done: number, total: number) => void) => {
    const progress = new Channel<[number, number]>();
    progress.onmessage = ([done, total]) => onProgress(done, total);
    return call("tracks_apply", { changes, progress });
  },

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
  butlerTasks: () => call<ButlerTask[]>("butler_tasks"),
  butlerTask: (name: string, start: boolean) => call("butler_task", { name, start }),
  prefs: () => call<Pref[]>("prefs"),
  setPref: (id: string, value: string) => call("set_pref", { id, value }),
  usersDevices: () => call<UsersDevices>("users_devices"),
};
