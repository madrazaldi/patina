// The Seerr commands against tools/mock_seerr.py: connecting with an API key or
// the Plex account, discovering and requesting titles, and managing requests.
import { afterAll, beforeAll, expect, test } from "bun:test";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { dispatch } from "../commands";
import type { Ctx } from "../commands";
import * as seerr from "../core/seerr";
import { AppState } from "../core/state";
import * as session from "../core/session";
import { hasPython, startMock, startMockPms } from "./mock-pms";
import type { MockPms } from "./mock-pms";

const SCRIPT = resolve(import.meta.dir, "../../tools/mock_seerr.py");
const saved = process.env.XDG_CONFIG_HOME;
let dir = "";
let mock: MockPms | undefined;
let pms: MockPms | undefined;
let app: AppState;
const opened: string[] = [];
const ctx: Ctx = {
  host: {
    async openUrl(url) {
      opened.push(url);
    },
    async openPath() {},
  },
  channel: () => ({ send() {} }),
};
const call = (cmd: string, args?: Record<string, unknown>) => dispatch(app, cmd, args, ctx) as Promise<any>;
const events = () =>
  mock!
    .events()
    .split("\n")
    .filter(Boolean)
    .map((l) => JSON.parse(l));

beforeAll(async () => {
  if (!hasPython()) return;
  dir = mkdtempSync(join(tmpdir(), "patina-seerr-"));
  process.env.XDG_CONFIG_HOME = join(dir, "config");
  mock = await startMock(SCRIPT, "mock Seerr on ", ["--pending"], join(dir, "mock"));
  app = new AppState({ emit() {} });
  pms = await startMockPms({ media: join(dir, "media"), dir: join(dir, "pms") });
  writeFileSync(join(dir, "media", "movie.mkv"), new Uint8Array(1024));
  await session.signInManual(app, pms.url, "x");
});

afterAll(async () => {
  seerr.reset();
  app?.dispose();
  await mock?.stop();
  await pms?.stop();
  if (saved === undefined) delete process.env.XDG_CONFIG_HOME;
  else process.env.XDG_CONFIG_HOME = saved;
  if (dir) rmSync(dir, { recursive: true, force: true });
});

const t = hasPython() ? test : test.skip;

t("nothing works until Seerr is set up, and a bad key is not saved", async () => {
  expect(await call("seerr_config")).toBeNull();
  await expect(call("seerr_search", { query: "line" })).rejects.toThrow("Seerr is not set up");
  await expect(call("seerr_connect", { url: mock!.url, apiKey: "wrong" })).rejects.toThrow("did not accept the sign-in");
  expect(await call("seerr_config")).toBeNull();
});

t("connecting with an API key saves the instance and reports an admin", async () => {
  const status = await call("seerr_connect", { url: `${mock!.url}/api/v1/`, apiKey: "mock-key" });
  expect(status).toMatchObject({ url: mock!.url, auth: "apiKey", version: "2.7.3", user: { name: "Mock Admin" } });
  expect(status.can).toEqual({ requestMovie: true, requestTv: true, request4kMovie: true, request4kTv: false, advanced: true, manage: true });
  expect(await call("seerr_config")).toEqual({ url: mock!.url, auth: "apiKey" });
  const file = readFileSync(join(dir, "config", "patina", "config.toml"), "utf8");
  expect(file).toContain(`[seerr]\nurl = "${mock!.url}"\napi_key = "mock-key"`);
  // Reconnecting without a key keeps the saved one.
  expect(await call("seerr_connect", { url: mock!.url, apiKey: null })).toMatchObject({ auth: "apiKey" });
  // The key never reaches the webview.
  expect(JSON.stringify(await call("settings"))).not.toContain("mock-key");
});

t("search and discover list movies and shows with their availability", async () => {
  const found = await call("seerr_search", { query: "Kid's Own (Director's Cut)!" });
  expect(found.results.map((r: seerr.SeerrTitle) => r.title)).toEqual(["Kid's Own (Director's Cut)!"]);
  const trending = await call("seerr_discover", { list: "trending", page: 1 });
  expect(trending.results.every((r: seerr.SeerrTitle) => r.mediaType === "movie" || r.mediaType === "tv")).toBe(true);
  const line = trending.results.find((r: seerr.SeerrTitle) => r.tmdbId === 9001);
  expect(line).toMatchObject({ title: "The Verdigris Line", year: 2014, status: "available", ratingKey: "100" });
  await expect(call("seerr_discover", { list: "nope" })).rejects.toThrow("Unknown list nope.");
});

