import { afterEach, beforeEach, expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { AppState, Changes, eventName, eventPayload } from "./state";
import type { CoreEvent } from "./state";

let dir: string;
let saved: string | undefined;
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "patina-state-"));
  saved = process.env.XDG_CONFIG_HOME;
  process.env.XDG_CONFIG_HOME = dir;
});
afterEach(() => {
  if (saved === undefined) delete process.env.XDG_CONFIG_HOME;
  else process.env.XDG_CONFIG_HOME = saved;
  rmSync(dir, { recursive: true, force: true });
});

test("event names and payloads are what the webview listens for", () => {
  const cases: Array<[CoreEvent, string, unknown]> = [
    [{ kind: "connection", online: false }, "connection", false],
    [{ kind: "settingsChanged" }, "settings-changed", null],
    [{ kind: "stage", stage: { kind: "ready" } }, "stage", { kind: "ready" }],
    [{ kind: "revision", revision: 3 }, "revision", 3],
    [{ kind: "changes", changes: Changes.section("1", "10") }, "library-changed", { all: false, sections: ["1"], items: ["10"] }],
    [{ kind: "toast", toast: { text: "hi", error: false } }, "toast", { text: "hi", error: false }],
    [{ kind: "sessionsChanged" }, "sessions-changed", null],
    [{ kind: "session", session: null, sections: [] }, "session", { session: null, sections: [] }],
  ];
  for (const [event, name, payload] of cases) {
    expect(eventName(event)).toBe(name);
    expect(eventPayload(event)).toEqual(payload);
  }
});

test("setters store the value and then emit", () => {
  const seen: CoreEvent[] = [];
  const app = new AppState({ emit: (e) => seen.push(e) });
  try {
    expect(app.snapshot().stage).toEqual({ kind: "booting" });
    app.setStage({ kind: "login" });
    app.toast("a");
    app.toastUndo("b", 7);
    app.error("c");
    app.bumpWith(Changes.everything());
    const names = seen.map(eventName);
    expect(names).toEqual(["stage", "toast", "toast", "toast", "library-changed", "revision"]);
    expect(eventPayload(seen[2]!)).toEqual({ text: "b", error: false, undo: 7 });
    expect(eventPayload(seen[3]!)).toEqual({ text: "c", error: true });
    const snap = app.snapshot();
    expect(snap.stage).toEqual({ kind: "login" });
    expect(snap.revision).toBe(1);
    expect(snap.connected).toBe(true);
    expect(() => app.server()).toThrow("Not connected to a server.");
    expect(() => app.plexTv()).toThrow("Not signed in to plex.tv.");
    app.setConnected(false);
    expect(seen.at(-1)).toEqual({ kind: "connection", online: false });
  } finally {
    app.dispose();
  }
});

test("settings updates are saved to the shared file and re-read", () => {
  const app = new AppState({ emit() {} });
  try {
    app.updateSettings((s) => {
      s.player.quality = "4m-720";
    });
    expect(app.settings().player.quality).toBe("4m-720");
  } finally {
    app.dispose();
  }
});

test("spawn logs failures instead of leaving unhandled rejections", async () => {
  const app = new AppState({ emit() {} });
  try {
    expect(await app.spawn(Promise.resolve(5))).toBe(5);
    expect(await app.spawn(Promise.reject(new Error("boom")))).toBeUndefined();
    await app.settled();
  } finally {
    app.dispose();
  }
});
