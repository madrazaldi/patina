import { describe, expect, test } from "bun:test";

import { Params } from "../../plex";
import type { Directory, Metadata } from "../../plex";
import { choose, defaultPickQuery, filterId, matches, parseKind, type PickQuery, ratingOrder, Rng, serverFilters } from "./pick";
import { metadataFromJson } from "./util";

function query(over: Partial<PickQuery> = {}): PickQuery {
  return { ...defaultPickQuery(), ...over };
}

function movie(rk: string, minutes: number, year: number, genre: string, rating: string, audience: number, views: number): Metadata {
  return metadataFromJson({
    ratingKey: rk,
    type: "movie",
    title: `Movie ${rk}`,
    year,
    duration: minutes * 60_000,
    contentRating: rating,
    audienceRating: audience,
    viewCount: views,
    Genre: [{ tag: genre }],
  });
}

function show(rk: string, leaves: number, viewed: number): Metadata {
  return metadataFromJson({ ratingKey: rk, type: "show", title: "S", leafCount: leaves, viewedLeafCount: viewed, duration: 45 * 60_000 });
}

describe("pick", () => {
  test("unwatched_short_film_with_genre_and_rating", () => {
    const q = query({
      maxMinutes: 100,
      genres: ["comedy"],
      contentRatings: ["PG", "PG-13"],
      minRating: 7.0,
      yearMin: 2000,
      yearMax: 2020,
    });
    expect(matches(movie("1", 95, 2010, "Comedy", "PG", 7.5, 0), q)).toBe(true);
    expect(matches(movie("2", 101, 2010, "Comedy", "PG", 7.5, 0), q)).toBe(false); // too long
    expect(matches(movie("3", 95, 2010, "Drama", "PG", 7.5, 0), q)).toBe(false); // wrong genre
    expect(matches(movie("4", 95, 2010, "Comedy", "R", 7.5, 0), q)).toBe(false); // wrong rating
    expect(matches(movie("5", 95, 1999, "Comedy", "PG", 7.5, 0), q)).toBe(false); // too old
    expect(matches(movie("6", 95, 2021, "Comedy", "PG", 7.5, 0), q)).toBe(false); // too new
    expect(matches(movie("7", 95, 2010, "Comedy", "PG", 6.9, 0), q)).toBe(false); // rated too low
    expect(matches(movie("8", 95, 2010, "Comedy", "PG", 7.5, 2), q)).toBe(false); // already watched
  });

  test("filters_off_accept_everything_of_the_kind", () => {
    const q = query({ unwatched: false });
    expect(matches(movie("1", 300, 1950, "Drama", "R", 1.0, 5), q)).toBe(true);
    expect(matches(show("2", 10, 0), q)).toBe(false); // shows are not movies
    const any = { ...q, kind: "any" as const };
    expect(matches(show("2", 10, 0), any)).toBe(true);
  });

  test("a_show_with_episodes_left_counts_as_unwatched", () => {
    const q = query({ kind: "show" });
    expect(matches(show("1", 10, 0), q)).toBe(true);
    expect(matches(show("2", 10, 4), q)).toBe(true);
    expect(matches(show("3", 10, 10), q)).toBe(false);
    const short = query({ kind: "show", maxMinutes: 30 });
    expect(matches(show("4", 10, 0), short)).toBe(false); // 45 minute episodes
  });

  test("items_without_a_length_fail_a_length_filter", () => {
    const m = movie("1", 90, 2010, "Drama", "PG", 7.0, 0);
    m.duration = null;
    expect(matches(m, defaultPickQuery())).toBe(true);
    expect(matches(m, query({ maxMinutes: 120 }))).toBe(false);
  });

  test("choose_returns_the_requested_count_without_repeats", () => {
    const fits = Array.from({ length: 20 }, (_, i) => movie(String(i), 90, 2010, "Drama", "PG", 7.0, 0));
    const q = query({ count: 4 });
    const [items, matchCount, remaining, wrapped] = choose(fits, q, new Rng(7));
    expect([items.length, matchCount, remaining, wrapped]).toEqual([4, 20, 20, false]);
    const keys = new Set(items.map((m) => m.ratingKey));
    expect(keys.size).toBe(4);
  });

  test("reroll_skips_what_was_shown_then_starts_over", () => {
    const fits = () => Array.from({ length: 5 }, (_, i) => movie(String(i), 90, 2010, "Drama", "PG", 7.0, 0));
    const rng = new Rng(1);
    const q = query({ count: 3 });
    const [first] = choose(fits(), q, rng);
    q.exclude = first.map((m) => m.ratingKey ?? "");
    const [second, matchCount, remaining, wrapped] = choose(fits(), q, rng);
    expect([second.length, matchCount, remaining, wrapped]).toEqual([2, 5, 2, false]);
    expect(second.every((m) => !q.exclude.includes(m.ratingKey ?? ""))).toBe(true);
    q.exclude.push(...second.map((m) => m.ratingKey ?? ""));
    const [third, , , wrappedAgain] = choose(fits(), q, rng);
    expect(wrappedAgain).toBe(true); // everything was shown, so it starts over
    expect(third.length).toBe(3);
  });

  test("choose_handles_nothing_and_duplicates", () => {
    const q = defaultPickQuery();
    const [items, matchCount, remaining, wrapped] = choose([], q, new Rng());
    expect([items.length, matchCount, remaining, wrapped]).toEqual([0, 0, 0, false]);
    const same = movie("1", 90, 2010, "Drama", "PG", 7.0, 0);
    const [items2, matches2] = choose([same, { ...same }], q, new Rng());
    expect([items2.length, matches2]).toEqual([1, 1]);
  });

  test("server_filters_cover_every_criterion", () => {
    const q = query({
      maxMinutes: 100,
      contentRatings: ["PG", "R"],
      yearMin: 1990,
      yearMax: 1999,
      minRating: 6.5,
    });
    const p = serverFilters(q, ["3", "9"], true).toQueryString();
    for (const part of ["genre=3%2C9", "contentRating=PG%2CR", "year%3E%3E=1990", "year%3C%3C=1999", "audienceRating%3E%3E=6.5", "duration%3C%3C=6000000"]) {
      expect(p).toContain(part);
    }
    expect(serverFilters(q, [], false).toQueryString()).toBe("contentRating=PG%2CR");
    expect(serverFilters(q, [], false)).toBeInstanceOf(Params);
  });

  test("ratings_sort_from_gentle_to_strict", () => {
    const r = ["R", "Unrated", "G", "PG-13", "PG"];
    const key = (x: string) => ratingOrder(x);
    r.sort((a, b) => {
      const [ra, sa] = key(a);
      const [rb, sb] = key(b);
      return ra - rb || (sa < sb ? -1 : sa > sb ? 1 : 0);
    });
    expect(r).toEqual(["G", "PG", "PG-13", "R", "Unrated"]);
  });

  test("genre_filter_value_ids", () => {
    const plain = { key: "12" } as Directory;
    const full = { key: "/library/sections/1/all?genre=12" } as Directory;
    expect(filterId(plain)).toBe("12");
    expect(filterId(full)).toBe("12");
  });

  test("kind_names_parse", () => {
    expect(parseKind("Film")).toBe("movie");
    expect(parseKind("tv")).toBe("show");
    expect(parseKind("any")).toBe("any");
    expect(parseKind("music")).toBeNull();
  });
});
