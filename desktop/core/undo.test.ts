import { describe, expect, test } from "bun:test";

import { defaultMetadata, type Metadata } from "../plex";
import { callsFor, changing, DEPTH, locate, place, type Slot, Stack, watchLabel, type WatchState } from "./undo";

function item(rk: string, views: number, offset: number): Metadata {
  return defaultMetadata({ ratingKey: rk, title: rk, viewCount: views, viewOffset: offset });
}

function push(stack: Stack, scope: string, label: string): number {
  return stack.push(scope, label, { kind: "progress", server: null, ratingKey: "1", offset: 5 });
}

describe("undo", () => {
  test("undo_takes_the_newest_entry_first", () => {
    const s = new Stack();
    const a = push(s, "x", "a");
    const b = push(s, "x", "b");
    expect(s.last("x")?.id).toBe(b);
    expect(s.take("x", null)?.id).toBe(b);
    expect(s.take("x", a)?.label).toBe("a");
    expect(s.take("x", null)).toBeNull();
  });

  test("a_specific_entry_can_be_taken_out_of_order", () => {
    const s = new Stack();
    const a = push(s, "x", "a");
    const b = push(s, "x", "b");
    expect(s.take("x", a)?.id).toBe(a);
    expect(s.take("x", a)).toBeNull(); // already undone
    expect(s.length).toBe(1);
    expect(s.last("x")?.id).toBe(b);
  });

  test("entries_never_cross_account_or_server_scopes", () => {
    const s = new Stack();
    const a = push(s, "alice:server1", "a");
    expect(s.last("alice:server2")).toBeNull();
    expect(s.take("bob:server1", a)).toBeNull();
    // Taking from another scope also forgot the old one.
    expect(s.isEmpty()).toBe(true);
    push(s, "alice:server1", "a");
    push(s, "alice:server2", "b");
    expect(s.length).toBe(1); // pushing in a new scope drops the old scope
    s.retainScope("alice:server1");
    expect(s.isEmpty()).toBe(true);
  });

  test("history_is_bounded", () => {
    const s = new Stack();
    for (let i = 0; i < DEPTH + 10; i++) push(s, "x", String(i));
    expect(s.length).toBe(DEPTH);
    expect(s.last("x")?.label).toBe(String(DEPTH + 9));
    expect(s.entries[0]!.label).toBe("10");
  });

  test("a_failed_undo_goes_back_on_the_stack", () => {
    const s = new Stack();
    const a = push(s, "x", "a");
    const e = s.take("x", null)!;
    s.restore(e);
    expect(s.last("x")?.id).toBe(a);
  });

  test("only_items_whose_state_changes_are_remembered", () => {
    const items = () => [item("unwatched", 0, 0), item("partial", 0, 30_000), item("watched", 2, 0)];
    const markedWatched = changing(items(), true);
    expect(markedWatched.map((s) => s.ratingKey)).toEqual(["unwatched", "partial"]);
    // The resume position of the partly watched one is kept so undo can put it back.
    expect(markedWatched[1]!.offset).toBe(30_000);
    const markedUnwatched = changing(items(), false);
    expect(markedUnwatched.map((s) => s.ratingKey)).toEqual(["partial", "watched"]);
    expect(changing([item("a", 0, 0)], false)).toEqual([]); // nothing changes, nothing to undo
  });

  test("restoring_replays_state_then_position", () => {
    const restore: WatchState[] = [
      { ratingKey: "1", watched: false, offset: 30_000 },
      { ratingKey: "2", watched: true, offset: 0 },
      { ratingKey: "3", watched: false, offset: 0 },
    ];
    expect(callsFor(restore)).toEqual([
      { kind: "unscrobble", ratingKey: "1" },
      { kind: "progress", ratingKey: "1", ms: 30_000 },
      { kind: "scrobble", ratingKey: "2" },
      { kind: "unscrobble", ratingKey: "3" },
    ]);
  });

  const slots = (ids: number[]): Slot[] => ids.map((i) => ({ entryId: i, ratingKey: `rk${i}`, title: `t${i}` }));

  test("locating_an_entry_finds_its_predecessor", () => {
    const s = slots([10, 20, 30]);
    const found = locate(s, 20);
    expect(found && [found[0].entryId, found[1]]).toEqual([20, 10]);
    expect(locate(s, 10)?.[1]).toBeNull(); // first entry has no predecessor
    expect(locate(s, 99)).toBeNull();
  });

  test("a_restored_entry_goes_back_after_its_predecessor", () => {
    // The restored entry (40) is last; it used to follow 10.
    const now = slots([10, 30, 40]);
    expect(place(now, 10)).toEqual({ after: 10 });
    expect(place(now, null)).toEqual({ after: null }); // it was first
    expect(place(now, 99)).toBeNull(); // the predecessor is gone, so it stays at the end
    // The entry itself is not its own predecessor.
    expect(place(slots([40]), 40)).toBeNull();
  });

  test("labels_name_a_single_item_and_count_many", () => {
    const one = [item("Heat", 0, 0)];
    expect(watchLabel(one, 1, true)).toBe("Marked Heat watched");
    expect(watchLabel([], 1, false)).toBe("Marked as unwatched");
    expect(watchLabel(one, 7, true)).toBe("Marked 7 items watched");
  });
});
