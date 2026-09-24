// Global app state. Mirrors the Rust side (crates/app/src/state.rs) through
// its events, plus UI-only state such as menus and dialogs.

import { listen } from "@tauri-apps/api/event";
import { api } from "./api";
import type {
  Directory,
  Info,
  Metadata,
  NowPlaying,
  PlayerSettings,
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

let toastId = 0;

export const message = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : String(e));

class App {
  stage = $state<Stage>({ kind: "booting" });
  session = $state<SessionView | null>(null);
  sections = $state<Directory[]>([]);
  nowPlaying = $state<NowPlaying | null>(null);
  /** Bumped whenever watch state may have changed; views refetch. */
  revision = $state(0);
  settings = $state<Settings | null>(null);
  info = $state<Info | null>(null);
  toasts = $state<Toast[]>([]);

  menu = $state<Menu | null>(null);
  /** Item waiting for the "add to playlist" dialog. */
  playlistTarget = $state<Metadata | null>(null);
  /** Show, season or episode waiting for the bulk audio/subtitle dialog. */
  tracksTarget = $state<Metadata | null>(null);
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
      listen<Stage>("stage", (e) => (this.stage = e.payload)),
      listen<{ session: SessionView | null; sections: Directory[] }>("session", (e) => {
        this.session = e.payload.session;
        this.sections = e.payload.sections;
      }),
      listen<NowPlaying | null>("now-playing", (e) => (this.nowPlaying = e.payload)),
      listen<number>("revision", () => this.revision++),
      listen<{ text: string; error: boolean }>("toast", (e) => this.#push(e.payload.text, e.payload.error)),
    ]);
    const [snap, settings, info] = await Promise.all([api.snapshot(), api.settings(), api.info()]);
    this.stage = snap.stage;
    this.session = snap.session ?? null;
    this.sections = snap.sections;
    this.nowPlaying = snap.nowPlaying ?? null;
    this.settings = settings;
    this.info = info;
  }

  /** Refetch everything that depends on watch state. */
  refresh() {
    this.revision++;
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

  /** Change settings locally at once; write them to disk shortly after. */
  updatePlayer(patch: Partial<PlayerSettings>) {
    if (!this.settings) return;
    Object.assign(this.settings.player, patch);
    this.#save();
  }

  updateUi(patch: Partial<UiSettings>) {
    if (!this.settings) return;
    Object.assign(this.settings.ui, patch);
    this.#save();
  }

  #save() {
    clearTimeout(this.#saveTimer);
    this.#saveTimer = setTimeout(() => {
      const s = $state.snapshot(this.settings);
      if (!s) return;
      Promise.all([api.setPlayerSettings(s.player), api.setUiSettings(s.ui)]).catch((e) => this.error(e));
    }, 250);
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
