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
let session: SessionView = { accountId: "alice", serverId: "mock", serverName: "Mock Server", baseUrl: "http://127.0.0.1:32499", owned: true, hasTv: true, servers: [] };
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
// Commands whose responses are held back until released, to test late responses.
const held = new Map<string, (() => void)[]>();
// Watchlist: 100 titles, the even ones are on the server. `lookups` tracks find_by_guid concurrency.
const watchlist = Array.from({ length: 100 }, (_, n) => ({ ...item(1000 + n), guid: `plex://movie/${n}`, title: `Watchlist ${n}` }));
const lookups = { active: 0, max: 0, total: 0, delay: 15, fail: false, present: new Set<number>(watchlist.map((_, n) => n).filter((n) => n % 2 === 0)) };
const emit = (event: string, payload: unknown) => { const id = listeners.get(event); if (id !== undefined) callbacks.get(id)?.({ event, id, payload }); };
const downloads = [{ id: "mock:alice:1", account: "alice", serverId: "mock", serverName: "Mock Server", item: item(1), size: 10000, received: 10000, state: "done", addedAt: 1 }];
let profileRules: Record<string, unknown> = {};
const smart = { policies: [{ id: "p1", account: "alice", serverId: "mock", serverName: "Mock Server", ratingKey: "500", target: "show", title: "Station Eleven Below", keep: 3, deleteWatched: true, enabled: true, active: true, downloads: 2, bytes: 2000 }], budget: null as number | null, used: 10000 };
// Undo entries, newest last; `set_watched` records one and announces it with a toast like core does.
const undoStack: { id: number; label: string }[] = [];
let nextUndo = 7;
// Search across servers: the connected server answers first, "other" when released (or at once unless held).
const xsearch = { hold: false, release: () => {} };
const copy = (serverId: string, serverName: string, current: boolean, index: number, resolution: string) => ({
  serverId, serverName, current, library: "Movies",
  item: { ...item(index), ...(current ? {} : { patinaServer: serverId }), title: `Shared ${index}` },
  versions: [{ label: `${resolution} · HEVC · MKV · 12.4 Mbps · 2.1 GB`, resolution, videoCodec: "HEVC", container: "MKV", bitrate: 12400, size: 2100000000, hdr: null }],
});
const status = (id: string, name: string, current: boolean, state: string, count: number) => ({ id, name, current, state, count, error: null });
const allResults = (done: boolean) => ({
  query: "shared", done,
  servers: [status("mock", "Mock Server", true, "done", 2), status("other", "Other Server", false, done ? "done" : "pending", done ? 2 : 0)],
  groups: [
    { key: "plex://movie/1", title: "Shared 1", year: 2024, kind: "movie", subtitle: null, copies: done ? [copy("mock", "Mock Server", true, 31, "1080p"), copy("other", "Other Server", false, 41, "4K")] : [copy("mock", "Mock Server", true, 31, "1080p")] },
    ...(done ? [{ key: "plex://movie/2", title: "Only Elsewhere", year: 2020, kind: "movie", subtitle: null, copies: [copy("other", "Other Server", false, 42, "720p")] }] : []),
  ],
});
const test = {
  calls, settings, xsearch,
  fail(value: boolean) { fail = value; },
  setSavedQueue(value: unknown) { savedQueue = value; },
  delay(value: number) { delay = value; },
  hold(cmd: string) { held.set(cmd, []); },
  release(cmd: string) { for (const go of held.get(cmd) ?? []) go(); held.delete(cmd); },
  lookups,
  emit,
  setServers() {
    const server = (id: string, name: string) => ({ id, name, owned: true, presence: true });
    session = { ...session, servers: [server("mock", "Mock Server"), server("other", "Other Server")] };
    emit("session", { session, sections });
  },
  switchServer() { session = { ...session, serverId: "other", serverName: "Other Server" }; emit("session", { session, sections }); },
};
Object.assign(window, { __PATINA_TEST__: test, __TAURI_INTERNALS__: {
  transformCallback(fn: (event: unknown) => void) { const id = ++nextId; callbacks.set(id, fn); return id; },
  convertFileSrc(path: string) { return path; },
  async invoke(cmd: string, args: Record<string, any> = {}) {
    calls.push({ cmd, args });
    if (held.has(cmd)) await new Promise<void>((go) => held.get(cmd)!.push(go));
    if (cmd === "plugin:event|listen") { listeners.set(args.event, args.handler); return args.handler; }
    if (cmd === "snapshot") return { stage: { kind: "ready" }, session, sections, activities: [], revision: 0, connected: true };
    if (cmd === "settings") return structuredClone(settings);
    if (cmd === "set_ui_settings") { Object.assign(settings.ui, args.ui); return; }
    if (cmd === "set_player_settings") { Object.assign(settings.player, args.player); return; }
    if (cmd === "info") return { version: "test", configPath: "isolated fixture", accents: [], qualities: [] };
    if (cmd === "downloads") return downloads;
    if (cmd === "saved_queue") return savedQueue;
    if (cmd === "livetv_available") return false;
    if (cmd === "playback_prefs") return { profile: profileRules, show: args.show ? {} : null, effective: profileRules };
    if (cmd === "playback_prefs_shows") return [{ ratingKey: "500", title: "Station Eleven Below", rules: { audioLanguages: ["ja"], subtitles: "always" } }];
    if (cmd === "set_playback_prefs") { if (args.show == null) profileRules = args.rules; return; }
    if (cmd === "smart_downloads") return structuredClone(smart);
    if (cmd === "set_download_budget") { smart.budget = args.bytes; emit("downloads", downloads); return; }
    if (cmd === "update_smart_download") { Object.assign(smart.policies[0], args.patch); emit("downloads", downloads); return; }
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
    if (cmd === "watchlist") return watchlist;
    if (cmd === "find_by_guid") {
      lookups.active++; lookups.total++; lookups.max = Math.max(lookups.max, lookups.active);
      try {
        await new Promise((r) => setTimeout(r, lookups.delay));
        if (lookups.fail) throw new Error("Lookup failed");
        const n = Number(String(args.guid).split("/").pop());
        return lookups.present.has(n) ? String(1000 + n) : null;
      } finally { lookups.active--; }
    }
    if (cmd === "set_watched" || cmd === "set_watched_many") {
      const entry = { id: nextUndo++, label: `Marked ${args.ratingKey ?? "items"} watched` };
      undoStack.push(entry);
      emit("toast", { text: entry.label, error: false, undo: entry.id });
      return;
    }
    if (cmd === "undo_last") return undoStack.at(-1) ?? null;
    if (cmd === "undo_action") {
      const at = args.id == null ? undoStack.length - 1 : undoStack.findIndex((e) => e.id === args.id);
      if (at < 0) throw new Error("Nothing to undo.");
      const [entry] = undoStack.splice(at, 1);
      emit("toast", { text: `Undid: ${entry.label}`, error: false });
      return entry.label;
    }
    if (cmd === "pick_options") return { sections: [{ id: "1", title: "Movies", kind: "movie" }], genres: ["Comedy", "Drama"], contentRatings: ["PG-13", "R"] };
    if (cmd === "pick_something") {
      const offset = 10 + (args.query.exclude?.length ?? 0);
      return { items: [0, 1, 2].map((n) => item(offset + n)), matches: 40, remaining: 30, wrapped: false, libraries: ["Movies"] };
    }
    if (cmd === "search_all_servers") {
      const send = (message: unknown, index: number) => callbacks.get(args.progress.id)?.({ message, index });
      send(allResults(false), 0);
      if (xsearch.hold) await new Promise<void>((go) => (xsearch.release = go));
      send(allResults(true), 1);
      return allResults(true);
    }
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
