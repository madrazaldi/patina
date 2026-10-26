import { describe, expect, test } from "bun:test";

import { acknowledge, belongs } from "./offline";
import type { Progress } from "./offline";

function progress(): Progress {
  return {
    id: "one",
    account: "alice",
    server: "home",
    rating_key: "1",
    position: 10,
    duration: 100,
    watched: false,
    updated: 1,
    revision: "old",
    pending: true,
  };
}

describe("offline", () => {
  test("never replays to a different user or server", () => {
    const p = progress();
    expect(belongs(p, "home", "alice")).toBe(true);
    expect(belongs(p, "home", "bob")).toBe(false);
    expect(belongs(p, "away", "alice")).toBe(false);
  });

  test("acknowledging a request does not erase newer progress", () => {
    const sent = progress();
    const list = [{ ...sent }];
    list[0]!.revision = "new";
    acknowledge(list, sent);
    expect(list[0]!.pending).toBe(true);
    const newest = { ...list[0]! };
    acknowledge(list, newest);
    expect(list[0]!.pending).toBe(false);
  });
});
