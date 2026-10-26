import { expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { ClientInfo, PlexServer } from "../plex";
import {
  MAX_FETCHES,
  Semaphore,
  SingleFlight,
  cachePath,
  cachedFile,
  currentServer,
  fetch as fetchImage,
  pruneDir,
  readCached,
  setServer,
  writeAtomic,
} from "./images";

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

test("simultaneous requests share one fetch", async () => {
  const flights = new SingleFlight<number>();
  let started = 0;
  const calls = Array.from({ length: 20 }, () =>
    flights.run("poster", async () => {
      started += 1;
      await sleep(50);
      return 7;
    }),
  );
  const results = await Promise.all(calls);
  expect(results.every((r) => r === 7)).toBe(true);
  expect(started).toBe(1);
  expect(flights.running()).toBe(0); // a finished key must be forgotten
  // A later request starts again.
  expect(await flights.run("poster", async () => 8)).toBe(8);
});

test("different keys run independently and work survives abandonment", async () => {
  const flights = new SingleFlight<number>();
  let done = 0;
  // Nobody waits for "a": dropping the promise must not cancel the work.
  void flights.run("a", async () => {
    await sleep(30);
    done += 1;
    return 1;
  });
  const b = await flights.run("b", async () => 2);
  expect(b).toBe(2);
  await sleep(100);
  expect(done).toBe(1);
  expect(flights.running()).toBe(0);
});

test("a panicking task is reported and forgotten", async () => {
  const flights = new SingleFlight<number>();
  const r = await flights.run("x", async () => {
    throw new Error("boom");
  });
  expect(r).toBeNull();
  expect(flights.running()).toBe(0); // a panic must not wedge the key
  expect(await flights.run("x", async () => 3)).toBe(3);
});

test("fetches are bounded", async () => {
  let running = 0;
  let peak = 0;
  const permits = new Semaphore(3);
  const flights = new SingleFlight<void>();
  const calls = Array.from({ length: 12 }, (_, i) =>
    flights.run(`k${i}`, async () => {
      const release = await permits.acquire();
      running += 1;
      peak = Math.max(peak, running);
      await sleep(20);
      running -= 1;
      release();
    }),
  );
  await Promise.all(calls);
  expect(peak).toBe(3);
});

function entry(dir: string, name: string, bytes: number, usedMs: number) {
  const sub = join(dir, name.slice(0, 2));
  mkdirSync(sub, { recursive: true });
  const path = join(sub, name);
  writeFileSync(path, Buffer.alloc(bytes, 1));
  // A new file's access time is now; set both so the entry looks last used at `used`.
  utimesSync(path, usedMs / 1000, usedMs / 1000);
}

function names(dir: string): string[] {
  return readdirSync(dir)
    .flatMap((d) => readdirSync(join(dir, d)))
    .sort();
}

test("prune drops old entries and dead temporary files", () => {
  const tmp = mkdtempSync(join(tmpdir(), "patina-images-"));
  const now = Date.now();
  const day = 86_400_000;
  entry(tmp, "aa-fresh", 10, now - day);
  entry(tmp, "bb-stale", 10, now - 40 * day);
  entry(tmp, "cc-dead.tmp-1", 10, now - 2 * day);
  entry(tmp, "dd-writing.tmp-2", 10, now);
  const pruned = pruneDir(tmp, now, 30 * day, Number.MAX_SAFE_INTEGER);
  expect(pruned).toEqual({ files: 2, bytes: 20 });
  expect(names(tmp)).toEqual(["aa-fresh", "dd-writing.tmp-2"]);
  rmSync(tmp, { recursive: true });
});

test("prune enforces the size budget oldest first", () => {
  const tmp = mkdtempSync(join(tmpdir(), "patina-images-"));
  const now = Date.now();
  const hour = 3_600_000;
  for (let i = 0; i < 10; i++) {
    // e0 is the most recently used, e9 the least.
    entry(tmp, `e${i}`, 100, now - hour * (i + 1));
  }
  // 1000 bytes against a 500 budget trims to 90% of it (450): the six least recently used go.
  const pruned = pruneDir(tmp, now, 86_400_000 * 30, 500);
  expect(pruned.bytes).toBe(600);
  expect(names(tmp)).toEqual(["e0", "e1", "e2", "e3"]);
  // Under budget: nothing changes.
  expect(pruneDir(tmp, now, 86_400_000 * 30, 500)).toEqual({ files: 0, bytes: 0 });
  rmSync(tmp, { recursive: true });
});

test("atomic writes leave no temporary files", async () => {
  const tmp = mkdtempSync(join(tmpdir(), "patina-images-"));
  const path = join(tmp, "ab", "abcdef");
  await writeAtomic(path, new TextEncoder().encode("first"));
  await writeAtomic(path, new TextEncoder().encode("second"));
  expect(readFileSync(path, "utf8")).toBe("second");
  expect(names(tmp)).toEqual(["abcdef"]);
  // An unwritable destination cleans up after itself.
  const blocked = join(tmp, "ab", "abcdef", "child");
  await expect(writeAtomic(blocked, new TextEncoder().encode("x"))).rejects.toThrow();
  expect(names(tmp)).toEqual(["abcdef"]);
  rmSync(tmp, { recursive: true });
});

/** A server answering every request with a small PNG after a delay, counting requests and the most it served at once. */
function slowImageServer(delayMs: number) {
  const stats = { requests: 0, peak: 0, active: 0 };
  const body = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 1, 2, 3, 4]);
  const server = Bun.serve({
    port: 0,
    async fetch() {
      stats.requests += 1;
      stats.active += 1;
      stats.peak = Math.max(stats.peak, stats.active);
      await sleep(delayMs);
      stats.active -= 1;
      return new Response(body, { headers: { "Content-Type": "image/png" } });
    },
  });
  return { url: `http://127.0.0.1:${server.port}`, stats, server };
}

