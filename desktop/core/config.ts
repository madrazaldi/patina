// Persistent settings, stored as TOML in `~/.config/patina/config.toml`
// (or the platform config dir on Windows). Port of crates/core/src/config.rs.
//
// The on-disk layout is the same as the Rust `toml` crate's, so the TUI and the
// desktop app read each other's file. Property names are the serde field names
// (snake_case), because they are what the file and the UI contain.

import { existsSync, readFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { isAbsolute, join } from "node:path";
import { parse, stringify } from "smol-toml";

import { logger } from "./log";
import * as storage from "./storage";
import { defaultProgram } from "../mpv";

const log = logger("config");

export const PRODUCT = "Patina";
/** Workspace version of the Rust crates (`[workspace.package] version`); tools/release.sh bumps both. */
export const VERSION = "0.2.0";

export interface AuthSettings {
  account_id: string | null;
  /** plex.tv account token (or the active Plex Home user's token). */
  token: string | null;
}

export interface ServerSettings {
  /** `clientIdentifier` of the preferred server. */
  id: string | null;
  /** Last connection URI that worked, tried first on startup. */
  last_uri: string | null;
  /** Manually configured server (no plex.tv account). */
  manual_url: string | null;
  manual_token: string | null;
}

export interface PlayerSettings {
  /** mpv executable; auto-detected when empty. */
  mpv_path: string;
  /** Extra mpv arguments, whitespace separated. Your mpv.conf still applies. */
  extra_args: string;
  /** Quality preset id (see QUALITIES). */
  quality: string;
  auto_skip_intro: boolean;
  auto_skip_credits: boolean;
  /** mpv key that skips the current intro/credits marker. */
  skip_key: string;
  /** Apply the audio/subtitle tracks selected in Plex, and save changes made in mpv back to Plex. */
  sync_streams: boolean;
  /** Fraction of the runtime after which an item counts as watched. */
  watched_threshold: number;
  /** Autoplay following episodes. */
  continuous: boolean;
  /** Show what's playing in the OS media controls (Now Playing, MPRIS, SMTC). */
  media_controls: boolean;
  /** Show what's playing as Discord Rich Presence. */
  discord: boolean;
  /** Discord application id; its name is what Discord shows ("Watching Patina"). */
  discord_app_id: string;
  /** Where downloads for offline playback go; a default folder when empty. */
  downloads_dir: string;
}

export interface HomeLayout {
  order: string[];
  hidden: string[];
  /** Hubs that are hidden by default (the legacy On Deck) but turned on. */
  shown: string[];
  compact_continue: boolean;
}

export interface UiSettings {
  /** Named library queries and home layouts, scoped by account and server in the GUI. */
  saved_views: unknown[];
  home_layouts: Record<string, HomeLayout>;
  accent: string;
  /** Poster width in px. */
  poster_size: number;
  /** Blur thumbnails and summaries of unwatched episodes. */
  hide_spoilers: boolean;
}

export interface TuiSettings {
  /** Show the inspector pane next to the columns. */
  show_inspector: boolean;
  show_watch_status: boolean;
  show_library_counts: boolean;
  hide_watched: boolean;
  /** Accent colour; empty means the theme's. */
  accent: string;
}

/** A Seerr (Overseerr/Jellyseerr) instance for requesting titles the server doesn't have. */
export interface SeerrSettings {
  /** Base URL such as `http://seerr.local:5055`; empty when not set up. */
  url: string;
  /** API key (Settings → General in Seerr); empty signs in with the Plex account instead. */
  api_key: string;
}

export interface Settings {
  /** Stable per-install identifier sent as `X-Plex-Client-Identifier`. */
  client_id: string;
  auth: AuthSettings;
  server: ServerSettings;
  player: PlayerSettings;
  /** The GUI's appearance. */
  ui: UiSettings;
  /** The terminal client's appearance and behaviour. */
  tui: TuiSettings;
  seerr: SeerrSettings;
}

export function defaultAuthSettings(): AuthSettings {
  return { account_id: null, token: null };
}
export function defaultServerSettings(): ServerSettings {
  return { id: null, last_uri: null, manual_url: null, manual_token: null };
}
export function defaultPlayerSettings(): PlayerSettings {
  return {
    mpv_path: "",
    extra_args: "",
    quality: "original",
    auto_skip_intro: false,
    auto_skip_credits: false,
    skip_key: "TAB",
    sync_streams: true,
    watched_threshold: 0.9,
    continuous: true,
    media_controls: true,
    discord: false,
    discord_app_id: "",
    downloads_dir: "",
  };
}
export function defaultHomeLayout(): HomeLayout {
  return { order: [], hidden: [], shown: [], compact_continue: false };
}
export function defaultUiSettings(): UiSettings {
  return { saved_views: [], home_layouts: {}, accent: "paper", poster_size: 160, hide_spoilers: false };
}
export function defaultTuiSettings(): TuiSettings {
  return { show_inspector: true, show_watch_status: true, show_library_counts: true, hide_watched: false, accent: "" };
}
export function defaultSeerrSettings(): SeerrSettings {
  return { url: "", api_key: "" };
}
export function defaultSettings(): Settings {
  return {
    client_id: crypto.randomUUID(),
    auth: defaultAuthSettings(),
    server: defaultServerSettings(),
    player: defaultPlayerSettings(),
    ui: defaultUiSettings(),
    tui: defaultTuiSettings(),
    seerr: defaultSeerrSettings(),
  };
}

/** `Settings::load()` / `Settings::default()` live here too, as in Rust. */
export const Settings = {
  load: (): Settings => loadSettings(),
  default: (): Settings => defaultSettings(),
};

// Methods of `PlayerSettings` in Rust. They are free functions here because the settings are plain data.

/** The mpv executable: the configured one, else the platform default. */
export function mpvProgram(p: PlayerSettings): string {
  return p.mpv_path.trim() === "" ? defaultProgram() : p.mpv_path.trim();
}

/** `PlayerSettings::quality()` (a field of the same name exists in TS): the selected preset, original when unknown. */
export function playerQuality(p: PlayerSettings): Quality {
  return quality(p.quality) ?? QUALITIES[0]!;
}

/** Where downloads go: the configured folder, else `~/Videos/Patina` (platform video folder), else the data dir. */
export function downloadsDir(p: PlayerSettings): string {
  const custom = p.downloads_dir.trim();
  if (custom !== "") return custom;
  const videos = videoDir();
  return videos ? join(videos, "Patina") : join(dataDir(), "downloads");
}

/** The platform video folder (`directories::UserDirs::video_dir`). */
function videoDir(): string | null {
  const home = homedir();
  switch (process.platform) {
    case "darwin":
      return join(home, "Movies");
    case "win32":
      return join(process.env.USERPROFILE || home, "Videos");
    default: {
      // xdg-user-dirs: XDG_VIDEOS_DIR in user-dirs.dirs; no fallback, like the directories crate.
      const base = envAbsolute("XDG_CONFIG_HOME") ?? join(home, ".config");
      try {
        const text = readFileSync(join(base, "user-dirs.dirs"), "utf8");
        const m = /^\s*XDG_VIDEOS_DIR\s*=\s*"?([^"\n]*)"?/m.exec(text);
        if (!m || !m[1]) return null;
        const dir = m[1].replace(/^\$HOME/, home);
        return isAbsolute(dir) ? dir : null;
      } catch {
        return null;
      }
    }
  }
}

