import { expect, test } from "bun:test";
import { mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { lockPath, readJson, replaceJson, tryLock, updateJson, write } from "./storage";

function tempDir(): string {
  return mkdtempSync(join(tmpdir(), "patina-storage-"));
}

test("unchanged transactions do not replace the file", () => {
  const dir = tempDir();
  const path = join(dir, "state.json");
  writeFileSync(path, "[1,2]");
  updateJson<number[], void>(path, () => [], () => {});
  expect(readFileSync(path, "utf8")).toBe("[1,2]");
  rmSync(dir, { recursive: true });
});

test("concurrent writers preserve every update", async () => {
  const dir = tempDir();
  const path = join(dir, "counter.json");
  // Separate processes, like the TUI and the desktop app: only the OS lock keeps them apart.
  const code = `
    import { replaceJson } from ${JSON.stringify(join(import.meta.dir, "storage.ts"))};
    for (let i = 0; i < 20; i++) replaceJson(${JSON.stringify(path)}, 0, (n) => n + 1);
  `;
  const procs = Array.from({ length: 8 }, () => Bun.spawn([process.execPath, "-e", code], { stdout: "inherit", stderr: "inherit" }));
  for (const p of procs) expect(await p.exited).toBe(0);
  expect(readJson<number>(path, 0)).toBe(160);
  // Also within one process.
  for (let i = 0; i < 5; i++) replaceJson<number>(path, 0, (n) => n + 1);
  expect(readJson<number>(path, 0)).toBe(165);
  rmSync(dir, { recursive: true });
});

test("corrupt state is not overwritten", () => {
  const dir = tempDir();
  const path = join(dir, "state.json");
  writeFileSync(path, "broken");
  expect(() => updateJson<string[], void>(path, () => [], (v) => v.splice(0))).toThrow();
  expect(readFileSync(path, "utf8")).toBe("broken");
  rmSync(dir, { recursive: true });
});

test("transfer lease excludes another writer", () => {
  const dir = tempDir();
  const path = join(dir, "worker");
  const first = tryLock(path);
  expect(first).not.toBeNull();
  expect(tryLock(path)).toBeNull();
  first!.release();
  const again = tryLock(path);
  expect(again).not.toBeNull();
  again!.release();
  rmSync(dir, { recursive: true });
});

test("lock files sit next to the state with the extension replaced", () => {
  expect(lockPath("/a/b/config.toml")).toBe("/a/b/config.lock");
  expect(lockPath("/a/b/offline-sync")).toBe("/a/b/offline-sync.lock");
  expect(lockPath("/a/b/downloads.json")).toBe("/a/b/downloads.lock");
});

test("writes are private and atomic", () => {
  const dir = tempDir();
  const path = join(dir, "sub", "file");
  write(path, new TextEncoder().encode("one"));
  write(path, new TextEncoder().encode("two"));
  expect(readFileSync(path, "utf8")).toBe("two");
  if (process.platform !== "win32") expect(statSync(path).mode & 0o777).toBe(0o600);
  rmSync(dir, { recursive: true });
});
