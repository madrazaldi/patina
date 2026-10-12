// Global app state. Mirrors the Rust side (crates/app/src/state.rs) through
// its events, plus UI-only state such as menus and dialogs.

import { listen } from "@tauri-apps/api/event";
import { nav } from "./router.svelte";
import { api } from "./api";
import type {
  Directory,
  Download,
  Info,
  Metadata,
  NowPlaying,
  PlayerSettings,
  QueueView,
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

let toastId = 0;

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
  /** Queue drawer / lyrics panel above the now-playing bar. */
  panel = $state<"queue" | "lyrics" | null>(null);
  search = $state("");

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
          nav.reset(); this.search = ""; this.menu = null; this.playlistTarget = null; this.playlistItems = [];
          this.confirming?.resolve(false); this.confirming = null;
        }
        this.sections = e.payload.sections;
      }),
      listen<NowPlaying | null>("now-playing", (e) => (this.nowPlaying = e.payload)),
      listen<QueueView | null>("queue", (e) => {
        this.queue = e.payload;
        // The queue only goes away when playback ends, unlike now-playing between tracks.
        if (!e.payload) this.panel = null;
      }),
      listen<ServerActivity[]>("activities", (e) => (this.activities = e.payload)),
      listen<Download[]>("downloads", (e) => (this.downloads = e.payload)),
      listen("sessions-changed", () => this.sessionsTick++),
      listen<number>("revision", () => this.revision++),
      listen<{ text: string; error: boolean }>("toast", (e) => this.#push(e.payload.text, e.payload.error)),
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
    this.downloads = downloads;
    this.settings = settings;
    this.info = info;
  }

  /** Refetch everything that depends on watch state. */
  refresh() {
    this.revision++;
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

  #push(text: string, error: boolean) {
    const id = ++toastId;
    this.toasts.push({ id, text, error });
    setTimeout(() => (this.toasts = this.toasts.filter((t) => t.id !== id)), error ? 6000 : 3200);
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
