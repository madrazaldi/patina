import { afterEach, beforeEach, expect, test } from "bun:test";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { parse, stringify } from "smol-toml";

import type { HomeLayout, PlayerSettings } from "./config";
import { VERSION } from "./config";
import {
  configDir,
  defaultHomeLayout,
  defaultPlayerSettings,
  loadSettings,
  mpvConfigDir,
  parseSettings,
  patch,
  reloadSettings,
  settingsToToml,
  updateSettings,
} from "./config";

let dir: string;
let saved: string | undefined;

beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "patina-config-"));
  saved = process.env.XDG_CONFIG_HOME;
  process.env.XDG_CONFIG_HOME = dir;
});

afterEach(() => {
  if (saved === undefined) delete process.env.XDG_CONFIG_HOME;
  else process.env.XDG_CONFIG_HOME = saved;
  rmSync(dir, { recursive: true });
});

test("old settings and saved views round trip", () => {
  const settings = parseSettings("[ui]\nposter_size = 180\n");
  expect(settings.ui.saved_views).toEqual([]);
  expect(settings.ui.poster_size).toBe(180);
  settings.ui.saved_views.push({
    id: "view-1",
    name: "Unwatched drama",
    scope: "alice:home",
    library: "1",
    mediaType: 0,
    sort: "titleSort",
    desc: false,
    unwatched: true,
    toggles: [],
    picked: [{ filter: "genre", title: "Genre", value: "10", label: "Drama" }],
  });
  settings.ui.home_layouts["alice:home"] = { ...defaultHomeLayout(), compact_continue: true };
  const text = settingsToToml(settings);
  expect(parseSettings(text)).toEqual(settings);
});

test("partial settings update preserves other fields", () => {
  const player = { ...defaultPlayerSettings(), quality: "4m-720" };
  patch(player, { continuous: false });
  expect(player.quality).toBe("4m-720");
  expect(player.continuous).toBe(false);
  expect(() => patch(player, { continuous: "wrong" })).toThrow();
  expect(player.continuous).toBe(false);
  expect(() => patch(player, { nope: 1 })).toThrow("Unknown setting: nope");
  expect(() => patch(player, 3)).toThrow("Settings patch must be an object");
});

test("the file layout matches what the Rust toml crate writes", () => {
  const settings = loadSettings();
  const text = readFileSync(join(dir, "patina", "config.toml"), "utf8");
  // Top-level scalar first, then the tables in declaration order; None fields are absent.
  const order = [...text.matchAll(/^\[([a-z.]+)\]$/gm)].map((m) => m[1]);
  expect(order).toEqual(["auth", "server", "player", "ui", "tui", "seerr"]);
  expect(text.startsWith(`client_id = "${settings.client_id}"`)).toBe(true);
  expect(text).not.toContain("account_id");
  expect(text).toContain("watched_threshold = 0.9");
  expect(text).toContain('skip_key = "TAB"');
});

test("seerr settings round trip and default to empty", () => {
  expect(parseSettings("").seerr).toEqual({ url: "", api_key: "" });
  const settings = parseSettings('[seerr]\nurl = "http://seerr.local:5055"\napi_key = "k"\n');
  expect(settings.seerr).toEqual({ url: "http://seerr.local:5055", api_key: "k" });
  expect(parseSettings(settingsToToml(settings)).seerr).toEqual(settings.seerr);
  expect(() => parseSettings("[seerr]\nurl = 3\n")).toThrow();
});

test("the client id is created once and kept", () => {
  const first = loadSettings();
  const second = loadSettings();
  expect(second.client_id).toBe(first.client_id);
  expect(configDir()).toBe(join(dir, "patina"));
});

test("unknown keys and sections survive a rewrite", () => {
  const path = join(dir, "patina", "config.toml");
  loadSettings();
  const doc = parse(readFileSync(path, "utf8")) as Record<string, Record<string, unknown>>;
  doc.player!.future_option = "x";
  doc.extras = { keep: [1, 2] };
  writeFileSync(path, stringify(doc));
  const s = updateSettings(loadSettings(), (s) => {
    s.player.quality = "2m-720";
  });
  expect(s.player.quality).toBe("2m-720");
  const after = parse(readFileSync(path, "utf8")) as Record<string, Record<string, unknown>>;
  expect(after.player!.future_option).toBe("x");
  expect(after.extras).toEqual({ keep: [1, 2] });
  expect(after.player!.quality).toBe("2m-720");
});

test("an unreadable file is never overwritten with defaults", () => {
  const path = join(dir, "patina", "config.toml");
  const first = loadSettings();
  writeFileSync(path, "this is = = not toml");
  expect(() => updateSettings(first, () => {})).toThrow("Settings could not be read; the existing file was preserved.");
  expect(readFileSync(path, "utf8")).toBe("this is = = not toml");
  // Reload falls back to the settings in memory.
  expect(reloadSettings(first)).toBe(first);
});

test("a wrongly typed field fails the whole file like serde", () => {
  expect(() => parseSettings('[player]\ncontinuous = "yes"\n')).toThrow();
  expect(() => parseSettings("[ui]\nposter_size = -3\n")).toThrow();
  expect(parseSettings("[player]\nwatched_threshold = 1\n").player.watched_threshold).toBe(1);
});

test("reload keeps the client id when the file has none", () => {
  const mine = loadSettings();
  writeFileSync(join(dir, "patina", "config.toml"), "[tui]\nhide_watched = true\n");
  const re = reloadSettings(mine);
  expect(re.client_id).toBe(mine.client_id);
  expect(re.tui.hide_watched).toBe(true);
});

test("directories follow XDG_CONFIG_HOME when absolute", () => {
  if (process.platform === "win32") return;
  expect(mpvConfigDir()).toBe(process.env.MPV_HOME ?? join(dir, "mpv"));
  process.env.XDG_CONFIG_HOME = "relative/path";
  expect(configDir().endsWith(join(".config", "patina"))).toBe(true);
});

test("exports for sub-settings stay usable", () => {
  const p: PlayerSettings = defaultPlayerSettings();
  const h: HomeLayout = defaultHomeLayout();
  expect(p.skip_key).toBe("TAB");
  expect(h.order).toEqual([]);
});

test("the_version_matches_the_workspace_version", () => {
  // tools/release.sh bumps both; the packaged app reads Cargo.toml.
  const cargo = readFileSync(new URL("../../Cargo.toml", import.meta.url), "utf8");
  expect<string | undefined>(VERSION).toBe(cargo.match(/^version = "([^"]+)"/m)?.[1]);
});
