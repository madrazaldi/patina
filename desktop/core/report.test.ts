import { expect, test } from "bun:test";
import type { TimelineState } from "../plex";
import { type Config, type Op, Pending, Reporter, type Sink, isProgress } from "./report";

function update(key: string, state: TimelineState, timeMs: number): Op {
  return {
    kind: "timeline",
    update: { ratingKey: key, key: `/library/metadata/${key}`, state, timeMs, durationMs: 100_000, playQueueItemId: null, session: null },
  };
}

function times(p: Pending<string>): [string, number, string][] {
  return p.ops().flatMap((o): [string, number, string][] => (o.kind === "timeline" ? [[o.update.ratingKey, o.update.timeMs, o.update.state]] : []));
}

test("progress_collapses_to_the_newest_checkpoint", () => {
  const p = new Pending<string>();
  for (let t = 0; t < 50; t++) p.push("s", update("1", "playing", t * 1000));
  p.push("s", update("1", "paused", 49_500));
  expect(times(p)).toEqual([["1", 49_500, "paused"]]);
});

test("different_items_and_servers_do_not_merge", () => {
  const p = new Pending<string>();
  p.push("a", update("1", "playing", 1));
  p.push("a", update("2", "playing", 2));
  p.push("b", update("1", "playing", 3));
  expect(p.length).toBe(3);
});

test("the_final_stop_replaces_progress_and_is_kept", () => {
  const p = new Pending<string>();
  p.push("s", update("1", "playing", 10_000));
  p.push("s", update("1", "stopped", 90_000));
  p.push("s", { kind: "scrobble", ratingKey: "1" });
  expect(times(p)).toEqual([["1", 90_000, "stopped"]]);
  expect(p.ops()[1]!.kind).toBe("scrobble");
});

test("progress_of_a_new_playback_never_merges_into_an_earlier_stop", () => {
  const p = new Pending<string>();
  p.push("s", update("1", "stopped", 90_000));
  p.push("s", update("1", "playing", 0));
  p.push("s", update("1", "playing", 5_000));
  expect(times(p)).toEqual([
    ["1", 90_000, "stopped"],
    ["1", 5_000, "playing"],
  ]);
});

test("stream_choices_merge_per_part", () => {
  const p = new Pending<string>();
  p.push("s", { kind: "streams", partId: 7, audio: 1, subtitle: null });
  p.push("s", { kind: "streams", partId: 7, audio: null, subtitle: 5 });
  p.push("s", { kind: "streams", partId: 7, audio: 2, subtitle: null });
  p.push("s", { kind: "streams", partId: 8, audio: 9, subtitle: null });
  expect(p.ops()).toEqual([
    { kind: "streams", partId: 7, audio: 2, subtitle: 5 },
    { kind: "streams", partId: 8, audio: 9, subtitle: null },
  ]);
});

test("transcode_pings_are_deduplicated_and_dropped_when_stopping", () => {
  const p = new Pending<string>();
  for (let i = 0; i < 5; i++) p.push("s", { kind: "pingTranscode", session: "x" });
  expect(p.length).toBe(1);
  p.push("s", { kind: "stopTranscode", session: "x" });
  expect(p.ops()).toEqual([{ kind: "stopTranscode", session: "x" }]);
  p.push("s", { kind: "pingTranscode", session: "x" });
  expect(p.length).toBe(1);
});

/** Records what reaches "Plex"; each request takes `delay`, and the first `fail` of them fail. */
class Fake implements Sink<string> {
  seen: Op[] = [];
  hang = false;
  constructor(
    private readonly delay: number,
    private fail: number,
  ) {}
  async send(_target: string, op: Op): Promise<void> {
    if (this.hang) await new Promise<never>(() => {});
    await Bun.sleep(this.delay);
    if (this.fail > 0) {
      this.fail -= 1;
      throw new Error("boom");
    }
    this.seen.push(op);
  }
}

const fast = (): Config => ({ timeoutMs: 200, attempts: 4, backoffMs: 10 });

test("a_slow_server_costs_the_caller_nothing_and_collapses_the_backlog", async () => {
  const fake = new Fake(150, 0);
  const reporter = Reporter.spawn(fake, fast());
  reporter.send("s", update("1", "playing", 0));
  // The first request is now in flight and takes 150ms.
  await Bun.sleep(20);
  const began = performance.now();
  for (let t = 1; t < 200; t++) reporter.send("s", update("1", "playing", t * 1000));
  reporter.send("s", update("1", "stopped", 200_000));
  reporter.send("s", { kind: "scrobble", ratingKey: "1" });
  expect(performance.now() - began).toBeLessThan(50);
  expect(await reporter.flush(5000)).toBe(true);
  const seen = fake.seen;
  expect(seen.length).toBeLessThanOrEqual(4);
  const stops = seen.filter((o) => o.kind === "timeline" && o.update.state === "stopped");
  expect(stops.length).toBe(1);
  const beforeLast = seen[seen.length - 2]!;
  expect(beforeLast.kind === "timeline" && beforeLast.update.state === "stopped" && beforeLast.update.timeMs === 200_000).toBe(true);
  expect(seen[seen.length - 1]!.kind).toBe("scrobble");
  // Nothing older than the stop was sent after it.
  const stopAt = seen.findIndex((o) => o.kind === "timeline" && o.update.state === "stopped");
  expect(seen.slice(stopAt).every((o) => !isProgress(o))).toBe(true);
});

test("the_final_stop_survives_failures", async () => {
  const fake = new Fake(1, 2);
  const reporter = Reporter.spawn(fake, fast());
  reporter.send("s", update("1", "stopped", 42_000));
  expect(await reporter.flush(5000)).toBe(true);
  expect(fake.seen.length).toBe(1);
  const o = fake.seen[0]!;
  expect(o.kind === "timeline" && o.update.timeMs === 42_000 && o.update.state === "stopped").toBe(true);
});

test("failed_progress_is_not_retried_and_not_resent_over_newer_state", async () => {
  const fake = new Fake(1, 1);
  const reporter = Reporter.spawn(fake, fast());
  reporter.send("s", update("1", "playing", 1_000));
  await Bun.sleep(50);
  reporter.send("s", update("1", "playing", 2_000));
  expect(await reporter.flush(5000)).toBe(true);
  expect(fake.seen.length).toBe(1);
  const o = fake.seen[0]!;
  expect(o.kind === "timeline" && o.update.timeMs === 2_000).toBe(true);
});

test("a_hanging_server_is_cut_off_so_later_reports_get_through", async () => {
  const hang = new Fake(0, 0);
  hang.hang = true;
  const reporter = Reporter.spawn(hang, { timeoutMs: 30, attempts: 2, backoffMs: 5 });
  reporter.send("s", update("1", "stopped", 1));
  reporter.send("s", update("2", "stopped", 2));
  const began = performance.now();
  expect(await reporter.flush(5000)).toBe(true);
  expect(performance.now() - began).toBeLessThan(2000);
});

test("flush_times_out_while_the_server_is_slow", async () => {
  const fake = new Fake(300, 0);
  const reporter = Reporter.spawn(fake, { ...fast(), timeoutMs: 5000 });
  reporter.send("s", update("1", "stopped", 1));
  expect(await reporter.flush(20)).toBe(false);
});
