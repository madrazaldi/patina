import { expect, test } from "bun:test";
import { defaultChapter } from "../plex";
import { type ConnectionInfo, Tracker, isLocalHost, nextRepeat, offlineConnection } from "./controls";

test("recognizes_local_addresses", () => {
  for (const host of [
    "localhost",
    "127.0.0.1",
    "192.168.1.20",
    "10.0.0.2",
    "172.16.5.5",
    "nas.local",
    "nas",
    "[::1]",
    "192-168-1-20.abc123.plex.direct",
    "fd12::1",
  ]) {
    expect(isLocalHost(host), host).toBe(true);
  }
  for (const host of ["8.8.8.8", "plex.example.com", "203-0-113-9.abc123.plex.direct", "2001:db8::1"]) {
    expect(isLocalHost(host), host).toBe(false);
  }
});

test("repeat_cycles_through_every_mode", () => {
  expect(nextRepeat("off")).toBe("all");
  expect(nextRepeat("all")).toBe("one");
  expect(nextRepeat("one")).toBe("off");
});

test("follows_mpv_properties_and_flags_urgent_ones", () => {
  const t = new Tracker({ kind: "local", address: null } satisfies ConnectionInfo);
  expect(t.onProperty("volume", 80.0)).toBe(true);
  expect(t.onProperty("mute", true)).toBe(true);
  expect(t.onProperty("demuxer-cache-duration", 12.34)).toBe(false);
  expect(t.onProperty("frame-drop-count", 3)).toBe(false);
  expect(t.onProperty("paused-for-cache", true)).toBe(true);
  expect(t.onProperty("hwdec-current", "videotoolbox")).toBe(true);
  expect(t.onProperty("time-pos", 1.0)).toBe(false);
  const v = t.view();
  expect([v.volume, v.muted]).toEqual([80.0, true]);
  expect(v.diagnostics.bufferedSecs).toBe(12.3);
  expect(v.diagnostics.droppedFrames).toBe(3);
  expect(v.diagnostics.hwdec).toBe("videotoolbox");
  expect(v.diagnostics.buffering).toBe(true);
  t.newItem([]);
  expect(t.view().diagnostics.droppedFrames).toBe(0);
  expect(t.view().diagnostics.connection?.kind).toBe("local");
});

test("chapters_come_from_mpv_or_else_plex", () => {
  const t = new Tracker(offlineConnection());
  const plex = [defaultChapter({ tag: "Opening", startTimeOffset: 0 }), defaultChapter({ tag: null, startTimeOffset: 60_000 })];
  t.newItem(plex);
  t.onTime(70_000);
  let v = t.view();
  expect(v.chapters.map((c) => c.title)).toEqual(["Opening", "Chapter 2"]);
  expect(v.chapter).toBe(1);
  t.onProperty("chapter-list", [
    { title: "A", time: 0.0 },
    { title: "", time: 30.5 },
    { title: "C", time: 90.0 },
  ]);
  t.onProperty("chapter", 2);
  v = t.view();
  expect(v.chapters.length).toBe(3);
  expect(v.chapters[1]).toEqual({ title: "Chapter 2", timeMs: 30_500 });
  expect(v.chapter).toBe(2);
});
