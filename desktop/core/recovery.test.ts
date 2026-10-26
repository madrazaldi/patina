import { expect, test } from "bun:test";
import { defaultMetadata } from "../plex";
import { MAX_QUEUE_ITEMS, RECOVERY_WINDOW, slim, usable, window } from "./recovery";
import type { Checkpoint } from "./recovery";
import { Metadata } from "../plex";

function cp(position: number, duration: number, updated: number): Checkpoint {
  return { account: "a", server: "s", rating_key: "1", position, duration, updated };
}

test("a_checkpoint_is_used_until_the_server_has_seen_the_item_later", () => {
  const c = cp(120_000, 600_000, 1000);
  expect(usable(c, null, 1100)).toBe(120_000);
  expect(usable(c, 900, 1100)).toBe(120_000); // the server last saw it before the checkpoint
  expect(usable(c, 1000, 1100)).toBe(120_000);
  expect(usable(c, 1001, 1100)).toBeNull(); // watched since, maybe elsewhere
});

test("finished_or_stale_checkpoints_are_ignored", () => {
  expect(usable(cp(0, 600_000, 1000), null, 1100)).toBeNull();
  expect(usable(cp(590_000, 600_000, 1000), null, 1100)).toBeNull();
  expect(usable(cp(5000, 600_000, 1000), null, 1000 + RECOVERY_WINDOW + 1)).toBeNull();
});

test("long_queues_keep_the_playing_item_in_the_window", () => {
  expect(window(10, 4)).toEqual([{ start: 0, end: 10 }, 4]);
  let [range, current] = window(1000, 500);
  expect(range.end - range.start).toBe(MAX_QUEUE_ITEMS);
  expect(range.start + current).toBe(500);
  [range, current] = window(1000, 999);
  expect(range.end).toBe(1000);
  expect(range.start + current).toBe(999);
  expect(window(1000, 0)[0].start).toBe(0);
});

test("slim_items_keep_what_is_needed_to_list_and_requeue", () => {
  const full = defaultMetadata({
    ratingKey: "5",
    key: "/library/metadata/5",
    type: "episode",
    title: "Pilot",
    grandparentTitle: "Show",
    summary: "long",
    index: 1,
    parentIndex: 1,
  });
  const s = slim(full);
  expect(Metadata.fullTitle(s)).toBe(Metadata.fullTitle(full));
  expect(s.summary).toBeNull();
  expect(Metadata.isPlayable(s)).toBe(true);
});