t("requesting a movie and seasons of a show", async () => {
  expect(await call("seerr_request", { mediaType: "movie", tmdbId: 9002, is4k: false })).toBe("approved");
  const movie = await call("seerr_details", { mediaType: "movie", tmdbId: 9002 });
  expect(movie.status).toBe("processing");
  expect(movie.requests).toMatchObject([{ state: "approved", mine: true, title: "Lighthouse Keepers" }]);

  const show = await call("seerr_details", { mediaType: "tv", tmdbId: 8001 });
  expect(show.tvdbId).toBe(7001);
  // The Plex copy is offered once this server's item has the same TMDB id.
  expect(show.ratingKey).toBe("500");
  expect(show.seasons.map((s: seerr.SeerrSeason) => [s.number, s.status])).toEqual([[1, "available"], [2, "available"], [3, "unknown"]]);
  await expect(call("seerr_request", { mediaType: "tv", tmdbId: 8001, seasons: [] })).rejects.toThrow("Choose at least one season.");
  await call("seerr_request", { mediaType: "tv", tmdbId: 8001, tvdbId: 7001, seasons: [3], serverId: 0, profileId: 2, rootFolder: "/tv-archive" });
  const after = await call("seerr_details", { mediaType: "tv", tmdbId: 8001 });
  expect(after.seasons.find((s: seerr.SeerrSeason) => s.number === 3).requested).toBe("approved");
  expect(events().find((e) => e.what === "request" && e.mediaType === "tv")).toMatchObject({
    mediaId: 8001, seasons: [3], tvdbId: 7001, serverId: 0, profileId: 2, rootFolder: "/tv-archive",
  });
  // Nothing left to request is Seerr's 202, not a success.
  await expect(call("seerr_request", { mediaType: "tv", tmdbId: 8001, seasons: [3] })).rejects.toThrow("No seasons available to request");
  // Seerr's own message comes through.
  await expect(call("seerr_request", { mediaType: "movie", tmdbId: 9002 })).rejects.toThrow("Request for this media already exists.");
});

t("advanced options list the servers of the right kind with profiles and folders", async () => {
  const movies4k = await call("seerr_request_options", { mediaType: "movie", is4k: true });
  expect(movies4k).toEqual([
    {
      id: 1, name: "Radarr 4K", isDefault: true, activeProfileId: 2, activeDirectory: "/movies-4k",
      profiles: [{ id: 1, name: "HD-1080p" }, { id: 2, name: "Ultra-HD" }],
      rootFolders: [{ path: "/movies-4k", freeSpace: 4e12 }, { path: "/movies-4k-archive", freeSpace: 1e12 }],
    },
  ]);
  expect(await call("seerr_request_options", { mediaType: "tv", is4k: true })).toEqual([]);
});

t("requests list with titles, filter, and can be approved, declined and deleted", async () => {
  const all = await call("seerr_requests", { filter: "all", page: 1 });
  expect(all.results.map((r: seerr.SeerrRequest) => [r.title, r.state, r.requestedBy])).toEqual([
    ["Station Eleven Below", "approved", "Mock Admin"],
    ["Lighthouse Keepers", "approved", "Mock Admin"],
    ["Northern Static", "pending", "Alice"],
  ]);
  const pending = await call("seerr_requests", { filter: "pending", page: 1 });
  expect(pending.results).toHaveLength(1);
  const id = pending.results[0].id;
  await call("seerr_request_action", { id, action: "approve" });
  await expect(call("seerr_request_action", { id, action: "decline" })).rejects.toThrow("Only pending requests can be approved or declined.");
  await call("seerr_request_action", { id, action: "delete" });
  expect((await call("seerr_requests", { filter: "all", page: 1 })).results).toHaveLength(2);
  expect((await call("seerr_requests", { filter: "all", page: 1, mine: true })).results).toHaveLength(2);
  await expect(call("seerr_request_action", { id, action: "frob" })).rejects.toThrow("Unknown request action frob.");
  await expect(call("seerr_requests", { filter: "frob" })).rejects.toThrow("Unknown filter frob.");
});

t("open the Seerr page of a title", async () => {
  await call("seerr_open", { mediaType: "tv", tmdbId: 8001 });
  expect(opened.at(-1)).toBe(`${mock!.url}/tv/8001`);
});

t("without an API key Patina signs in with the Plex account", async () => {
  app.tryUpdateSettings((s) => (s.auth.token = "plex-token"));
  const status = await call("seerr_connect", { url: mock!.url, apiKey: "" });
  expect(status).toMatchObject({ auth: "plex", user: { name: "Alice" } });
  expect(status.can).toMatchObject({ requestMovie: true, request4kMovie: false, manage: false });
  // Not auto-approved, and only Alice's own requests are listed.
  expect(await call("seerr_request", { mediaType: "movie", tmdbId: 9004 })).toBe("pending");
  const mine = await call("seerr_requests", { filter: "all", page: 1 });
  expect(mine.results.map((r: seerr.SeerrRequest) => [r.title, r.mine])).toEqual([["Paper Satellites", true]]);
  expect(events().filter((e) => e.what === "plex-sign-in")).toHaveLength(1);
  await call("seerr_disconnect");
  expect(await call("seerr_config")).toBeNull();
});