export interface Accent {
  id: string;
  label: string;
  color: string;
}

/**
 * Accents are used sparingly (progress, watched marks, focus), so they are
 * kept muted. Ids are stable; they are stored in the config file.
 */
export const ACCENTS: readonly Accent[] = [
  { id: "paper", label: "Paper", color: "#e6e1d6" },
  { id: "amber", label: "Amber", color: "#d6a44c" },
  { id: "copper", label: "Copper", color: "#c98a5e" },
  { id: "verdigris", label: "Verdigris", color: "#7fb3a0" },
  { id: "sky", label: "Sky", color: "#7ea6cc" },
  { id: "iris", label: "Iris", color: "#9d98d4" },
  { id: "rose", label: "Rose", color: "#cf8a98" },
];

export interface Quality {
  id: string;
  label: string;
  /** `null` means direct play. */
  kbps: number | null;
  resolution: string;
}

export const QUALITIES: readonly Quality[] = [
  { id: "original", label: "Original (direct play)", kbps: null, resolution: "" },
  { id: "40m-4k", label: "4K · 40 Mbps", kbps: 40000, resolution: "3840x2160" },
  { id: "20m-1080", label: "1080p · 20 Mbps", kbps: 20000, resolution: "1920x1080" },
  { id: "12m-1080", label: "1080p · 12 Mbps", kbps: 12000, resolution: "1920x1080" },
  { id: "8m-1080", label: "1080p · 8 Mbps", kbps: 8000, resolution: "1920x1080" },
  { id: "4m-720", label: "720p · 4 Mbps", kbps: 4000, resolution: "1280x720" },
  { id: "2m-720", label: "720p · 2 Mbps", kbps: 2000, resolution: "1280x720" },
  { id: "1m-480", label: "480p · 1 Mbps", kbps: 1000, resolution: "854x480" },
];

