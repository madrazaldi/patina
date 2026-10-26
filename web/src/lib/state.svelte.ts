// Global app state. Mirrors the Rust side (crates/app/src/state.rs) through
// its events, plus UI-only state such as menus and dialogs.

import { listen } from "./ipc";
import { untrack } from "svelte";
import { nav } from "./router.svelte";
import { api } from "./api";
import type {
  Directory,
  Download,
  Info,
  Metadata,
  NowPlaying,
  PlaybackFailure,
  PlaybackState,
  PlayerSettings,
  QueueView,
  SavedView,
  SavedQueueView,
  ServerActivity,
  SessionView,
  Settings,
  Stage,
  UiSettings,
} from "./types";

export interface Toast {
  id: number;
  text: string;
  error: boolean;
  /** Id of the undo entry that reverses what the toast reports (see `app.undo`). */
  undo?: number;
}

export interface Menu {
  item: Metadata;
  x: number;
  y: number;
}

export interface Confirm {
  title: string;
  text?: string;
  /** Label of the confirming button. */
  action: string;
  danger: boolean;
  resolve: (ok: boolean) => void;
}

/** What a revision covered, as reported by the `library-changed` event. */
export interface Changes {
  all: boolean;
  sections: string[];
  items: string[];
}

let toastId = 0;

/**
 * Capture the active account and server. `valid()` turns false once either changes,
 * so late responses and queued mutations can be dropped instead of landing on another server.
 */
export function captureScope() {
  const scope = untrack(() => app.scope);
  return { scope, valid: () => untrack(() => app.scope) === scope };
}

export const message = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : String(e));

class App {
  connected = $state(true);
  playlistItems = $state<Metadata[]>([]);
  get scope() { return `${this.session?.accountId}:${this.session?.serverId}`; }
  stage = $state<Stage>({ kind: "booting" });
  session = $state<SessionView | null>(null);
  sections = $state<Directory[]>([]);
  nowPlaying = $state<NowPlaying | null>(null);
  queue = $state<QueueView | null>(null);
  /** Volume, speed, repeat, sleep timer, chapters and diagnostics of the running player. */
  playback = $state<PlaybackState | null>(null);
  /** Why playback last failed, until it is retried or dismissed. */
  failure = $state<PlaybackFailure | null>(null);
  /** The queue that was playing when Patina last stopped, offered as a Resume (never auto-played). */
  savedQueue = $state<SavedQueueView | null>(null);
  /** Server background tasks, live. */
  activities = $state<ServerActivity[]>([]);
  downloads = $state<Download[]>([]);
  /** Whether the server has a DVR (Live TV page). */
  liveTv = $state(false);
  /** Bumped whenever watch state may have changed; views refetch. */
  revision = $state(0);
  /** Bumped when playback sessions on the server change (Activity page refetches). */
  sessionsTick = $state(0);
  settings = $state<Settings | null>(null);
  info = $state<Info | null>(null);
  toasts = $state<Toast[]>([]);

  menu = $state<Menu | null>(null);
  /** Item waiting for the "add to playlist" dialog. */
  playlistTarget = $state<Metadata | null>(null);
  /** Open command palette (Cmd/Ctrl+K). */
  palette = $state(false);
  /** A saved view the command palette asked a library page to apply once it is showing. */
  pendingView = $state<SavedView | null>(null);
  /** Show, season or episode waiting for the bulk audio/subtitle dialog. */
  tracksTarget = $state<Metadata | null>(null);
  /** Show, season or episode waiting for its automatic playback preferences. */
  prefsTarget = $state<Metadata | null>(null);
  /** Show, season or playlist waiting for the "keep next N downloaded" dialog. */
  keepTarget = $state<Metadata | null>(null);
  /** Item waiting for the "add to collection" dialog. */
  collectionTarget = $state<Metadata | null>(null);
  /** Item waiting for the metadata editor. */
  editTarget = $state<Metadata | null>(null);
  /** Item and kind waiting for the poster/background picker. */
  artworkTarget = $state<{ item: Metadata; kind: "poster" | "art" } | null>(null);
  /** Item waiting for the "fix match" dialog. */
  matchTarget = $state<Metadata | null>(null);
  /** Movie or episode waiting for the online subtitle search. */
  subtitleTarget = $state<Metadata | null>(null);
  /** Open confirmation dialog (see `confirm`). */
  confirming = $state<Confirm | null>(null);
  /** Queue drawer / lyrics / player panel above the now-playing bar. */
  panel = $state<"queue" | "lyrics" | "player" | null>(null);
  search = $state("");
  /** Search every server of the account instead of the connected one (the Search page's toggle). */
  searchAll = $state(false);

