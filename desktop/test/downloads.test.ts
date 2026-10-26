// Real HTTP against tools/mock_pms.py; isolated subprocesses keep global config/player state out of
// other tests (port of the download and offline tests of crates/core/tests/reliability.rs).
//
// `download_and_offline_worker` and `restart_worker` do the actual work. They only run when the
// parent test started them as `bun test` subprocesses (PATINA_TEST_SERVER is set), exactly like the
// Rust test binary re-executing itself with `--exact <name>`; a plain run of the file passes them
// trivially. The first worker process exits with a second transfer still unfinished; the second one
// must pick it up from the manifest and finish it.

import { expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { open } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { ClientInfo, Metadata, PlexServer } from "../plex";
import * as config from "../core/config";
import * as downloads from "../core/downloads";
import * as offline from "../core/offline";
import { type AppState as AppStateType, AppState, type CoreEvent, type EventSink, type Session } from "../core/state";
import * as storage from "../core/storage";
import { hasPython, startMockPms } from "./mock-pms";

const MOVIE_BYTES = 8 * 1024 * 1024;

const sleep = (ms: number) => Bun.sleep(ms);

/** Run one of this file's worker tests in a fresh `bun test` process (the Rust tests re-run their own binary). */
async function runWorker(name: string, env: Record<string, string>): Promise<{ code: number; output: string }> {
  const proc = Bun.spawn([process.execPath, "test", import.meta.path, "--test-name-pattern", `^${name}$`], {
    env: { ...process.env, ...env },
    stdin: "ignore",
    stdout: "pipe",
    stderr: "pipe",
  });
  const [out, err, code] = await Promise.all([new Response(proc.stdout).text(), new Response(proc.stderr).text(), proc.exited]);
  return { code, output: `${out}${err}` };
}

async function downloadReliability(validator: "normal" | "missing" | "changed"): Promise<void> {
  const temp = mkdtempSync(join(tmpdir(), "patina-dl-"));
  const media = join(temp, "media");
  mkdirSync(media, { recursive: true });
  writeFileSync(join(media, "movie.mkv"), new Uint8Array(MOVIE_BYTES).fill(42));
  const server = await startMockPms({
    media,
    dir: temp,
    args: ["--download-delay", "0.012", "--drop-download-once", "--resume-validator", validator, "--missing-metadata", "999998"],
  });
  try {
    // Everything the workers write (config, data, caches, downloads) stays below `temp`.
    const base = {
      PATINA_TEST_SERVER: server.url,
      HOME: join(temp, "home"),
      XDG_CONFIG_HOME: join(temp, "config"),
      XDG_DATA_HOME: join(temp, "xdg-data"),
      XDG_CACHE_HOME: join(temp, "xdg-cache"),
      PATINA_DATA_DIR: join(temp, "data"),
      PATINA_TEST_DOWNLOADS: join(temp, "files"),
    };
    mkdirSync(base.HOME, { recursive: true });
    const result = await runWorker("download_and_offline_worker", { ...base, PATINA_TEST_RESUME_VALIDATOR: validator });
    const recovery = await runWorker("restart_worker", base);
    await server.stop();
    expect(result.code, `${server.log()}\n${result.output}`).toBe(0);
    expect(recovery.code, `${server.log()}\n${recovery.output}`).toBe(0);
    // A worker that matched no test would "pass" without doing anything.
    expect(result.output).toContain("1 pass");
    expect(recovery.output).toContain("1 pass");
    const events = server.events();
    expect(events.includes("bytes="), `resume must issue a Range request: ${events}`).toBe(true);
  } finally {
    await server.stop();
    rmSync(temp, { recursive: true, force: true });
  }
}

const python = hasPython();

test.skipIf(!python)("interrupted_downloads_and_offline_sync", () => downloadReliability("normal"), 120_000);

test.skipIf(!python)("resumed_download_without_validator_restarts", () => downloadReliability("missing"), 120_000);

test.skipIf(!python)("resumed_download_with_changed_validator_restarts", () => downloadReliability("changed"), 120_000);

const Sink: EventSink = {
  emit(_: CoreEvent) {},
};

function session(url: string, token: string): Session {
  return {
    tv: null,
    server: new PlexServer(new ClientInfo("test", "Patina", "test"), url, token),
    serverId: "mock-machine-0001",
    serverName: "Mock",
    user: null,
    owned: true,
    servers: [],
  };
}

const first = <T>(list: T[]): T => {
  const x = list[0];
  if (x === undefined) throw new Error("expected a download");
  return x;
};

test("download_and_offline_worker", async () => {
  const url = process.env.PATINA_TEST_SERVER;
  if (!url) return;
  const app = new AppState(Sink);
  app.updateSettings((s) => {
    s.player.downloads_dir = process.env.PATINA_TEST_DOWNLOADS ?? "";
  });
  app.setSession(session(url, "alice"), []);
  await downloads.enqueue(app, "100");
  const start = Date.now();
  for (;;) {
    const d = first(downloads.list());
    if (d.received > 0) {
      downloads.pause(app, d.id);
      await sleep(350);
      expect(first(downloads.list()).state).toBe("paused");
      const partial = `${d.file}.part`;
      expect(statSync(partial).size).toBeGreaterThan(0);
      if (process.env.PATINA_TEST_RESUME_VALIDATOR !== "normal") {
        // Simulate a partial from a different source with the same length.
        const f = await open(partial, "r+");
        await f.write(new Uint8Array(1024), 0, 1024, 0);
        await f.close();
      }
      downloads.retry(app, d.id);
      break;
    }
    if (Date.now() - start >= 15_000) throw new Error(`download did not start: ${JSON.stringify(d)}`);
    await sleep(50);
  }
  let d = first(downloads.list());
  for (;;) {
    d = first(downloads.list());
    if (d.state === "done") break;
    if (d.state === "failed") throw new Error(`failed: ${JSON.stringify(d)}`);
    if (Date.now() - start >= 30_000) throw new Error(`timed out: ${JSON.stringify(d)}`);
    await sleep(50);
  }
  expect(new Uint8Array(readFileSync(d.file!))).toEqual(new Uint8Array(MOVIE_BYTES).fill(42));
  app.setSession(session(url, "bob"), []);
  // Both a 404 and an empty metadata response must leave later outbox entries usable.
  for (const key of ["999998", "999999"]) {
    const missing = structuredClone(d);
    missing.id = `${d.id}:${key}`;
    missing.item.ratingKey = key;
    offline.record(missing, 45000, 180000, false, 0.9);
  }
  offline.record(d, 90000, 180000, false, 0.9);
  expect(offline.position(d)).toBe(90000);
  await offline.sync(app);
  const server = session(url, "alice").server;
  const item = await server.metadata("100");
  expect(item?.viewOffset, "other profile must not replay Alice's progress").toBe(61000);
  app.setSession(session(url, "alice"), []);
  await syncUntil(app, server, (item) => item.viewOffset === 90000);
  const progress = storage.readJson<offline.Progress[]>(join(config.configDir(), "offline-progress.json"), []);
  for (const key of ["999998", "999999"]) {
    const missing = progress.find((p) => p.rating_key === key);
    expect(missing).toBeDefined();
    expect(missing!.pending, "missing titles must be retired from the outbox").toBe(false);
    expect(missing!.position, "local resume points must survive removal from Plex").toBe(45000);
  }
  offline.record(d, 179000, 180000, true, 0.9);
  await syncUntil(app, server, (item) => Metadata.isWatched(item));
  expect(offline.position(d)).toBe(0);
  // Leave a second transfer unfinished, then exit this process entirely.
  await downloads.enqueue(app, "101");
  const began = Date.now();
  for (;;) {
    if (downloads.list().some((d) => d.item.ratingKey === "101" && d.state === "downloading" && d.received > 0)) break;
    if (Date.now() - began >= 10_000) throw new Error("second transfer did not start");
    await sleep(50);
  }
}, 120_000);

/** The background synchronizer may own the lease; retry until the server acknowledges the change. */
async function syncUntil(app: AppStateType, server: PlexServer, expected: (item: Metadata) => boolean): Promise<void> {
  const began = Date.now();
  for (;;) {
    await offline.sync(app);
    const item = await server.metadata("100");
    if (item && expected(item)) return;
    if (Date.now() - began >= 10_000) throw new Error("offline progress was not replayed");
    await sleep(100);
  }
}

test("restart_worker", async () => {
  const url = process.env.PATINA_TEST_SERVER;
  if (!url) return;
  const app = new AppState(Sink);
  app.setSession(session(url, "alice"), []);
  downloads.resume(app);
  const start = Date.now();
  for (;;) {
    const d = downloads.list().find((d) => d.item.ratingKey === "101");
    if (!d) throw new Error("interrupted transfer persisted");
    if (d.state === "done") {
      expect(new Uint8Array(readFileSync(d.file!))).toEqual(new Uint8Array(MOVIE_BYTES).fill(42));
      break;
    }
    if (d.state === "failed") throw new Error(`failed: ${JSON.stringify(d)}`);
    if (Date.now() - start >= 15_000) throw new Error("restart did not finish");
    await sleep(50);
  }
}, 60_000);