test("fetch shares requests, bounds concurrency and caches", async () => {
  const { url, stats, server } = slowImageServer(150);
  setServer(new PlexServer(new ClientInfo("test", "Patina", "test"), url, "token"));
  const run = crypto.randomUUID().replaceAll("-", "");

  // Thirty simultaneous requests for one image make one request.
  const same = `/library/metadata/1/thumb/${run}`;
  const results = await Promise.all(Array.from({ length: 30 }, () => fetchImage(same, 300, 450)));
  expect(results.every((r) => r !== null && r[0].length === 8 && r[1] === "image/png")).toBe(true);
  expect(stats.requests).toBe(1);
  // And it is cached, so asking again makes none.
  expect(await fetchImage(same, 300, 450)).not.toBeNull();
  expect(stats.requests).toBe(1);
  const file = await cachedFile(same, 300, 450);
  expect(file).not.toBeNull();
  expect(readFileSync(file!).length).toBe(8);

  // A grid's worth of different images never has more than MAX_FETCHES in flight.
  const many = Array.from({ length: 4 * MAX_FETCHES }, (_, i) => `/library/metadata/${100 + i}/thumb/${run}`);
  const all = await Promise.all(many.map((s) => fetchImage(s, 300, 450)));
  expect(all.every((r) => r !== null)).toBe(true);
  expect(stats.requests).toBe(1 + many.length);
  expect(stats.peak).toBeLessThanOrEqual(MAX_FETCHES);
  expect(stats.peak).toBeGreaterThan(1); // fetches should still overlap

  const current = currentServer();
  for (const s of [...many, same]) rmSync(cachePath(current, s, 300, 450), { force: true });
  setServer(null);
  await server.stop(true);
});

test("empty cache files are misses", async () => {
  const tmp = mkdtempSync(join(tmpdir(), "patina-images-"));
  const path = join(tmp, "empty");
  writeFileSync(path, "");
  expect(await readCached(path)).toBeNull();
  writeFileSync(path, new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0, 0]));
  const hit = await readCached(path);
  expect(hit).not.toBeNull();
  expect([hit![0].length, hit![1]]).toEqual([6, "image/png"]);
  rmSync(tmp, { recursive: true });
});

test("the cache key is the sha256 of scope|source|w|h in a two-level folder", () => {
  const p = cachePath(null, "/a", 1, 2);
  const hex = require("node:crypto").createHash("sha256").update("|/a|1|2").digest("hex");
  expect(p.endsWith(join(hex.slice(0, 2), hex))).toBe(true);
});
