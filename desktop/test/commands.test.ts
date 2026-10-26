// The webview's commands against tools/mock_pms.py: every command the UI's
// api.ts calls exists, and the read paths answer with the shapes the UI reads.
import { afterAll, beforeAll, expect, test } from "bun:test";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { commandNames, dispatch } from "../commands";
import type { Ctx } from "../commands";
import * as live from "../core/live";
import * as prefs from "../core/prefs";
import * as session from "../core/session";
import { AppState } from "../core/state";
import type { CoreEvent } from "../core/state";
import { hasPython, startMockPms } from "./mock-pms";
import type { MockPms } from "./mock-pms";

const saved = { config: process.env.XDG_CONFIG_HOME, data: process.env.PATINA_DATA_DIR };
let dir = "";
let mock: MockPms | undefined;
let app: AppState;
const events: CoreEvent[] = [];
const opened: string[] = [];
const ctx: Ctx = {
  host: {
    async openUrl(url) {
      opened.push(url);
    },
    async openPath(path) {
      opened.push(path);
    },
  },
  channel: <T>(ref: unknown) => ({
    send(message: T) {
      pushed.push({ ref, message });
    },
  }),
};
const pushed: { ref: unknown; message: unknown }[] = [];
const call = (cmd: string, args?: Record<string, unknown>) => dispatch(app, cmd, args, ctx);

beforeAll(async () => {
  if (!hasPython()) return;
  dir = mkdtempSync(join(tmpdir(), "patina-commands-"));
  process.env.XDG_CONFIG_HOME = join(dir, "config");
  process.env.PATINA_DATA_DIR = join(dir, "data");
  const media = join(dir, "media");
  mock = await startMockPms({ media, dir: join(dir, "mock") });
  writeFileSync(join(media, "movie.mkv"), new Uint8Array(1024).fill(42));
  app = new AppState({ emit: (e) => events.push(e) });
  await session.signInManual(app, mock.url, "x");
});

afterAll(async () => {
  if (app) {
    live.stop(app);
    app.dispose();
  }
  await mock?.stop();
  if (saved.config === undefined) delete process.env.XDG_CONFIG_HOME;
  else process.env.XDG_CONFIG_HOME = saved.config;
  if (saved.data === undefined) delete process.env.PATINA_DATA_DIR;
  else process.env.PATINA_DATA_DIR = saved.data;
  if (dir) rmSync(dir, { recursive: true, force: true });
});

const t = hasPython() ? test : test.skip;

