import { expect, test } from "bun:test";

import { Metadata, defaultMedia, defaultMetadata, defaultPart } from "../plex";
import type { Download } from "./downloads";
import { Scope } from "./recovery";
import { type Candidate, type Fetched, type Held, type Plan, type Policy, candidates, emptyPlan, evaluate, inScope, split, watchOrder } from "./smart";

function policy(id: string, keep: number, deleteWatched: boolean): Policy {
  return {
    id,
    account: "alice",
    serverId: "home",
    serverName: "Home",
    ratingKey: `show-${id}`,
    target: "show",
    title: id,
    keep,
    deleteWatched,
    enabled: true,
  };
}

function episodes(prefix: string, watched: number, total: number, size: number): Candidate[] {
  return Array.from({ length: total }, (_, i) => i + 1).map((n) => ({
    ratingKey: `${prefix}${n}`,
    id: `home:alice:${prefix}${n}`,
    watched: n <= watched,
    size,
  }));
}

function held(prefix: string, numbers: number[], owner: string | null, size: number): Held[] {
  return numbers.map((n) => ({ id: `home:alice:${prefix}${n}`, policy: owner, size }));
}

const known = (ids: string[]): Set<string> => new Set(ids);

const added = (plan: Plan): string[] => plan.add.map(([, k]) => k);

test("keeps_the_next_unwatched_episodes", () => {
  const p = policy("a", 3, true);
  const fetched: Fetched[] = [{ policy: p, candidates: episodes("e", 1, 6, 100) }];
  let plan = evaluate(fetched, [], known(["a"]), 0, null);
  expect(added(plan)).toEqual(["e2", "e3", "e4"]);
  expect(plan.remove).toEqual([]);
  // Already downloaded episodes are not fetched again.
  plan = evaluate(fetched, held("e", [2, 3], "a", 100), known(["a"]), 0, null);
  expect(added(plan)).toEqual(["e4"]);
});

test("removes_watched_downloads_and_fetches_the_next_ones", () => {
  const p = policy("a", 3, true);
  // e2 was watched since the last round: it goes, the window moves on to e5.
  const fetched: Fetched[] = [{ policy: p, candidates: episodes("e", 2, 6, 100) }];
  const plan = evaluate(fetched, held("e", [2, 3, 4], "a", 100), known(["a"]), 0, null);
  expect(plan.remove).toEqual(["home:alice:e2"]);
  expect(added(plan)).toEqual(["e5"]);
});

test("keeps_watched_downloads_unless_the_policy_says_otherwise", () => {
  const p = policy("a", 2, false);
  const fetched: Fetched[] = [{ policy: p, candidates: episodes("e", 2, 6, 100) }];
  const plan = evaluate(fetched, held("e", [1, 2, 3], "a", 100), known(["a"]), 0, null);
  expect(plan.remove).toEqual([]);
  expect(added(plan)).toEqual(["e4"]);
});

test("downloads_that_fell_out_of_the_window_are_removed", () => {
  // Keep was lowered from 4 to 2: the two furthest episodes go.
  const p = policy("a", 2, false);
  const fetched: Fetched[] = [{ policy: p, candidates: episodes("e", 0, 6, 100) }];
  const plan = evaluate(fetched, held("e", [1, 2, 3, 4], "a", 100), known(["a"]), 0, null);
  expect(plan.remove).toEqual(["home:alice:e3", "home:alice:e4"]);
  expect(plan.add).toEqual([]);
});

test("manual_downloads_are_never_removed", () => {
  const p = policy("a", 2, true);
  // e1 was watched and downloaded by hand: it stays. It does not count towards the window.
  const fetched: Fetched[] = [{ policy: p, candidates: episodes("e", 1, 5, 100) }];
  const plan = evaluate(fetched, held("e", [1], null, 100), known(["a"]), 0, null);
  expect(plan.remove).toEqual([]);
  expect(added(plan)).toEqual(["e2", "e3"]);
});

test("a_policy_that_could_not_be_fetched_removes_nothing", () => {
  // No Fetched entry for "a" (the server was unreachable, or it is disabled).
  const plan = evaluate([], held("e", [1, 2], "a", 100), known(["a"]), 0, null);
  expect(plan).toEqual(emptyPlan());
});

test("the_budget_limits_what_is_added", () => {
  const p = policy("a", 4, true);
  const fetched: Fetched[] = [{ policy: p, candidates: episodes("e", 0, 6, 100) }];
  // 250 bytes: two fit, and the rest wait in order (no skipping ahead).
  let plan = evaluate(fetched, [], known(["a"]), 0, 250);
  expect(added(plan)).toEqual(["e1", "e2"]);
  expect(plan.overBudget).toBe(2);
  // Other servers' downloads count against the budget too.
  plan = evaluate(fetched, [], known(["a"]), 150, 250);
  expect(added(plan)).toEqual(["e1"]);
  // Existing downloads count; nothing is removed to make room.
  plan = evaluate(fetched, held("e", [1, 2], "a", 100), known(["a"]), 0, 250);
  expect(added(plan)).toEqual([]);
  expect(plan.overBudget).toBe(2);
  expect(plan.remove).toEqual([]);
});

