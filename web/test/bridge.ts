// Development-only IPC fixture. This entry is not part of the production build.
import type { Metadata, Settings, SessionView } from "../src/lib/types";
const callbacks = new Map<number, (event: unknown) => void>();
const listeners = new Map<string, number>();
let nextId = 0;
const item = (index: number): Metadata => ({
  ratingKey: String(index), key: `/library/metadata/${index}`, type: "movie", title: `Title ${String(index).padStart(5, "0")}`,
  year: 2024, duration: 600000, viewOffset: index === 1 ? 100000 : 0,
  Media: [], Genre: [], Role: [], Director: [], Writer: [], Country: [], Collection: [], Label: [], Mood: [], Style: [], Image: [], smart: false,
});
let session: SessionView = { accountId: "alice", serverId: "mock", serverName: "Mock Server", baseUrl: "http://127.0.0.1:32499", owned: true, hasTv: false, servers: [] };
const settings: Settings = {
  player: { mpv_path: "", extra_args: "", quality: "original", auto_skip_intro: false, auto_skip_credits: false, skip_key: "TAB", sync_streams: true, watched_threshold: .9, continuous: true, media_controls: true, discord: false, discord_app_id: "", downloads_dir: "" },
  ui: { accent: "paper", poster_size: 160, hide_spoilers: false, saved_views: [], home_layouts: {} },
};
const sections = [{ key: "1", title: "Movies", type: "movie" }];
const pref = (id: string, label: string, group: string, kind = "bool", value = "true", extra = {}) => ({ id, label, group, kind, value, options: [], advanced: false, ...extra });
const prefs = [
  { id: "general", label: "General", prefs: [pref("FriendlyName", "Friendly name", "general", "text", "Mock Server"), pref("logDebug", "Debug logging", "general", "bool", "false", { advanced: true })] },
  { id: "network", label: "Network", prefs: [pref("secureConnections", "Secure connections", "network", "int", "1", { options: [["0", "Required"], ["1", "Preferred"]] })] },
  { id: "transcoder", label: "Transcoder", prefs: [pref("TranscoderQuality", "Transcoder quality", "transcoder", "int", "0", { options: [["0", "Automatic"], ["2", "Prefer higher quality encoding"]] })] },
];
const maintenance = {
  schedule: [pref("ButlerStartHour", "Time at which tasks start to run", "butler", "int", "2", { options: [["2", "2 am"], ["3", "3 am"]] })],
  tasks: [
    { name: "BackupDatabase", title: "Backup Database", description: "Create a backup copy of the server's database.", interval: 3, enabled: true, pref: pref("ButlerTaskBackupDatabase", "Backup database every three days", "butler") },
    { name: "CleanOldBundles", title: "Clean Old Bundles", description: "Remove old metadata bundles.", interval: 7, enabled: false, pref: pref("ButlerTaskCleanOldBundles", "Remove old bundles every week", "butler", "bool", "false") },
    { name: "CheckForUpdates", title: "Check For Updates", interval: 1, enabled: true },
  ],
};
const calls: { cmd: string; args: Record<string, any> }[] = [];
let fail = false, delay = 0, savedQueue: unknown = null;
const emit = (event: string, payload: unknown) => { const id = listeners.get(event); if (id !== undefined) callbacks.get(id)?.({ event, id, payload }); };
const downloads = [{ id: "mock:alice:1", account: "alice", serverId: "mock", serverName: "Mock Server", item: item(1), size: 10000, received: 10000, state: "done", addedAt: 1 }];
const test = {
  calls, settings,
  fail(value: boolean) { fail = value; },
  setSavedQueue(value: unknown) { savedQueue = value; },
  delay(value: number) { delay = value; },
  emit,
  switchServer() { session = { ...session, serverId: "other", serverName: "Other Server" }; emit("session", { session, sections }); },
};
Object.assign(window, { __PATINA_TEST__: test, __TAURI_INTERNALS__: {
  transformCallback(fn: (event: unknown) => void) { const id = ++nextId; callbacks.set(id, fn); return id; },
  convertFileSrc(path: string) { return path; },
  async invoke(cmd: string, args: Record<string, any> = {}) {
    calls.push({ cmd, args });
    if (cmd === "plugin:event|listen") { listeners.set(args.event, args.handler); return args.handler; }
    if (cmd === "snapshot") return { stage: { kind: "ready" }, session, sections, activities: [], revision: 0, connected: true };
    if (cmd === "settings") return structuredClone(settings);
    if (cmd === "set_ui_settings") { Object.assign(settings.ui, args.ui); return; }
    if (cmd === "set_player_settings") { Object.assign(settings.player, args.player); return; }
    if (cmd === "info") return { version: "test", configPath: "isolated fixture", accents: [], qualities: [] };
    if (cmd === "downloads") return downloads;
    if (cmd === "saved_queue") return savedQueue;
    if (cmd === "livetv_available") return false;
    if (cmd === "section_sorts") return [{ key: "titleSort", title: "Title" }, { key: "addedAt", title: "Date added" }];
    if (cmd === "section_filters") return [{ filter: "genre", title: "Genre", kind: "string" }];
    if (cmd === "section_filter_values") return [{ key: "10", title: "Drama" }];
    if (cmd === "first_characters") return [{ title: "A", size: 9600 }, { title: "Z", size: 400 }];
    if (cmd === "section_items") {
      const current = session.serverId;
      if (delay) await new Promise((r) => setTimeout(r, delay));
      if (fail) throw new Error("Mock server unavailable");
      return { totalSize: 10000, size: args.size, Metadata: Array.from({ length: Math.min(args.size, 10000 - args.start) }, (_, n) => ({ ...item(args.start + n), summary: current })) };
    }
    if (cmd === "metadata") return item(Number(args.ratingKey));
    if (cmd === "item_hubs") return [];
    if (cmd === "home_hubs") return [
      { hubIdentifier: "continue", title: "Continue Watching", Metadata: [item(1)], Directory: [] },
      { hubIdentifier: "recent", title: "Recently Added", Metadata: [item(2), item(3)], Directory: [] },
    ];
    if (cmd === "prefs") return structuredClone(prefs);
    if (cmd === "maintenance") return structuredClone(maintenance);
    if (cmd === "playlists") return [{ ...item(50), type: "playlist", title: "Favorites" }];
    if (cmd === "queue") emit("queue", { items: [{ uid: 1, item: args.command.item }], shuffled: false, stopAfter: false });
    if (cmd === "remove_download") { downloads.splice(0); emit("downloads", downloads); }
    return null;
  },
} });
await import("../src/main");