test("every command the UI calls is handled", () => {
  const api = readFileSync(join(import.meta.dir, "../../web/src/lib/api.ts"), "utf8");
  const called = [...api.matchAll(/call(?:<[^(]*>)?\(\s*"([a-z_]+)"/g)].map((m) => m[1]!);
  expect(called.length).toBeGreaterThan(100);
  expect(called.filter((c) => !commandNames.includes(c))).toEqual([]);
  expect(commandNames.length).toBe(153);
});

test("unknown commands are rejected", async () => {
  const a = new AppState({ emit() {} });
  try {
    await expect(dispatch(a, "nope", {}, ctx)).rejects.toThrow("Unknown command nope.");
    // Inherited names are not commands (`constructor` would hand back the AppState).
    for (const cmd of ["constructor", "toString", "__proto__", "hasOwnProperty"]) {
      await expect(dispatch(a, cmd, {}, ctx)).rejects.toThrow(`Unknown command ${cmd}.`);
    }
  } finally {
    a.dispose();
  }
});

t("app commands", async () => {
  const snap = (await call("snapshot")) as { stage: { kind: string }; session: { baseUrl: string } | null };
  expect(snap.stage.kind).toBe("ready");
  expect(snap.session?.baseUrl.replace(/\/$/, "")).toBe(mock!.url);
  const info = (await call("info")) as { version: string; configPath: string };
  expect(info.configPath).toContain("config.toml");
  const settings = (await call("settings")) as { player: { quality: string }; ui: { accent: string } };
  expect(Object.keys(settings).sort()).toEqual(["player", "ui"]);
  await call("set_ui_settings", { ui: { poster_size: 200 } });
  await call("set_player_settings", { player: { quality: "original" } });
  expect(((await call("settings")) as { ui: { poster_size: number } }).ui.poster_size).toBe(200);
  await call("open_url", { url: "https://example.com" });
  expect(opened).toContain("https://example.com");
});

t("browsing", async () => {
  const snap = (await call("snapshot")) as { sections: { key: string; type: string }[] };
  const section = snap.sections.find((s) => s.type === "movie")!;
  const hubs = (await call("home_hubs")) as { title: string; Metadata: { ratingKey: string }[] }[];
  expect(hubs.length).toBeGreaterThan(0);
  expect(await call("section_hubs", { id: section.key })).toBeArray();
  expect(await call("section_sorts", { id: section.key })).toBeArray();
  expect(await call("section_filters", { id: section.key })).toBeArray();
  expect(await call("first_characters", { id: section.key })).toBeArray();
  expect(await call("collections", { id: section.key })).toBeArray();
  const items = (await call("section_items", {
    id: section.key,
    unwatched: false,
    filters: [],
    start: 0,
    size: 10,
    sort: "titleSort",
  })) as { totalSize?: number; Metadata: { ratingKey: string; title: string; Media: unknown[] }[] };
  expect(items.Metadata.length).toBeGreaterThan(0);
  const first = items.Metadata[0]!;
  expect(first.Media).toBeArray();
  const full = (await call("metadata", { ratingKey: first.ratingKey })) as { title: string } | null;
  expect(full?.title).toBe(first.title);
  expect(await call("item_hubs", { ratingKey: first.ratingKey, kind: "movie" })).toBeArray();
  const found = (await call("search", { query: first.title.slice(0, 3) })) as unknown[];
  expect(found).toBeArray();
  expect((await call("listing", { path: `/library/sections/${section.key}/all`, start: 0, size: 5 })) as object).toHaveProperty("Metadata");
  expect(await call("playlists")).toBeArray();
  expect(await call("sessions")).toBeArray();
  expect(await call("section_folders", { id: section.key })).toBeObject();
});

t("playback state and queues without a player", async () => {
  expect(await call("playback_state")).toBeNull();
  expect(await call("playback_failure")).toBeNull();
  expect(await call("saved_queue")).toBeNull();
  expect(await call("undo_last")).toBeNull();
  expect(await call("downloads")).toBeArray();
  expect(typeof (await call("livetv_available"))).toBe("boolean");
  await call("control", { command: { kind: "pause" } });
  await call("tune", { command: { kind: "cycleRepeat" } });
});

t("server administration reads", async () => {
  expect(await call("server_overview")).toBeObject();
  expect(await call("tasks")).toBeObject();
  expect(await call("maintenance")).toBeObject();
  expect(await call("prefs")).toBeArray();
  expect(await call("users_devices")).toBeObject();
  expect(await call("history", { page: 0 })).toBeObject();
  expect(await call("pick_options")).toBeObject();
  const picked = (await call("pick_something", { query: { kind: "movie" } })) as { items: unknown[] };
  expect(picked.items).toBeArray();
  expect(await call("smart_downloads")).toBeObject();
  expect(await call("playback_prefs", { show: null })).toBeObject();
  // The profile's entry is stored with `show: null` (as the TUI writes it); the UI's save replaces it.
  prefs.save(app, null, null, { audioLanguages: ["eng"] });
  await call("set_playback_prefs", { show: null, title: null, rules: { audioLanguages: ["jpn"] } });
  const profile = (await call("playback_prefs", { show: null })) as { profile: { audioLanguages: string[] | null } };
  expect(profile.profile.audioLanguages).toEqual(["jpn"]);
  await call("set_playback_prefs", { show: null, title: null, rules: {} });
  expect(((await call("playback_prefs", { show: null })) as typeof profile).profile.audioLanguages).toBeNull();
  expect(await call("playback_prefs_shows")).toBeArray();
});

t("search across servers streams progress to the channel", async () => {
  const result = (await call("search_all_servers", { query: "a", progress: { __channel: 7 } })) as { done: boolean };
  expect(result.done).toBe(true);
  expect(pushed.length).toBeGreaterThan(0);
  expect(pushed[0]!.ref).toEqual({ __channel: 7 });
});

t("marking watched announces an undoable toast", async () => {
  const snap = (await call("snapshot")) as { sections: { key: string }[] };
  const items = (await call("section_items", { id: snap.sections[0]!.key, unwatched: false, filters: [], start: 0, size: 1 })) as {
    Metadata: { ratingKey: string }[];
  };
  const key = items.Metadata[0]!.ratingKey;
  await call("set_watched", { ratingKey: key, watched: true });
  const last = (await call("undo_last")) as { id: number; label: string };
  expect(last.label).toBeString();
  expect(events.some((e) => e.kind === "toast")).toBe(true);
  await call("undo_action", { id: last.id });
  expect(await call("undo_last")).toBeNull();
});

t("errors reject with a message", async () => {
  await expect(call("section_action", { id: "1", action: "bogus" })).rejects.toThrow("Unknown library action bogus.");
});
