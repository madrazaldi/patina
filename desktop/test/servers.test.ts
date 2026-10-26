// Real HTTP against several instances of tools/mock_pms.py: searching every server of the
// account, keeping each item with its own server, and undo.
// Port of crates/core/tests/servers.rs.
//
// Each test uses its own machine ids, because connections to other servers are cached
// per account and machine id for the whole process.
import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { ClientInfo, Metadata, PlexServer, PlexTv, normalizeResource } from "../plex";
import type { Resource } from "../plex";
import * as failure from "../core/failure";
import type { Report } from "../core/failure";
import type { PlayRequest } from "../core/player";
import { Scope } from "../core/recovery";
import * as servers from "../core/servers";
import { AppState, NullSink } from "../core/state";
import type { Session } from "../core/state";
import * as undo from "../core/undo";
import * as xsearch from "../core/ops/xsearch";

const python = Bun.which("python3");
const script = join(import.meta.dir, "..", "..", "tools", "mock_pms.py");
const mocks: Mock[] = [];
const dirs: string[] = [];

interface Mock {
  proc: ReturnType<typeof Bun.spawn>;
  url: string;
  port: number;
  machine: string;
  name: string;
}

async function startMock(name: string, machine: string, seed: number, extra: string[] = []): Promise<Mock> {
  const media = mkdtempSync(join(tmpdir(), "patina-mock-media-"));
  dirs.push(media);
  writeFileSync(join(media, "movie.mkv"), new Uint8Array(1024).fill(42));
  const proc = Bun.spawn(
    [python!, script, "--port", "0", "--name", name, "--machine", machine, "--seed", String(seed), ...extra, "--media", media],
    { stdout: "ignore", stderr: "pipe" },
  );
  const reader = (proc.stderr as ReadableStream<Uint8Array>).getReader();
  const decoder = new TextDecoder();
  let log = "";
  while (!log.includes("\n")) {
    const { value, done } = await reader.read();
    if (done) break;
    log += decoder.decode(value, { stream: true });
  }
  const first = log.split("\n")[0]!.trim();
  const prefix = "mock PMS on ";
  if (!first.startsWith(prefix)) throw new Error(`mock server startup: ${first}`);
  const url = first.slice(prefix.length);
  // Keep reading so the mock never blocks on a full pipe.
  void (async () => {
    for (;;) {
      const { done } = await reader.read().catch(() => ({ done: true }));
      if (done) return;
    }
  })();
  const port = Number(url.slice(url.lastIndexOf(":") + 1));
  const mock = { proc, url, port, machine, name };
  mocks.push(mock);
  return mock;
}

function resource(m: Mock): Resource {
  return normalizeResource({
    name: m.name,
    clientIdentifier: m.machine,
    provides: "server",
    owned: true,
    presence: true,
    accessToken: "t",
    connections: [{ protocol: "http", address: "127.0.0.1", port: m.port, uri: m.url, local: true, relay: false }],
  });
}

const apps: AppState[] = [];

/** An app connected to `current`, on an account that also has the `others`. */
function appFor(current: Mock, others: Mock[]): AppState {
  const app = new AppState(NullSink);
  apps.push(app);
  app.setSession(sessionFor(current, others), []);
  return app;
}

function sessionFor(current: Mock, others: Mock[]): Session {
  const info = new ClientInfo("test", "Patina", "test");
  return {
    tv: new PlexTv(info, "x"),
    server: new PlexServer(info, current.url, "alice"),
    serverId: current.machine,
    serverName: current.name,
    user: null,
    owned: true,
    servers: [current, ...others].map(resource),
  };
}

let config: string;
const saved = { config: process.env.XDG_CONFIG_HOME, data: process.env.PATINA_DATA_DIR };

beforeAll(() => {
  config = mkdtempSync(join(tmpdir(), "patina-servers-"));
  process.env.XDG_CONFIG_HOME = join(config, "config");
  process.env.PATINA_DATA_DIR = join(config, "data");
});

afterAll(() => {
  for (const app of apps) app.dispose();
  for (const m of mocks) m.proc.kill();
  for (const d of dirs) rmSync(d, { recursive: true, force: true });
  if (saved.config === undefined) delete process.env.XDG_CONFIG_HOME;
  else process.env.XDG_CONFIG_HOME = saved.config;
  if (saved.data === undefined) delete process.env.PATINA_DATA_DIR;
  else process.env.PATINA_DATA_DIR = saved.data;
  rmSync(config, { recursive: true, force: true });
});

