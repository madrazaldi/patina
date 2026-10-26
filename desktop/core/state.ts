// Application state shared by the front end, the session flow and the player.
// Port of crates/core/src/state.rs.
//
// Changes are pushed to the front end as CoreEvents; it asks for a Snapshot once
// on startup and follows the events from then on. Property names of the view types
// are the serde (camelCase) names, because they are what the webview receives.

import { ClientInfo, Resource, User } from "../plex";
import type { Directory, Metadata, PlexServer, PlexTv } from "../plex";
import { PRODUCT, VERSION, loadSettings, reloadSettings, updateSettings as saveUpdatedSettings } from "./config";
import type { Settings } from "./config";
import type { PlaybackState } from "./controls";
import * as downloads from "./downloads";
import type { Download } from "./downloads";
import * as failure from "./failure";
import type { PlaybackFailure } from "./failure";
import { logger } from "./log";
import * as offline from "./offline";
import * as smart from "./smart";
import * as undo from "./undo";

const log = logger("state");

/** Where the app is in the sign-in / connect flow. */
export type Stage =
  | { kind: "booting" }
  | { kind: "login" }
  | { kind: "pickServer"; servers: ServerView[]; user: UserView }
  | { kind: "connecting"; name: string }
  | { kind: "ready" }
  | { kind: "failed"; message: string };

/** A connected server plus the account that reached it. */
export interface Session {
  /** plex.tv client; `null` for manually configured servers. */
  tv: PlexTv | null;
  server: PlexServer;
  serverName: string;
  serverId: string;
  user: User | null;
  /** Whether the signed-in user owns (administers) this server. */
  owned: boolean;
  servers: Resource[];
}

/** A plex.tv account as front ends see it (no tokens). */
export interface UserView {
  uuid: string | null;
  name: string;
  email: string | null;
  thumb: string | null;
  plexPass: boolean;
}

export function userView(u: User): UserView {
  return {
    uuid: u.uuid,
    name: User.displayName(u),
    email: u.email ?? null,
    thumb: u.thumb ?? null,
    plexPass: u.subscription?.active ?? false,
  };
}

/** A server resource as front ends see it (no tokens or connections). */
export interface ServerView {
  id: string;
  name: string;
  owned: boolean;
  sourceTitle: string | null;
  platform: string | null;
  version: string | null;
  presence: boolean;
}

export function serverView(r: Resource): ServerView {
  return {
    id: Resource.id(r),
    name: Resource.name(r),
    owned: r.owned,
    sourceTitle: r.sourceTitle ?? null,
    platform: r.platform ?? null,
    version: r.productVersion ?? null,
    presence: r.presence,
  };
}

export interface SessionView {
  accountId: string;
  serverName: string;
  serverId: string;
  machineIdentifier: string | null;
  baseUrl: string;
  owned: boolean;
  hasTv: boolean;
  user: UserView | null;
  servers: ServerView[];
}

export function sessionView(s: Session): SessionView {
  return {
    accountId: offline.account(s),
    serverName: s.serverName,
    serverId: s.serverId,
    machineIdentifier: s.server.machineIdentifier() ?? null,
    baseUrl: String(s.server.baseUrl()),
    owned: s.owned,
    hasTv: s.tv !== null,
    user: s.user ? userView(s.user) : null,
    servers: s.servers.map(serverView),
  };
}

export interface NowPlaying {
  item: Metadata;
  positionMs: number;
  durationMs: number;
  paused: boolean;
  transcoding: boolean;
  queueLen: number;
  queuePos: number;
  /** Queue entry id of the item (see QueueView). */
  uid: number;
  /** In the closing credits or last seconds with something queued after: show "Up next". */
  ending: boolean;
  /** Playback stops when this item ends. */
  stopAfter: boolean;
  live: boolean;
  /** A downloaded file played offline. */
  local: boolean;
}

export interface QueueItem {
  uid: number;
  item: Metadata;
}

/** The running play queue, in play order. */
export interface QueueView {
  items: QueueItem[];
  current: number | null;
  shuffled: boolean;
  stopAfter: boolean;
}

/** A background task on the server (scan, analysis, ...), from live notifications. */
export interface ServerActivity {
  uuid: string;
  kind: string;
  title: string;
  subtitle: string | null;
  progress: number;
  cancellable: boolean;
  sectionId: string | null;
}