/** A quality preset by id or label. */
export function quality(name: string): Quality | undefined {
  return QUALITIES.find((q) => q.id === name || q.label === name);
}

// Directories (read per call, never cached: tests and sandboxes change the environment).

/** An environment variable that holds an absolute path (like `PathBuf::is_absolute` filtering in Rust). */
function envAbsolute(name: string): string | null {
  const v = process.env[name];
  return v && isAbsolute(v) ? v : null;
}

/** `directories::ProjectDirs::from("", "", "patina")` base directories for this platform. */
function projectDirs(): { config: string; data: string; cache: string } | null {
  const home = homedir();
  if (!home) return null;
  switch (process.platform) {
    case "darwin":
      return {
        config: join(home, "Library", "Application Support", "patina"),
        data: join(home, "Library", "Application Support", "patina"),
        cache: join(home, "Library", "Caches", "patina"),
      };
    case "win32": {
      const roaming = process.env.APPDATA || join(home, "AppData", "Roaming");
      const local = process.env.LOCALAPPDATA || join(home, "AppData", "Local");
      return {
        config: join(roaming, "patina", "config"),
        data: join(roaming, "patina", "data"),
        cache: join(local, "patina", "cache"),
      };
    }
    default:
      return {
        config: join(envAbsolute("XDG_CONFIG_HOME") ?? join(home, ".config"), "patina"),
        data: join(envAbsolute("XDG_DATA_HOME") ?? join(home, ".local", "share"), "patina"),
        cache: join(envAbsolute("XDG_CACHE_HOME") ?? join(home, ".cache"), "patina"),
      };
  }
}

export function configDir(): string {
  if (process.platform === "win32") {
    const dirs = projectDirs();
    if (dirs) return dirs.config;
  }
  // Same convention as mpv on macOS/Linux: ~/.config (or $XDG_CONFIG_HOME).
  const home = homedir();
  const base = envAbsolute("XDG_CONFIG_HOME") ?? (home ? join(home, ".config") : ".");
  return join(base, "patina");
}

export function cacheDir(): string {
  return projectDirs()?.cache ?? join(tmpdir(), "patina");
}

export function dataDir(): string {
  // Lets tests and sandboxes keep checkpoints and saved queues out of the real data directory.
  const override = envAbsolute("PATINA_DATA_DIR");
  if (override) return override;
  return projectDirs()?.data ?? join(configDir(), "data");
}

/** The directory mpv reads its configuration from by default. */
export function mpvConfigDir(): string {
  if (process.platform === "win32" && process.env.APPDATA) return join(process.env.APPDATA, "mpv");
  if (process.env.MPV_HOME) return process.env.MPV_HOME;
  const home = homedir();
  const base = envAbsolute("XDG_CONFIG_HOME") ?? (home ? join(home, ".config") : "");
  return join(base, "mpv");
}

function configFile(): string {
  return join(configDir(), "config.toml");
}

// Reading and writing

type Table = Record<string, unknown>;

function isTable(v: unknown): v is Table {
  return typeof v === "object" && v !== null && !Array.isArray(v) && !(v instanceof Date);
}

/** A strict typed field, like serde: the wrong type fails the whole struct. */
class Invalid extends Error {}

