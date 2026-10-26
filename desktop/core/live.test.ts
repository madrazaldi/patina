import { expect, test } from "bun:test";

import { entries, text } from "./live";
import { Changes } from "./state";

test("changes merge sections and items", () => {
  const c = new Changes();
  c.add("1", "10");
  c.add("1", "11");
  c.add("2", null);
  expect(c.all).toBe(false);
  expect(c.sections).toEqual(["1", "2"]);
  expect(c.items).toEqual(["10", "11"]);
  c.add(null, "12");
  expect(c.all).toBe(true); // a change without a section could be anywhere
});

test("text reads strings and numbers", () => {
  const v = { a: "1", b: 2, c: "", d: null };
  expect(text(v, "a")).toBe("1");
  expect(text(v, "b")).toBe("2");
  expect(text(v, "c")).toBeNull();
  expect(text(v, "d")).toBeNull();
  expect(text(v, "e")).toBeNull();
});

test("entries accept arrays, single objects and a NotificationContainer wrapper", () => {
  expect(entries({ TimelineEntry: [{ a: 1 }, { a: 2 }] }, "TimelineEntry").length).toBe(2);
  expect(entries({ NotificationContainer: { TimelineEntry: { a: 1 } } }, "TimelineEntry")).toEqual([{ a: 1 }]);
  expect(entries({ other: 1 }, "TimelineEntry")).toEqual([]);
  expect(entries(null, "x")).toEqual([]);
});