export interface Toast {
  text: string;
  error: boolean;
  /**
   * Id of an undo entry (see undo.ts) that reverses what the toast reports.
   * Absent for ordinary toasts, so their payload is unchanged.
   */
  undo?: number;
}

/** Everything a front end needs to render its first frame. */
export interface Snapshot {
  connected: boolean;
  stage: Stage;
  session: SessionView | null;
  sections: Directory[];
  nowPlaying: NowPlaying | null;
  queue: QueueView | null;
  activities: ServerActivity[];
  revision: number;
}

/**
 * What the next "revision" event covers, so views can refetch only what changed.
 * `all` means the change could not be narrowed down (watch state, unknown sections).
 */
export class Changes {
  all: boolean;
  /** Library section ids that changed. */
  sections: string[];
  /** Rating keys that changed, when the server said which. */
  items: string[];

  constructor(init: Partial<Pick<Changes, "all" | "sections" | "items">> = {}) {
    this.all = init.all ?? false;
    this.sections = init.sections ?? [];
    this.items = init.items ?? [];
  }

  static everything(): Changes {
    return new Changes({ all: true });
  }

  /** A change in one library section, optionally to one item. */
  static section(section?: string | null, item?: string | null): Changes {
    const c = new Changes();
    c.add(section, item);
    return c;
  }

  /** Fold another change in. Without a section the change could be anywhere. */
  add(section?: string | null, item?: string | null): void {
    if (section != null) {
      if (!this.sections.includes(section)) this.sections.push(section);
    } else {
      this.all = true;
    }
    if (item != null && !this.items.includes(item)) this.items.push(item);
  }
}

/** A state change for the front end. */
export type CoreEvent =
  | { kind: "connection"; online: boolean }
  | { kind: "settingsChanged" }
  | { kind: "stage"; stage: Stage }
  | { kind: "session"; session: SessionView | null; sections: Directory[] }
  | { kind: "nowPlaying"; nowPlaying: NowPlaying | null }
  | { kind: "queue"; queue: QueueView | null }
  | { kind: "activities"; activities: ServerActivity[] }
  /** Watch state may have changed; views refetch. */
  | { kind: "revision"; revision: number }
  /** What the revision that follows covers (sent just before it). A revision with no preceding `changes` may affect anything. */
  | { kind: "changes"; changes: Changes }
  | { kind: "toast"; toast: Toast }
  | { kind: "downloads"; downloads: Download[] }
  /** Playback sessions on the server changed. */
  | { kind: "sessionsChanged" }
  /** Volume, speed, chapters, repeat, sleep timer and diagnostics of the playing item; `null` when nothing plays. */
  | { kind: "playbackState"; state: PlaybackState | null }
  /** Playback failed and can be retried, changed or dismissed; `null` clears it. */
  | { kind: "playbackFailure"; failure: PlaybackFailure | null };

/** The event name the GUI's webview listens for (`CoreEvent::name`). */
export function eventName(e: CoreEvent): string {
  switch (e.kind) {
    case "connection":
      return "connection";
    case "settingsChanged":
      return "settings-changed";
    case "stage":
      return "stage";
    case "session":
      return "session";
    case "nowPlaying":
      return "now-playing";
    case "queue":
      return "queue";
    case "activities":
      return "activities";
    case "revision":
      return "revision";
    case "changes":
      return "library-changed";
    case "toast":
      return "toast";
    case "downloads":
      return "downloads";
    case "sessionsChanged":
      return "sessions-changed";
    case "playbackState":
      return "playback-state";
    case "playbackFailure":
      return "playback-failure";
  }
}

/** The event payload as the webview receives it (`CoreEvent::payload`). */
export function eventPayload(e: CoreEvent): unknown {
  switch (e.kind) {
    case "connection":
      return e.online;
    case "settingsChanged":
    case "sessionsChanged":
      return null;
    case "stage":
      return e.stage;
    case "session":
      return { session: e.session, sections: e.sections };
    case "nowPlaying":
      return e.nowPlaying;
    case "queue":
      return e.queue;
    case "activities":
      return e.activities;
    case "revision":
      return e.revision;
    case "changes":
      return { all: e.changes.all, sections: e.changes.sections, items: e.changes.items };
    case "toast":
      return e.toast;
    case "downloads":
      return e.downloads;
    case "playbackState":
      return e.state;
    case "playbackFailure":
      return e.failure;
  }
}

/** Receives every CoreEvent. */
export interface EventSink {
  emit(event: CoreEvent): void;
}