const str = (v: unknown, name: string): string => {
  if (typeof v !== "string") throw new Invalid(`invalid type for \`${name}\`: expected a string`);
  return v;
};
const bool = (v: unknown, name: string): boolean => {
  if (typeof v !== "boolean") throw new Invalid(`invalid type for \`${name}\`: expected a boolean`);
  return v;
};
const num = (v: unknown, name: string): number => {
  if (typeof v === "bigint") return Number(v);
  if (typeof v !== "number") throw new Invalid(`invalid type for \`${name}\`: expected a number`);
  return v;
};
const uint = (v: unknown, name: string): number => {
  const n = num(v, name);
  if (!Number.isInteger(n) || n < 0 || n > 0xffffffff) throw new Invalid(`invalid value for \`${name}\`: expected a u32`);
  return n;
};
const strings = (v: unknown, name: string): string[] => {
  if (!Array.isArray(v)) throw new Invalid(`invalid type for \`${name}\`: expected a sequence`);
  return v.map((x) => str(x, name));
};
const table = (v: unknown, name: string): Table => {
  if (!isTable(v)) throw new Invalid(`invalid type for \`${name}\`: expected a table`);
  return v;
};

/** Each `get` below maps a missing key to the default (`#[serde(default)]`). */
function field<T>(t: Table, key: string, def: T, read: (v: unknown, name: string) => T): T {
  return key in t ? read(t[key], key) : def;
}
const optStr = (v: unknown, name: string): string | null => str(v, name);

function authFrom(t: Table): AuthSettings {
  const d = defaultAuthSettings();
  return { account_id: field(t, "account_id", d.account_id, optStr), token: field(t, "token", d.token, optStr) };
}
function serverFrom(t: Table): ServerSettings {
  const d = defaultServerSettings();
  return {
    id: field(t, "id", d.id, optStr),
    last_uri: field(t, "last_uri", d.last_uri, optStr),
    manual_url: field(t, "manual_url", d.manual_url, optStr),
    manual_token: field(t, "manual_token", d.manual_token, optStr),
  };
}
function playerFrom(t: Table): PlayerSettings {
  const d = defaultPlayerSettings();
  return {
    mpv_path: field(t, "mpv_path", d.mpv_path, str),
    extra_args: field(t, "extra_args", d.extra_args, str),
    quality: field(t, "quality", d.quality, str),
    auto_skip_intro: field(t, "auto_skip_intro", d.auto_skip_intro, bool),
    auto_skip_credits: field(t, "auto_skip_credits", d.auto_skip_credits, bool),
    skip_key: field(t, "skip_key", d.skip_key, str),
    sync_streams: field(t, "sync_streams", d.sync_streams, bool),
    watched_threshold: field(t, "watched_threshold", d.watched_threshold, num),
    continuous: field(t, "continuous", d.continuous, bool),
    media_controls: field(t, "media_controls", d.media_controls, bool),
    discord: field(t, "discord", d.discord, bool),
    discord_app_id: field(t, "discord_app_id", d.discord_app_id, str),
    downloads_dir: field(t, "downloads_dir", d.downloads_dir, str),
  };
}
function homeLayoutFrom(t: Table): HomeLayout {
  const d = defaultHomeLayout();
  return {
    order: field(t, "order", d.order, strings),
    hidden: field(t, "hidden", d.hidden, strings),
    shown: field(t, "shown", d.shown, strings),
    compact_continue: field(t, "compact_continue", d.compact_continue, bool),
  };
}
function uiFrom(t: Table): UiSettings {
  const d = defaultUiSettings();
  return {
    saved_views: field(t, "saved_views", d.saved_views, (v, name) => {
      if (!Array.isArray(v)) throw new Invalid(`invalid type for \`${name}\`: expected a sequence`);
      return v;
    }),
    home_layouts: field(t, "home_layouts", d.home_layouts, (v, name) => {
      const out: Record<string, HomeLayout> = {};
      for (const [k, layout] of Object.entries(table(v, name))) out[k] = homeLayoutFrom(table(layout, name));
      return out;
    }),
    accent: field(t, "accent", d.accent, str),
    poster_size: field(t, "poster_size", d.poster_size, uint),
    hide_spoilers: field(t, "hide_spoilers", d.hide_spoilers, bool),
  };
}
function tuiFrom(t: Table): TuiSettings {
  const d = defaultTuiSettings();
  return {
    show_inspector: field(t, "show_inspector", d.show_inspector, bool),
    show_watch_status: field(t, "show_watch_status", d.show_watch_status, bool),
    show_library_counts: field(t, "show_library_counts", d.show_library_counts, bool),
    hide_watched: field(t, "hide_watched", d.hide_watched, bool),
    accent: field(t, "accent", d.accent, str),
  };
}

