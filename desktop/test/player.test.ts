// Real mpv against tools/mock_pms.py: ports of the `#[ignore]`d tests of
// crates/core/tests/reliability.rs (mpv_worker, reports_worker, queue_switch_worker,
// transcode_tracks_worker, failure_worker). They need mpv, ffmpeg and python3, and only run with
// PATINA_MPV_TESTS=1 (the Rust ones run with `--ignored`). mpv plays headless (--vo=null --ao=null).
import { afterEach, beforeEach, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import * as config from "../core/config";
import * as controls from "../core/controls";
import * as downloads from "../core/downloads";
import * as failure from "../core/failure";
import * as offline from "../core/offline";
import * as playback from "../core/ops/playback";
import * as player from "../core/player";
import * as prefs from "../core/prefs";
import * as report from "../core/report";
import * as servers from "../core/servers";
import { AppState, type CoreEvent, type Session } from "../core/state";
import * as storage from "../core/storage";
import { ClientInfo, Metadata as M, type Metadata, MetadataOptions, PlexServer, defaultMetadata } from "../plex";
import { type MockPms, hasPython, startMockPms } from "./mock-pms";

const enabled = process.env.PATINA_MPV_TESTS === "1" && hasPython() && !!Bun.which("mpv") && !!Bun.which("ffmpeg");
const t = enabled ? test : test.skip;

const saved = { config: process.env.XDG_CONFIG_HOME, data: process.env.PATINA_DATA_DIR };
let temp = "";
const mocks: MockPms[] = [];
const apps: AppState[] = [];

beforeEach(() => {
  temp = mkdtempSync(join(tmpdir(), "patina-mpv-"));
  process.env.XDG_CONFIG_HOME = join(temp, "config");
  process.env.PATINA_DATA_DIR = join(temp, "data");
});

afterEach(async () => {
  for (const app of apps.splice(0)) {
    await player.shutdown().catch(() => {});
    app.dispose();
  }
  for (const m of mocks.splice(0)) await m.stop();
  for (const [k, v] of [
    ["XDG_CONFIG_HOME", saved.config],
    ["PATINA_DATA_DIR", saved.data],
  ] as const) {
    if (v === undefined) delete process.env[k];
    else process.env[k] = v;
  }
  rmSync(temp, { recursive: true, force: true });
});

class Recorder {
  events: [string, unknown][] = [];
  emit = (event: CoreEvent) => {
    this.events.push([event.kind, event]);
  };
  last(kind: string): CoreEvent | undefined {
    return this.events.findLast(([k]) => k === kind)?.[1] as CoreEvent | undefined;
  }
}

function newApp(sink: { emit(e: CoreEvent): void } = { emit() {} }): AppState {
  const app = new AppState(sink);
  app.updateSettings((s) => {
    s.player.extra_args = "--no-config --vo=null --ao=null";
  });
  apps.push(app);
  return app;
}

function sine(path: string, seconds: number): void {
  const r = Bun.spawnSync(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", `sine=frequency=440:duration=${seconds}`, path]);
  expect(r.exitCode).toBe(0);
}

const session = (url: string, serverId = "mock-machine-0001", name = "Mock"): Session => ({
  tv: null,
  server: new PlexServer(new ClientInfo("test", "Patina", "test"), url, "alice"),
  serverId,
  serverName: name,
  user: null,
  owned: true,
  servers: [],
});

async function waitFor(what: string, withinMs: number, done: () => boolean): Promise<void> {
  const began = performance.now();
  while (!done()) {
    if (performance.now() - began > withinMs) throw new Error(`timed out waiting for ${what}`);
    await Bun.sleep(20);
  }
}

const settle = () => report.flush(40_000);

t("mpv_playback_smoke and mpv_worker: resume, skip and persisted completion of downloads", async () => {
  const file = join(temp, "tone.wav");
  sine(file, 20);
  const app = newApp();
  app.updateSettings((s) => {
    s.auth.account_id = "alice";
  });
  const meta = (key: string): Metadata => ({ ...defaultMetadata(), ratingKey: key, title: key === "tone" ? "Tone" : "Next", type: "track", duration: 20000 });
  const d: downloads.Download = {
    id: "mock:alice:tone",
    serverId: "mock",
    serverName: "Mock",
    item: meta("tone"),
    file,
    poster: null,
    size: 0,
    received: 0,
    state: "done",
    error: null,
    addedAt: 0,
    account: "alice",
    validator: null,
    sourceKey: null,
    policy: null,
  };
  const next: downloads.Download = { ...d, id: "mock:alice:next", item: meta("next") };
  storage.updateJson<downloads.Download[], void>(join(config.configDir(), "downloads.json"), () => [], (list) => {
    list.push(d, next);
  });
  offline.record(d, 8000, 20000, false, 0.9);
  offline.record(next, 4000, 20000, false, 0.9);
  playback.playDownloads(app, [d.id, next.id]);
  await waitFor("mpv to resume from the saved position", 8000, () => {
    const n = app.nowPlaying();
    return !!n && n.positionMs > 8100 && n.positionMs < 14000;
  });
  player.control({ kind: "next" });
  await waitFor("the new entry's now-playing state", 12_000, () => {
    const n = app.nowPlaying();
    return !!n && n.queuePos === 1 && n.positionMs > 5100;
  });
  expect(offline.position(d)).toBeGreaterThan(8100);
  player.control({ kind: "seekTo", seconds: 18 });
  await waitFor("mpv to persist completion", 15_000, () => app.nowPlaying() === null && offline.position(next) === 0);
}, 60_000);

async function reportsRun(mockArgs: string[]): Promise<void> {
  const media = join(temp, "media");
  mkdirSync(media, { recursive: true });
  sine(join(media, "movie.mkv"), 40);
  const mock = await startMockPms({ media, dir: temp, args: mockArgs });
  mocks.push(mock);
  const app = newApp();
  app.setSession(session(mock.url), []);
  const server = app.server();
  const first = (await server.metadata("100", new MetadataOptions()))!;
  const second = (await server.metadata("101", new MetadataOptions()))!;
  player.play(app, player.playRequest({ item: first, resume: false }));
  await waitFor("playback to start", 15_000, () => (app.nowPlaying()?.positionMs ?? 0) > 1500);

  let began = performance.now();
  player.control({ kind: "pause" });
  await waitFor("pause", 3000, () => !!app.nowPlaying()?.paused);
  expect(performance.now() - began).toBeLessThan(1500);

  began = performance.now();
  player.queue(app, { kind: "add", item: second, next: false });
  await waitFor("the queue edit", 3000, () => app.snapshot().queue?.items.length === 2);
  expect(performance.now() - began).toBeLessThan(1500);

  player.control({ kind: "play" });
  began = performance.now();
  player.control({ kind: "next" });
  await waitFor("the next item", 3000, () => {
    const n = app.nowPlaying();
    return !!n && n.queuePos === 1 && !n.paused;
  });
  expect(performance.now() - began).toBeLessThan(1500);

  began = performance.now();
  player.control({ kind: "stop" });
  await waitFor("playback to end", 6000, () => app.nowPlaying() === null);
  expect(performance.now() - began).toBeLessThan(4000);
  await settle();
  await Bun.sleep(500);
  const events = mock.events();
  const delivered = (key: string) =>
    events.split("\n").some((l) => l.includes('"what": "timeline"') && l.includes(`"ratingKey": "${key}"`) && l.includes('"state": "stopped"'));
  if (mockArgs.includes("--report-delay")) {
    expect(delivered("100") && delivered("101")).toBe(true);
  } else {
    expect(events).toContain("failed report");
  }
  // Progress is collapsed rather than queued up behind a slow server.
  expect(events.split("\n").filter((l) => l.includes('"what": "timeline"')).length).toBeLessThanOrEqual(8);
}

/** Pause, queue edits and stopping must not wait for Plex, however slowly it answers reports. */
t("mpv_slow_reports_do_not_block_playback", () => reportsRun(["--report-delay", "4"]), 120_000);
t("mpv_failing_reports_do_not_block_playback", () => reportsRun(["--report-status", "503"]), 120_000);

/** After switching servers mid-playback, the running queue only takes items from the server it plays from. */
t("mpv_queue_stays_on_its_server_after_a_switch", async () => {
  const start = async (name: string, machine: string, seed: string) => {
    const media = join(temp, name);
    mkdirSync(media, { recursive: true });
    sine(join(media, "movie.mkv"), 40);
    const mock = await startMockPms({ media, dir: join(temp, `${name}-log`), args: ["--name", name, "--machine", machine, "--seed", seed] });
    mocks.push(mock);
    return mock;
  };
  const alphaMock = await start("Alpha", "alpha-queue", "0");
  const betaMock = await start("Beta", "beta-queue", "1");
  const app = newApp();
  const alpha = session(alphaMock.url, "alpha-queue", "Alpha");
  const beta = session(betaMock.url, "beta-queue", "Beta");
  app.setSession(alpha, []);
  const first = (await alpha.server.metadata("100", new MetadataOptions()))!;
  const more = (await alpha.server.metadata("101", new MetadataOptions()))!;
  player.play(app, player.playRequest({ item: first, resume: false }));
  await waitFor("playback to start", 15_000, () => (app.nowPlaying()?.positionMs ?? 0) > 500);

  // Beta is now the connected server; its "101" has no tag, like Alpha's untagged items did.
  app.setSession(beta, []);
  const other = (await beta.server.metadata("101", new MetadataOptions()))!;
  expect(M.serverId(other)).toBeNull();
  expect(other.title).not.toBe(more.title);
  player.queue(app, { kind: "add", item: other, next: false });
  // Alpha's own item, found from Beta, carries Alpha's tag and is still welcome. Commands are
  // handled in order, so once it is queued Beta's has been decided too.
  servers.tag(app, [more], "alpha-queue");
  const title = more.title;
  player.queue(app, { kind: "add", item: more, next: false });
  await waitFor("Alpha's item to be queued", 5000, () => app.snapshot().queue?.items.at(-1)?.item.title === title);
  const titles = app.snapshot().queue!.items.map((e) => e.item.title);
  expect(titles.length).toBe(2);

  player.control({ kind: "stop" });
  await waitFor("playback to end", 6000, () => app.nowPlaying() === null);
}, 120_000);

/** A transcode plays (and burns in) the server's selected tracks, so the automatic choice is saved before the transcoder is asked. */
t("mpv_transcode_gets_the_automatic_track_choice", async () => {
  const media = join(temp, "media");
  mkdirSync(media, { recursive: true });
  writeFileSync(join(media, "movie.mkv"), new Uint8Array(4096).fill(7));
  const mock = await startMockPms({ media, dir: temp });
  mocks.push(mock);
  const app = newApp();
  app.setSession(session(mock.url), []);
  prefs.save(app, null, null, { audioLanguages: ["eng"], subtitles: "off" });
  const item = (await app.server().metadata("101", new MetadataOptions()))!;
  player.play(app, player.playRequest({ item, resume: false, quality: "20m-1080" }));
  // The mock has no transcoder, so mpv gives up; what matters is what was saved first.
  await waitFor("playback to start", 10_000, player.isActive);
  await waitFor("playback to end", 30_000, () => !player.isActive());
  await Bun.sleep(500);
  const lines = mock.events().split("\n");
  const saved = lines.findIndex((l) => l.includes('"what": "select_streams"') && l.includes('"audioStreamID": "1012"') && l.includes('"subtitleStreamID": "0"'));
  const transcode = lines.findIndex((l) => l.includes("/video/:/transcode/universal/start.m3u8"));
  expect(transcode).toBeGreaterThanOrEqual(0);
  expect(saved).toBeGreaterThanOrEqual(0);
  expect(saved).toBeLessThan(transcode);
}, 90_000);

/** A file mpv cannot play is reported with a reason and can be retried; controls, diagnostics and the saved queue work against a real mpv. */
t("mpv_failure_controls_and_resume", async () => {
  const media = join(temp, "media");
  mkdirSync(media, { recursive: true });
  // Not a media file: the first attempt fails, and the worker swaps in a real one before retrying.
  writeFileSync(join(media, "movie.mkv"), new Uint8Array(4096).fill(7));
  const mock = await startMockPms({ media, dir: temp });
  mocks.push(mock);
  const rec = new Recorder();
  const app = newApp(rec);
  app.setSession(session(mock.url), []);
  const item = (await app.server().metadata("100", new MetadataOptions()))!;

  // A file mpv cannot open: the failure arrives with a reason, before anything loaded.
  player.play(app, player.playRequest({ item, resume: false }));
  await waitFor("the failure", 20_000, () => failure.current() !== null);
  const f = failure.current()!;
  expect(f.phase).toBe("start");
  expect(f.reason.length > 0 && f.canRetry).toBe(true);
  expect(f.positionMs).toBe(0);
  expect(f.lowerQuality?.id).toBe("20m-1080");
  const ev = rec.last("playbackFailure") as unknown as { payload?: { item: { ratingKey: string } } } | undefined;
  expect(ev).toBeDefined();
  await waitFor("playback to end", 10_000, () => app.nowPlaying() === null && !player.isActive());

  // Fix the file and retry: playback starts and the failure is gone.
  sine(join(media, "movie.mkv"), 300);
  failure.recover(app, { kind: "retry" });
  await waitFor("playback after retry", 15_000, () => (app.nowPlaying()?.positionMs ?? 0) > 1000);
  expect(failure.current()).toBeNull();

  // Controls and diagnostics.
  player.tune(app, { kind: "volume", value: 40 });
  player.tune(app, { kind: "speed", value: 1.5 });
  player.tune(app, { kind: "repeat", mode: "all" });
  player.tune(app, { kind: "sleepMinutes", minutes: 30 });
  await waitFor("the controls to apply", 5000, () => {
    const s = controls.playbackState();
    return !!s && s.volume === 40 && s.speed === 1.5 && s.repeat === "all" && s.sleep !== null;
  });
  const state = controls.playbackState()!;
  expect(state.sleep).toMatchObject({ mode: "minutes" });
  expect(state.diagnostics.connection!.kind).toBe("local");
  expect(typeof state.diagnostics.bufferedSecs).toBe("number");
  player.tune(app, { kind: "sleepEndOfItem" });
  await waitFor("end-of-item sleep", 5000, () => !!app.nowPlaying()?.stopAfter);
  player.tune(app, { kind: "sleepOff" });
  await waitFor("sleep off", 5000, () => app.nowPlaying() !== null && !app.nowPlaying()!.stopAfter);
  player.tune(app, { kind: "mute", on: true });
  await waitFor("mute", 5000, () => !!controls.playbackState()?.muted);
  player.tune(app, { kind: "repeat", mode: "off" });

  // The queue is saved while playing, and offered again after stopping.
  await waitFor("a saved queue", 15_000, () => (playback.savedQueue(app)?.positionMs ?? 0) > 1000);
  player.control({ kind: "stop" });
  await waitFor("playback to end", 8000, () => app.nowPlaying() === null && !player.isActive());
  const savedQueue = playback.savedQueue(app);
  expect(savedQueue).not.toBeNull();
  expect(savedQueue!.current.ratingKey).toBe("100");

  playback.resumeSaved(app);
  await waitFor("the resumed playback", 15_000, () => (app.nowPlaying()?.positionMs ?? 0) >= savedQueue!.positionMs - 1000);
  expect(app.nowPlaying()!.positionMs).toBeLessThan(savedQueue!.positionMs + 15_000);

  // Stopping and forgetting clears the offer; a finished queue is not offered.
  player.control({ kind: "stop" });
  await waitFor("playback to end", 8000, () => app.nowPlaying() === null && !player.isActive());
  playback.forgetSaved(app);
  expect(playback.savedQueue(app)).toBeNull();
  await report.flush(10_000);
}, 180_000);