// The mock server is a Python script, as in the Rust test.
const t = python ? test : test.skip;
describe("servers", () => {
  t("a search groups a title across servers with each servers copy", async () => {
    const [alpha, beta] = [await startMock("Alpha", "alpha-search", 0), await startMock("Beta", "beta-search", 1)];
    const app = appFor(alpha, [beta]);

    let updates = 0;
    const found = await xsearch.searchAll(app, "e", () => {
      updates += 1;
    });
    expect(found.done).toBe(true);
    expect(updates).toBeGreaterThanOrEqual(3); // the start and each server's answer are reported
    expect(found.servers.every((s) => s.state === "done")).toBe(true);

    // Ten titles are on both servers, each in its own quality.
    const shared = found.groups.filter((g) => g.copies.length === 2);
    expect(shared.length).toBeGreaterThan(0); // a title was found on both servers
    const group = shared[0]!;
    expect(group.copies[0]!.current).toBe(true); // the connected server's copy comes first
    expect(Metadata.serverId(group.copies[0]!.item)).toBeNull();
    expect(Metadata.serverId(group.copies[1]!.item)).toBe("beta-search");
    expect(group.copies[0]!.versions[0]!.resolution).toBe("720p");
    expect(group.copies[1]!.versions[0]!.resolution).toBe("1080p");

    // Titles only one server has are still listed.
    expect(found.groups.some((g) => g.copies.length === 1 && Metadata.serverId(g.copies[0]!.item) === "beta-search")).toBe(true);
    expect(found.groups.some((g) => g.copies.length === 1 && g.copies[0]!.current)).toBe(true);

    // The other server's copy is fetched from that server: the same rating key means another title on this one.
    const copy = group.copies[1]!.item;
    const there = await (await servers.serverOf(app, copy)).metadata(Metadata.ratingKey(copy), {});
    expect(there!.title).toBe(group.title);
    const here = await app.server().metadata(Metadata.ratingKey(copy), {});
    expect(here!.title).not.toBe(group.title); // the key must not be looked up on the connected server
  });

  t("a slow server times out while the others answer", async () => {
    const alpha = await startMock("Alpha", "alpha-slow", 0);
    const slow = await startMock("Slow", "slow-slow", 1, ["--search-delay", "4"]);
    const beta = await startMock("Beta", "beta-slow", 2);
    const app = appFor(alpha, [slow, beta]);

    const began = Date.now();
    let seenPending = false;
    const found = await xsearch.searchAllWithin(app, "e", 800, (r) => {
      seenPending ||= r.servers.some((s) => s.state === "pending") && r.groups.length > 0;
    });
    expect(Date.now() - began).toBeLessThan(3000); // the slow server must not hold the search
    expect(found.done).toBe(true);
    const state = (id: string) => found.servers.find((s) => s.id === id)!.state;
    expect(state("slow-slow")).toBe("timedOut");
    expect(state("alpha-slow")).toBe("done");
    expect(state("beta-slow")).toBe("done");
    expect(seenPending).toBe(true); // partial results are reported before the slow server finishes
    expect(found.groups.length).toBeGreaterThan(0);
    expect(found.groups.every((g) => g.copies.every((c) => c.serverId !== "slow-slow"))).toBe(true);
  });

  t("watch state changes and their undo stay on their own server", async () => {
    const [alpha, beta] = [await startMock("Alpha", "alpha-watch", 0), await startMock("Beta", "beta-watch", 1)];
    const app = appFor(alpha, [beta]);
    const here = app.server();
    const there = await servers.serverFor(app, "beta-watch");
    const watched = async (s: PlexServer) => Metadata.isWatched((await s.metadata("112", {}))!);

    // Marking the other server's item leaves this server's item with the same key alone.
    await undo.setWatched(app, "beta-watch", ["112"], true);
    expect(await watched(there)).toBe(true);
    expect(await watched(here)).toBe(false);
    const last = undo.last(app);
    expect(last).not.toBeNull(); // the change is undoable
    expect(last![1]).toContain("watched");
    await undo.undo(app, null);
    expect(await watched(there)).toBe(false);
    expect(undo.last(app)).toBeNull();

    // Bulk: several at once, undone together. Whatever was watched before stays watched.
    const keys = ["112", "113", "114"];
    const states = async () => {
      const out: boolean[] = [];
      for (const key of keys) out.push(Metadata.isWatched((await here.metadata(key, {}))!));
      return out;
    };
    const before = await states();
    expect(before).toContain(false); // the test needs an unwatched item
    await undo.setWatched(app, null, keys, true);
    expect(await states()).toEqual([true, true, true]);
    await undo.undo(app, null);
    expect(await states()).toEqual(before);
  });

  t("playlist entries come back where they were and resume points return", async () => {
    const alpha = await startMock("Alpha", "alpha-list", 0);
    const app = appFor(alpha, []);
    const server = app.server();
    const order = async () => (await server.playlistItems("950", 0, 100)).Metadata.map((m) => Metadata.ratingKey(m));
    expect(await order()).toEqual(["104", "105", "1012"]);

    // The middle entry goes back between its neighbours.
    await undo.removeFromPlaylist(app, "950", 7105);
    expect(await order()).toEqual(["104", "1012"]);
    await undo.undo(app, null);
    expect(await order()).toEqual(["104", "105", "1012"]);

    // So does the first.
    await undo.removeFromPlaylist(app, "950", 7104);
    expect(await order()).toEqual(["105", "1012"]);
    await undo.undo(app, null);
    expect(await order()).toEqual(["104", "105", "1012"]);

    // Hiding an item from Continue Watching and clearing its progress give the resume point back.
    const offset = async (s: PlexServer) => (await s.metadata("100", {}))!.viewOffset ?? 0;
    const before = await offset(server);
    expect(before).toBeGreaterThan(0); // the mock item has a resume point
    await undo.clearProgress(app, "100");
    expect(await offset(server)).toBe(0);
    await undo.undo(app, null);
    expect(await offset(server)).toBe(before);

    await undo.removeFromContinueWatching(app, "100");
    await undo.undo(app, null);
    expect(await offset(server)).toBe(before);
  });

  t("undo history is dropped when the server changes", async () => {
    const [alpha, beta] = [await startMock("Alpha", "alpha-scope", 0), await startMock("Beta", "beta-scope", 1)];
    const app = appFor(alpha, [beta]);
    await undo.setWatched(app, null, ["112"], true);
    expect(undo.last(app)).not.toBeNull();

    // A different session on the same state: the old entries must not apply to the new server.
    app.setSession(sessionFor(beta, [alpha]), []);
    expect(undo.last(app)).toBeNull();
    await expect(undo.undo(app, null)).rejects.toThrow();
  });

  t("an action finishing after a switch is not undoable on the new server", async () => {
    const alpha = await startMock("Alpha", "alpha-late", 0, ["--report-delay", "1"]);
    const beta = await startMock("Beta", "beta-late", 1);
    const app = appFor(alpha, [beta]);
    // Alpha holds the scrobble; the switch to Beta happens while it is in flight.
    const marking = undo.setWatched(app, null, ["112"], true);
    await Bun.sleep(300);
    app.setSession(sessionFor(beta, [alpha]), []);
    await marking;
    // Undo would unmark Beta's "112", a different item.
    expect(undo.last(app)).toBeNull();
  });

  t("a playback failure is not retried on another server", async () => {
    const [alpha, beta] = [await startMock("Alpha", "alpha-failure", 0), await startMock("Beta", "beta-failure", 1)];
    const app = appFor(alpha, [beta]);
    const item = (await app.server().metadata("112", {}))!;
    const onAlpha = Scope.of(app.session()!);
    const report = (): Report => ({
      item,
      error: "loading failed",
      kind: null,
      phase: "start",
      positionMs: 61_000,
      mediaIndex: 0,
      quality: "original",
      remaining: 0,
      request: { item } as PlayRequest,
      scope: onAlpha.clone(),
    });

    // Connecting to the same server again keeps the failure on offer.
    failure.record(app, report());
    app.setSession(app.session(), []);
    expect(failure.current()).not.toBeNull();

    // Another server: its rating key "112" is a different item, so the failure is dropped.
    app.setSession(sessionFor(beta, [alpha]), []);
    expect(failure.current()).toBeNull();
    expect(() => failure.recover(app, { kind: "retry" })).toThrow();

    // Alpha's playback failing late, after the switch, is not offered on Beta either.
    failure.record(app, report());
    expect(failure.current()).toBeNull();
  });
});