function seerrFrom(t: Table): SeerrSettings {
  const d = defaultSeerrSettings();
  return { url: field(t, "url", d.url, str), api_key: field(t, "api_key", d.api_key, str) };
}

/** Settings from a parsed document; throws on a type mismatch (serde would fail the same way). */
function settingsFrom(t: Table): Settings {
  const d = defaultSettings();
  return {
    client_id: field(t, "client_id", d.client_id, str),
    auth: "auth" in t ? authFrom(table(t.auth, "auth")) : d.auth,
    server: "server" in t ? serverFrom(table(t.server, "server")) : d.server,
    player: "player" in t ? playerFrom(table(t.player, "player")) : d.player,
    ui: "ui" in t ? uiFrom(table(t.ui, "ui")) : d.ui,
    tui: "tui" in t ? tuiFrom(table(t.tui, "tui")) : d.tui,
    seerr: "seerr" in t ? seerrFrom(table(t.seerr, "seerr")) : d.seerr,
  };
}

/** Parse `config.toml` text into settings (the `toml::from_str::<Settings>` of Rust). Throws if unreadable. */
export function parseSettings(text: string): Settings {
  return settingsFrom(parse(text) as Table);
}

interface Loaded {
  settings: Settings;
  clientIdSaved: boolean;
}

function read(): Loaded | null {
  const path = configFile();
  let text: string;
  try {
    text = readFileSync(path, "utf8");
  } catch {
    return null;
  }
  let t: Table;
  try {
    t = parse(text) as Table;
  } catch (e) {
    log.warn(`ignoring unreadable ${path}: ${(e as Error).message}`);
    return null;
  }
  try {
    return { settings: settingsFrom(t), clientIdSaved: "client_id" in t };
  } catch (e) {
    log.warn(`ignoring unreadable ${path}: ${(e as Error).message}`);
    return null;
  }
}

export function loadSettings(): Settings {
  const lock = tryOr(() => storage.lock(configFile()));
  try {
    const loaded = read();
    const [settings, persist] = loaded ? [loaded.settings, !loaded.clientIdSaved] : [defaultSettings(), !existsSync(configFile())];
    // Persist so the generated client id stays stable.
    if (persist) {
      try {
        saveSettings(settings);
      } catch (e) {
        log.error(`could not save settings: ${(e as Error).message}`);
      }
    }
    return settings;
  } finally {
    lock?.release();
  }
}

function tryOr<T>(f: () => T): T | null {
  try {
    return f();
  } catch {
    return null;
  }
}

/**
 * The file as saved now (another process may have changed it), keeping
 * this client id if the file has none. `current` when it can't be read.
 */
export function reloadSettings(current: Settings): Settings {
  const loaded = read();
  if (!loaded) return current;
  if (!loaded.clientIdSaved) loaded.settings.client_id = current.client_id;
  return loaded.settings;
}

/** Fields in the order the Rust structs declare them (the order `toml` writes them). */
function authOut(s: AuthSettings): Table {
  return omitNull({ account_id: s.account_id, token: s.token });
}
function serverOut(s: ServerSettings): Table {
  return omitNull({ id: s.id, last_uri: s.last_uri, manual_url: s.manual_url, manual_token: s.manual_token });
}
function omitNull(t: Table): Table {
  return Object.fromEntries(Object.entries(t).filter(([, v]) => v !== null && v !== undefined));
}