  get owned() {
    return this.session?.owned ?? false;
  }
  get hasTv() {
    return this.session?.hasTv ?? false;
  }
  get spoilers() {
    return this.settings?.ui.hide_spoilers ?? false;
  }

  async init() {
    await Promise.all([
      listen<boolean>("connection", (e) => (this.connected = e.payload)),
      listen("settings-changed", () => this.reloadSettings()),
      listen<Stage>("stage", (e) => (this.stage = e.payload)),
      listen<{ session: SessionView | null; sections: Directory[] }>("session", (e) => {
        const before = this.scope;
        this.session = e.payload.session;
        if (before !== this.scope) {
          nav.reset(); this.search = ""; this.searchAll = false;
          this.clearEditors();
          // Their entries belong to the previous account or server.
          this.toasts = this.toasts.filter((t) => t.undo === undefined);
          this.savedQueue = null;
          void this.loadSavedQueue();
        }
        this.sections = e.payload.sections;
      }),
      listen<NowPlaying | null>("now-playing", (e) => {
        this.nowPlaying = e.payload;
        // Once something plays the offer is moot (resuming, or the user chose otherwise).
        if (e.payload) this.savedQueue = null;
      }),
      listen<PlaybackState | null>("playback-state", (e) => (this.playback = e.payload)),
      listen<PlaybackFailure | null>("playback-failure", (e) => (this.failure = e.payload)),
      listen<QueueView | null>("queue", (e) => {
        this.queue = e.payload;
        // The queue only goes away when playback ends, unlike now-playing between tracks.
        if (!e.payload) this.panel = null;
      }),
      listen<ServerActivity[]>("activities", (e) => (this.activities = e.payload)),
      listen<Download[]>("downloads", (e) => (this.downloads = e.payload)),
      listen("sessions-changed", () => this.sessionsTick++),
      listen<Changes>("library-changed", (e) => this.#noteChanges(e.payload)),
      listen<number>("revision", () => this.#bump()),
      listen<{ text: string; error: boolean; undo?: number }>("toast", (e) => this.#push(e.payload.text, e.payload.error, e.payload.undo)),
    ]);
    const [snap, settings, info, downloads] = await Promise.all([
      api.snapshot(),
      api.settings(),
      api.info(),
      api.downloads(),
    ]);
    this.connected = snap.connected;
    this.stage = snap.stage;
    this.session = snap.session ?? null;
    this.sections = snap.sections;
    this.nowPlaying = snap.nowPlaying ?? null;
    this.queue = snap.queue ?? null;
    this.activities = snap.activities;
    void this.loadSavedQueue();
    api.playbackState().then((s) => (this.playback = s ?? this.playback)).catch(() => {});
    api.playbackFailure().then((f) => (this.failure = f ?? this.failure)).catch(() => {});
    this.downloads = downloads;
    this.settings = settings;
    this.info = info;
  }

  /** Look for a saved queue to offer, unless something is already playing. */
  async loadSavedQueue() {
    if (this.nowPlaying || !this.session) {
      this.savedQueue = null;
      return;
    }
    const scope = this.scope;
    try {
      const saved = await api.savedQueue();
      if (scope === this.scope && !this.nowPlaying) this.savedQueue = saved ?? null;
    } catch {
      // No offer is better than an error at start-up.
    }
  }

  /** Refetch everything that depends on watch state. */
  refresh() {
    this.#pending = null;
    this.#bump();
  }

  #pending: Changes | null = null;
  #log: ({ revision: number } & Changes)[] = [];
  #noteChanges(c: Changes) {
    const p = this.#pending ?? { all: false, sections: [], items: [] };
    this.#pending = {
      all: p.all || c.all,
      sections: [...new Set([...p.sections, ...c.sections])],
      items: [...new Set([...p.items, ...c.items])],
    };
  }
  /** A revision without a preceding `library-changed` could affect anything. */
  #bump() {
    const c = this.#pending ?? { all: true, sections: [], items: [] };
    this.#pending = null;
    this.revision++;
    this.#log.push({ revision: this.revision, ...c });
    if (this.#log.length > 64) this.#log.shift();
  }
  /** Whether anything in library section `section` may have changed since `revision`. */
  affects(section: string, since: number) {
    if (since >= this.revision) return false;
    const first = this.#log[0];
    if (!first || first.revision > since + 1) return true;
    return this.#log.some((c) => c.revision > since && (c.all || c.sections.includes(section)));
  }

  /** Close every menu, dialog and picker held in global state (page-local dialogs go with their page, which is keyed by scope). Called when the account or server changes, so none can act on the old one. */
  clearEditors() {
    this.menu = null;
    this.playlistTarget = null;
    this.playlistItems = [];
    this.tracksTarget = null;
    this.collectionTarget = null;
    this.editTarget = null;
    this.artworkTarget = null;
    this.matchTarget = null;
    this.subtitleTarget = null;
    this.prefsTarget = null;
    this.keepTarget = null;
    this.palette = false;
    this.pendingView = null;
    this.confirming?.resolve(false);
    this.confirming = null;
  }

  /** Ask before something destructive; resolves to whether it was confirmed. */
  confirm(title: string, opts: { text?: string; action?: string; danger?: boolean } = {}) {
    this.confirming?.resolve(false);
    return new Promise<boolean>((resolve) => {
      this.confirming = { title, text: opts.text, action: opts.action ?? "OK", danger: opts.danger ?? false, resolve };
    });
  }

  toast(text: string) {
    this.#push(text, false);
  }

  error(e: unknown) {
    this.#push(message(e), true);
  }

  /** Reverse a reversible action: the toast's Undo button passes its entry, Cmd/Ctrl+Z the latest. Core announces the result. */
  async undo(entry?: number) {
    if (entry !== undefined) this.toasts = this.toasts.filter((t) => t.undo !== entry);
    try {
      await api.undo(entry);
    } catch (e) {
      this.error(e);
    }
  }

  /** Cmd/Ctrl+Z. */
  async undoLatest() {
    const guard = captureScope();
    try {
      const last = await api.undoLast();
      if (!guard.valid()) return;
      if (last) await this.undo(last.id);
      else this.toast("Nothing to undo");
    } catch (e) {
      this.error(e);
    }
  }

  #push(text: string, error: boolean, undo?: number) {
    const id = ++toastId;
    this.toasts.push({ id, text, error, undo });
    setTimeout(() => (this.toasts = this.toasts.filter((t) => t.id !== id)), error ? 6000 : undo !== undefined ? 8000 : 3200);
  }