/** A sink that drops every event (tests). */
export const NullSink: EventSink = { emit() {} };

/** Servers offered on the "choose a server" screen. */
export interface PendingServers {
  tv: PlexTv;
  user: User;
  servers: Resource[];
}

/** The running notification listener (see live.ts). */
export interface LiveTask {
  abort: AbortController;
  /** Set when the task ended on its own. */
  finished: boolean;
}

/** Whether two server clients point at the same base URL with the same token (`PlexServer: PartialEq`). */
export function sameServer(a: PlexServer, b: PlexServer): boolean {
  return a === b || (String(a.baseUrl()) === String(b.baseUrl()) && a.token() === b.token());
}

/** Shared application state; a single instance per process. */
export class AppState {
  private readonly sink: EventSink;
  private _settings: Settings;
  private _stage: Stage = { kind: "booting" };
  private _session: Session | null = null;
  private _sections: Directory[] = [];
  private _nowPlaying: NowPlaying | null = null;
  private _queue: QueueView | null = null;
  private _activities: ServerActivity[] = [];
  private _revision = 0;
  private _connected = true;
  private ticker: ReturnType<typeof setInterval> | null = null;
  private readonly tasks = new Set<Promise<unknown>>();

  /**
   * Bumped whenever the session changes, which stops the previous
   * server's notification listener.
   */
  liveGeneration = 0;
  /** The running notification listener, aborted when it is replaced or stopped. */
  liveTask: LiveTask | null = null;
  /** Bumped to abandon an in-flight PIN sign-in. */
  loginAttempt = 0;
  /** Servers offered on the "choose a server" screen. */
  pending: PendingServers | null = null;
  /** Episodes loaded by the bulk audio/subtitle dialog, keyed by show. */
  tracks: [string, Metadata[]] | null = null;
  /** Reversible actions of the current account and server (see undo.ts). */
  undo: undo.Stack = new undo.Stack();

  /** Load the settings and start the background poller (downloads, settings, offline sync). */
  constructor(sink: EventSink) {
    this.sink = sink;
    this._settings = loadSettings();
    this.startTicker();
  }

  private startTicker(): void {
    let downloadsSeen = "";
    let settingsSeen = "";
    let ticks = 0;
    const tick = () => {
      const list = downloads.list();
      const next = JSON.stringify(list) + smart.fingerprint();
      if (next !== downloadsSeen) {
        downloadsSeen = next;
        this.emit({ kind: "downloads", downloads: list });
      }
      if (list.some((d) => d.state === "queued" || d.state === "downloading")) downloads.resume(this);
      const s = this.settings();
      const nextSettings = JSON.stringify([s.player, s.ui, s.tui]);
      if (nextSettings !== settingsSeen) {
        settingsSeen = nextSettings;
        this.emit({ kind: "settingsChanged" });
      }
      ticks += 1;
      // Smart downloads follow watch state; check now and then even when nothing announces a change.
      if (ticks % 150 === 5) smart.reconcileSoon(this);
      if (ticks % 15 === 1) {
        this.spawn(
          offline.sync(this).catch((e) => {
            log.debug(`offline progress remains queued: ${errorText(e)}`);
          }),
        );
      }
    };
    this.ticker = setInterval(() => {
      try {
        tick();
      } catch (e) {
        log.warn(`background tick failed: ${errorText(e)}`);
      }
    }, 2000);
    // Never keep the process alive for the poller (the weak reference in Rust).
    (this.ticker as { unref?: () => void }).unref?.();
  }

  /** Stop the background poller (the Rust version stops when the state is dropped). */
  dispose(): void {
    if (this.ticker) clearInterval(this.ticker);
    this.ticker = null;
  }

  emit(event: CoreEvent): void {
    this.sink.emit(event);
  }

  /**
   * Run a detached task: the promise (or the async function's promise) is tracked,
   * and its failure is logged instead of becoming an unhandled rejection.
   */
  spawn<T>(task: Promise<T> | (() => Promise<T>)): Promise<T | undefined> {
    let p: Promise<T>;
    try {
      p = typeof task === "function" ? task() : task;
    } catch (e) {
      p = Promise.reject(e);
    }
    const tracked = p.catch((e): undefined => {
      log.warn(`background task failed: ${errorText(e)}`);
      return undefined;
    });
    this.tasks.add(tracked);
    void tracked.finally(() => this.tasks.delete(tracked));
    return tracked;
  }