/** The settings as a TOML document, merged over `existing` so unknown keys and sections survive. */
export function settingsToToml(s: Settings, existing: Table | null = null): string {
  const known: Record<string, Table> = {
    auth: authOut(s.auth),
    server: serverOut(s.server),
    player: { ...s.player },
    ui: { ...s.ui },
    tui: { ...s.tui },
    seerr: { ...s.seerr },
  };
  const doc: Table = { client_id: s.client_id };
  for (const [name, fields] of Object.entries(known)) {
    const section = { ...fields };
    // Keep keys this version does not know inside known sections.
    const old = existing?.[name];
    if (isTable(old)) {
      const mine = knownKeys[name] ?? [];
      for (const [k, v] of Object.entries(old)) if (!mine.includes(k) && !(k in section)) section[k] = v;
    }
    doc[name] = section;
  }
  if (existing) {
    for (const [k, v] of Object.entries(existing)) if (!(k in doc)) doc[k] = v;
  }
  // Rust writes an f64 with a fractional marker; smol-toml writes integral numbers as integers.
  // An empty table is followed by a blank line, as the toml crate writes it.
  return stringify(doc)
    .replace(/^(watched_threshold = -?\d+)$/m, "$1.0")
    .replace(/^(\[[^\]\n]+\])\n(?=\[)/gm, "$1\n\n");
}

const knownKeys: Record<string, string[]> = {
  auth: ["account_id", "token"],
  server: ["id", "last_uri", "manual_url", "manual_token"],
  player: Object.keys(defaultPlayerSettings()),
  ui: Object.keys(defaultUiSettings()),
  tui: Object.keys(defaultTuiSettings()),
  seerr: Object.keys(defaultSeerrSettings()),
};

/** Existing document of the config file, when it parses (to keep unknown keys). */
function existingDocument(): Table | null {
  try {
    return parse(readFileSync(configFile(), "utf8")) as Table;
  } catch {
    return null;
  }
}

export function saveSettings(s: Settings): void {
  storage.write(configFile(), new TextEncoder().encode(settingsToToml(s, existingDocument())));
}

/** Hold an OS lock across the entire read-modify-write transaction. */
export function updateSettings(current: Settings, f: (s: Settings) => void): Settings {
  const lock = storage.lock(configFile());
  try {
    // Never overwrite an existing unreadable file with defaults.
    let latest: Settings;
    if (existsSync(configFile())) {
      const loaded = read();
      if (!loaded) throw new Error("Settings could not be read; the existing file was preserved.");
      latest = loaded.settings;
    } else {
      latest = structuredClone(current);
    }
    f(latest);
    saveSettings(latest);
    return latest;
  } finally {
    lock.release();
  }
}

/** Which settings struct a value is, from its fields (patches have to be checked against its field types). */
function kindOf(value: object): "player" | "ui" | "tui" | "auth" | "server" | "home" | "settings" | null {
  if ("mpv_path" in value) return "player";
  if ("poster_size" in value) return "ui";
  if ("show_inspector" in value) return "tui";
  if ("manual_url" in value) return "server";
  if ("account_id" in value) return "auth";
  if ("compact_continue" in value) return "home";
  if ("client_id" in value) return "settings";
  return null;
}

/**
 * Apply only fields actually changed by the front end, preserving other writers' fields.
 * Mutates `value`; throws (leaving it unchanged) on an unknown key or a value of the wrong type.
 */
export function patch<T extends object>(value: T, patchObj: unknown): void {
  if (!isTable(patchObj)) throw new Error("Settings patch must be an object");
  const json = structuredClone(value) as Table;
  for (const [key, v] of Object.entries(patchObj)) {
    if (!(key in json)) throw new Error(`Unknown setting: ${key}`);
    json[key] = v;
  }
  // Validate the way serde's from_value would. Optional strings accept null.
  const kind = kindOf(value);
  const clean = (t: Table): Table => Object.fromEntries(Object.entries(t).filter(([, v]) => v !== null));
  let next: object;
  switch (kind) {
    case "player":
      next = playerFrom(json);
      break;
    case "ui":
      next = uiFrom(json);
      break;
    case "tui":
      next = tuiFrom(json);
      break;
    case "auth":
      next = authFrom(clean(json));
      break;
    case "server":
      next = serverFrom(clean(json));
      break;
    case "home":
      next = homeLayoutFrom(json);
      break;
    case "settings":
      next = settingsFrom(clean(json));
      break;
    default:
      next = json;
  }
  Object.assign(value, next);
}
