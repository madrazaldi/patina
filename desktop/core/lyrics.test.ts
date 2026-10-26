import { expect, test } from "bun:test";

import { parse } from "./lyrics";

test("parses lrc", () => {
  const l = parse("[ti:Song]\n[00:01.50]One\n[00:03.00][00:10.00]Two\n");
  expect(l.synced).toBe(true);
  expect(l.lines.map((l) => l.timeMs)).toEqual([1500, 3000, 10000]);
  expect(l.lines[2]!.text).toBe("Two");
});

test("keeps plain text", () => {
  const l = parse("First line\n\nSecond\n\n");
  expect(l.synced).toBe(false);
  expect(l.lines.length).toBe(3);
});