  /** Resolves when every task spawned so far has finished (tests). */
  async settled(): Promise<void> {
    while (this.tasks.size > 0) await Promise.all([...this.tasks]);
  }

  snapshot(): Snapshot {
    return {
      connected: this._connected,
      stage: this._stage,
      session: this._session ? sessionView(this._session) : null,
      sections: [...this._sections],
      nowPlaying: this._nowPlaying,
      queue: this._queue,
      activities: [...this._activities],
      revision: this._revision,
    };
  }

  clientInfo(): ClientInfo {
    return new ClientInfo(this._settings.client_id, PRODUCT, VERSION);
  }

  /** The settings as saved now (another process may have changed the file). */
  settings(): Settings {
    return reloadSettings(this._settings);
  }

  /**
   * Change and save the settings. The file is re-read first so changes
   * made by another Patina process are kept.
   */
  updateSettings(f: (s: Settings) => void): void {
    try {
      this.tryUpdateSettings((s) => {
        f(s);
      });
    } catch (e) {
      this.error(`Could not save settings: ${errorText(e)}`);
    }
  }

  tryUpdateSettings(f: (s: Settings) => void): void {
    this._settings = saveUpdatedSettings(this._settings, f);
  }

  session(): Session | null {
    return this._session;
  }

  /** The connected server. */
  server(): PlexServer {
    if (!this._session) throw new Error("Not connected to a server.");
    return this._session.server;
  }

  tv(): PlexTv | null {
    return this._session?.tv ?? null;
  }

  /** The plex.tv client, or an error when not signed in to plex.tv. */
  plexTv(): PlexTv {
    const tv = this.tv();
    if (!tv) throw new Error("Not signed in to plex.tv.");
    return tv;
  }

  setConnected(online: boolean): void {
    const was = this._connected;
    this._connected = online;
    if (was !== online) {
      this.emit({ kind: "connection", online });
      if (online) {
        this.bump();
        failure.onReconnect(this);
      }
    }
  }

  stage(): Stage {
    return this._stage;
  }

  setStage(stage: Stage): void {
    this._stage = stage;
    this.emit({ kind: "stage", stage });
  }

  setSession(session: Session | null, sections: Directory[]): void {
    const view = session ? sessionView(session) : null;
    undo.sessionChanged(this, session);
    failure.sessionChanged(this, session);
    this._session = session;
    this._sections = sections;
    this.emit({ kind: "session", session: view, sections });
  }

  sections(): Directory[] {
    return [...this._sections];
  }

  setSections(sections: Directory[]): void {
    const session = this._session ? sessionView(this._session) : null;
    this._sections = sections;
    this.emit({ kind: "session", session, sections });
  }

  nowPlaying(): NowPlaying | null {
    return this._nowPlaying;
  }

  setNowPlaying(np: NowPlaying | null): void {
    this._nowPlaying = np;
    this.emit({ kind: "nowPlaying", nowPlaying: np });
  }

  queue(): QueueView | null {
    return this._queue;
  }

  setQueue(queue: QueueView | null): void {
    this._queue = queue;
    this.emit({ kind: "queue", queue });
  }

  activities(): ServerActivity[] {
    return [...this._activities];
  }

  setActivities(list: ServerActivity[]): void {
    this._activities = list;
    this.emit({ kind: "activities", activities: list });
  }

  updateActivities(f: (list: ServerActivity[]) => void): void {
    f(this._activities);
    this.emit({ kind: "activities", activities: [...this._activities] });
  }

  revision(): number {
    return this._revision;
  }

  /** Watch state may have changed; views refetch. */
  bump(): void {
    this._revision += 1;
    this.emit({ kind: "revision", revision: this._revision });
    // Watch state may have changed: smart downloads catch up.
    smart.reconcileSoon(this);
  }

  /** Like bump, telling front ends which sections and items changed. */
  bumpWith(changes: Changes): void {
    this.emit({ kind: "changes", changes });
    this.bump();
  }

  toast(text: string): void {
    this.emit({ kind: "toast", toast: { text, error: false } });
  }

  /** A toast that offers to undo what it reports. */
  toastUndo(text: string, undoId: number): void {
    this.emit({ kind: "toast", toast: { text, error: false, undo: undoId } });
  }

  error(text: unknown): void {
    this.emit({ kind: "toast", toast: { text: typeof text === "string" ? text : errorText(text), error: true } });
  }
}

/** The text of a caught value (`{e:#}`). */
export function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