  #saveTimer: ReturnType<typeof setTimeout> | undefined;
  #playerPatch: Partial<PlayerSettings> = {};
  #uiPatch: Partial<UiSettings> = {};
  #saving = false;
  #settingsGeneration = 0;
  async reloadSettings() {
    const generation = this.#settingsGeneration;
    try {
      const next = await api.settings();
      if (!this.#saving && generation === this.#settingsGeneration && !Object.keys(this.#playerPatch).length && !Object.keys(this.#uiPatch).length) this.settings = next;
    } catch (e) { this.error(e); }
  }
  updatePlayer(patch: Partial<PlayerSettings>) {
    if (!this.settings) return;
    Object.assign(this.settings.player, patch);
    Object.assign(this.#playerPatch, patch);
    this.#save();
  }
  updateUi(patch: Partial<UiSettings>) {
    if (!this.settings) return;
    Object.assign(this.settings.ui, patch);
    Object.assign(this.#uiPatch, patch);
    this.#save();
  }
  #save() {
    this.#settingsGeneration++;
    clearTimeout(this.#saveTimer);
    this.#saveTimer = setTimeout(() => this.#flush(), 250);
  }
  async #flush() {
    if (this.#saving) return;
    this.#saving = true;
    const player = this.#playerPatch, ui = this.#uiPatch;
    this.#playerPatch = {}; this.#uiPatch = {};
    try {
      if (Object.keys(player).length) await api.setPlayerSettings(player);
      if (Object.keys(ui).length) await api.setUiSettings(ui);
    } catch (e) { this.error(`Could not save settings: ${message(e)}`); }
    finally {
      this.#saving = false;
      if (Object.keys(this.#playerPatch).length || Object.keys(this.#uiPatch).length) this.#save();
      else await this.reloadSettings();
    }
  }

}

export const app = new App();

/** Run an action, toasting `ok` on success (and refreshing views if asked) or the error. */
export async function act(promise: Promise<unknown>, ok?: string, refresh = false) {
  try {
    await promise;
    if (ok) app.toast(ok);
    if (refresh) app.refresh();
    return true;
  } catch (e) {
    app.error(e);
    return false;
  }
}
