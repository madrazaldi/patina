// Sign-in flow against tools/mock_pms.py: manual server, restore on startup, sign out.
import { afterAll, beforeAll, expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import * as images from "../core/images";
import * as live from "../core/live";
import * as session from "../core/session";
import { AppState } from "../core/state";
import type { CoreEvent } from "../core/state";

const python = Bun.which("python3");
const script = join(import.meta.dir, "..", "..", "tools", "mock_pms.py");
let proc: ReturnType<typeof Bun.spawn> | undefined;
let url = "";
let media = "";
let config = "";
const saved = { config: process.env.XDG_CONFIG_HOME, data: process.env.PATINA_DATA_DIR };

beforeAll(async () => {
  if (!python) return;
  config = mkdtempSync(join(tmpdir(), "patina-session-"));
  process.env.XDG_CONFIG_HOME = join(config, "config");
  process.env.PATINA_DATA_DIR = join(config, "data");
  media = mkdtempSync(join(tmpdir(), "patina-session-media-"));
  writeFileSync(join(media, "movie.mkv"), new Uint8Array(1024).fill(42));
  proc = Bun.spawn([python, script, "--port", "0", "--media", media], { stdout: "ignore", stderr: "pipe" });
  const reader = (proc.stderr as ReadableStream<Uint8Array>).getReader();
  let log = "";
  while (!log.includes("\n")) {
    const { value, done } = await reader.read();
    if (done) break;
    log += new TextDecoder().decode(value);
  }
  url = log.split("\n")[0]!.trim().replace("mock PMS on ", "");
  void (async () => {
    for (;;) if ((await reader.read().catch(() => ({ done: true }))).done) return;
  })();
});

afterAll(() => {
  proc?.kill();
  if (saved.config === undefined) delete process.env.XDG_CONFIG_HOME;
  else process.env.XDG_CONFIG_HOME = saved.config;
  if (saved.data === undefined) delete process.env.PATINA_DATA_DIR;
  else process.env.PATINA_DATA_DIR = saved.data;
  if (config) rmSync(config, { recursive: true, force: true });
  if (media) rmSync(media, { recursive: true, force: true });
});

const t = python ? test : test.skip;

t("a manual server connects, is remembered, restores and signs out", async () => {
  const events: CoreEvent[] = [];
  const app = new AppState({ emit: (e) => events.push(e) });
  try {
    await session.restore(app);
    expect(app.snapshot().stage.kind).toBe("login");

    await session.signInManual(app, url.replace("http://", ""), " x ");
    expect(app.snapshot().stage.kind).toBe("ready");
    expect(app.session()?.owned).toBe(true);
    expect(app.sections().length).toBeGreaterThan(0);
    const settings = app.settings();
    expect(settings.server.manual_url).toBe(url);
    expect(settings.server.manual_token).toBe("x");
    expect(settings.auth.account_id).toBeTruthy();
    expect(images.currentServer()).not.toBeNull();
    expect(live.listening(app)).toBe(true);
    expect(events.some((e) => e.kind === "session")).toBe(true);

    // A new process restores the saved manual server.
    const again = new AppState({ emit() {} });
    try {
      await session.restore(again);
      expect(again.snapshot().stage.kind).toBe("ready");
      expect(again.session()?.serverName.length).toBeGreaterThan(0);
      live.stop(again);
    } finally {
      again.dispose();
    }

    session.signOut(app);
    expect(app.snapshot().stage.kind).toBe("login");
    expect(app.session()).toBeNull();
    expect(app.settings().server.manual_url).toBeNull();
    expect(images.currentServer()).toBeNull();
    expect(live.listening(app)).toBe(false);
  } finally {
    live.stop(app);
    app.dispose();
  }
}, 20_000);

t("a manual server that is down shows an error on the form", async () => {
  const app = new AppState({ emit() {} });
  try {
    await expect(session.signInManual(app, "127.0.0.1:9", "x")).rejects.toThrow(/^contacting server: /);
    expect(app.settings().server.manual_url).toBeNull();
  } finally {
    app.dispose();
  }
});