test("removing_watched_files_frees_budget_for_the_next_ones", () => {
  const p = policy("a", 2, true);
  const fetched: Fetched[] = [{ policy: p, candidates: episodes("e", 1, 5, 100) }];
  // e1 (watched) is removed, which leaves room for e3 within 200 bytes.
  const plan = evaluate(fetched, held("e", [1, 2], "a", 100), known(["a"]), 0, 200);
  expect(plan.remove).toEqual(["home:alice:e1"]);
  expect(added(plan)).toEqual(["e3"]);
  expect(plan.overBudget).toBe(0);
});

test("two_policies_share_the_budget_and_do_not_fight_over_files", () => {
  const a = policy("a", 2, true);
  const b = policy("b", 2, true);
  const fetched: Fetched[] = [
    { policy: a, candidates: episodes("x", 0, 3, 100) },
    // The second policy lists the same episode x1 plus its own.
    { policy: b, candidates: [...episodes("x", 0, 1, 100), ...episodes("y", 0, 3, 100)] },
  ];
  let plan = evaluate(fetched, [], known(["a", "b"]), 0, null);
  expect(added(plan)).toEqual(["x1", "x2", "y1"]);
  // A file wanted by b stays even though a no longer wants it.
  const aOnly: Fetched[] = [{ policy: a, candidates: episodes("x", 1, 3, 100) }, fetched[1]!];
  plan = evaluate(aOnly, held("x", [1], "a", 100), known(["a", "b"]), 0, null);
  expect(plan.remove).toEqual([]);
});

test("downloads_of_a_deleted_policy_are_adopted_or_released", () => {
  const b = policy("b", 2, true);
  const fetched: Fetched[] = [{ policy: b, candidates: episodes("e", 0, 4, 100) }];
  // "gone" no longer exists. e1 is wanted by b and adopted; e4 is nobody's and released.
  const files = [...held("e", [1], "gone", 100), ...held("e", [4], "gone", 100)];
  const plan = evaluate(fetched, files, known(["b"]), 0, null);
  expect(plan.assign).toEqual([
    ["home:alice:e1", "b"],
    ["home:alice:e4", null],
  ]);
  expect(plan.remove).toEqual([]);
});

test("only_this_account_and_server_are_considered", () => {
  const scope = new Scope("alice", "home");
  const mk = (server: string, account: string, size: number): Download => ({
    id: `${server}:${account}`,
    serverId: server,
    serverName: "",
    item: defaultMetadata(),
    file: null,
    poster: null,
    size,
    received: 0,
    state: "done",
    error: null,
    addedAt: 0,
    account,
    validator: null,
    sourceKey: null,
    policy: "a",
  });
  const all = [mk("home", "alice", 10), mk("home", "bob", 20), mk("away", "alice", 40)];
  const [heldList, outside] = split(all, scope);
  expect(heldList.map((h) => h.id)).toEqual(["home:alice"]);
  expect(outside).toBe(60);

  const other = policy("a", 1, true);
  expect(inScope(other, scope)).toBe(true);
  other.serverId = "away";
  expect(inScope(other, scope)).toBe(false);
  other.serverId = "home";
  other.account = "bob";
  expect(inScope(other, scope)).toBe(false);
});

function episode(season: number, number: number, key: string): Metadata {
  return defaultMetadata({ ratingKey: key, parentIndex: season, index: number });
}

test("episodes_are_watched_in_order_without_specials", () => {
  const items = [episode(2, 1, "c"), episode(0, 1, "special"), episode(1, 2, "b"), episode(1, 1, "a")];
  const keys = (v: Metadata[]) => v.map((m) => Metadata.ratingKey(m));
  expect(keys(watchOrder("show", [...items]))).toEqual(["a", "b", "c"]);
  // Asking for the specials season itself keeps them.
  const specials = [episode(0, 2, "s2"), episode(0, 1, "s1")];
  expect(keys(watchOrder("season", [...specials]))).toEqual(["s1", "s2"]);
  // A playlist keeps its own order.
  expect(keys(watchOrder("playlist", [...specials]))).toEqual(["s2", "s1"]);
});

test("candidates_follow_server_and_offline_watches", () => {
  const scope = new Scope("alice", "home");
  const media = () => [defaultMedia({ Part: [defaultPart({ size: 500 })] })];
  const seen = episode(1, 1, "e1");
  seen.type = "episode";
  seen.Media = media();
  seen.viewCount = 1;
  const offline = episode(1, 2, "e2");
  offline.type = "episode";
  offline.Media = media();
  const fresh = episode(1, 3, "e3");
  fresh.type = "episode";
  fresh.Media = media();
  const noFile = episode(1, 4, "e4");
  noFile.type = "episode";
  const watchedOffline = new Set(["home:alice:e2"]);
  const c = candidates([seen, offline, fresh, noFile], scope, watchedOffline);
  expect(c.map((c) => [c.ratingKey, c.watched, c.size])).toEqual([
    ["e1", true, 500],
    ["e2", true, 500],
    ["e3", false, 500],
  ]);
});
