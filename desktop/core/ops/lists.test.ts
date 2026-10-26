import { describe, expect, test } from "bun:test";

import { collectPages } from "./lists";

type Page = [number[], number | null];

/** A fake paged source of `n` numbers that serves at most `cap` per request. */
function source(n: number, cap: number, reportTotal: boolean) {
  return async (start: number, size: number): Promise<Page> => {
    const take = Math.min(size, cap);
    const page: number[] = [];
    for (let i = start; i < Math.min(n, start + take); i++) page.push(i);
    return [page, reportTotal ? n : null];
  };
}

describe("collect_pages", () => {
  test("collects_every_page", async () => {
    const [items, total] = await collectPages(1000, 100_000, source(2500, 1000, true));
    expect([items.length, total]).toEqual([2500, 2500]);
    expect(items.every((v, i) => i === v)).toBe(true);
  });

  test("exact_multiples_and_empty", async () => {
    let [items, total] = await collectPages(1000, 100_000, source(2000, 1000, true));
    expect([items.length, total]).toEqual([2000, 2000]);
    [items, total] = await collectPages(1000, 100_000, source(2000, 1000, false));
    expect([items.length, total]).toEqual([2000, 2000]);
    [items, total] = await collectPages(1000, 100_000, source(0, 1000, true));
    expect([items.length, total]).toEqual([0, 0]);
  });

  test("server_page_cap_below_requested_size", async () => {
    // The server hands out 300 at a time even though 1000 were requested.
    const [items, total] = await collectPages(1000, 100_000, source(1000, 300, true));
    expect([items.length, total]).toEqual([1000, 1000]);
  });

  test("limit_truncates_but_keeps_the_true_total", async () => {
    const [items, total] = await collectPages(1000, 2500, source(10_000, 1000, true));
    expect(items.length).toBe(2500);
    expect(total).toBe(10_000);
  });

  test("errors_bubble_up", async () => {
    const r = collectPages<number>(10, 100, async (start): Promise<Page> => {
      if (start === 0) return [Array.from({ length: 10 }, (_, i) => i), 30];
      throw new Error("boom");
    });
    await expect(r).rejects.toThrow();
  });
});
