import { expect, test } from "bun:test";

import { normalizeMetadata } from "../plex";
import { availability, capabilities, hasAny, isTmdbTitle, plexCopy, requestState, seasonsFrom, tmdbImage, titlesFrom } from "./seerr";

test("permissions follow Seerr: admins may do everything", () => {
  expect(hasAny(2, 16)).toBe(true);
  expect(hasAny(32, 16)).toBe(false);
  const user = capabilities(32, { movie: true, tv: false });
  expect(user).toEqual({ requestMovie: true, requestTv: true, request4kMovie: false, request4kTv: false, advanced: false, manage: false });
  const movies = capabilities(262144 | 2048, { movie: true, tv: true });
  expect([movies.requestMovie, movies.requestTv, movies.request4kMovie, movies.request4kTv]).toEqual([true, false, true, false]);
  // 4K needs a 4K server too.
  expect(capabilities(2, { movie: false, tv: true })).toEqual({
    requestMovie: true, requestTv: true, request4kMovie: false, request4kTv: true, advanced: true, manage: true,
  });
});

test("statuses map to names", () => {
  expect(availability(undefined)).toBeNull();
  expect([1, 2, 3, 4, 5, 6, 7].map(availability)).toEqual(["unknown", "pending", "processing", "partial", "available", "blocklisted", "deleted"]);
  expect([1, 2, 3, 4, 5].map(requestState)).toEqual(["pending", "approved", "declined", "failed", "completed"]);
});

test("results keep movies and shows with their artwork and availability", () => {
  const titles = titlesFrom([
    { id: 1, mediaType: "movie", title: "A", releaseDate: "2021-02-03", posterPath: "/a.jpg", mediaInfo: { status: 5, ratingKey: "100" } },
    { id: 2, mediaType: "tv", name: "B", firstAirDate: "", posterPath: "http://127.0.0.1/b.jpg" },
    { id: 3, mediaType: "person", name: "C" },
  ]);
  expect(titles.map((t) => [t.mediaType, t.title, t.year, t.poster, t.status, t.ratingKey])).toEqual([
    ["movie", "A", 2021, "https://image.tmdb.org/t/p/w500/a.jpg", "available", "100"],
    ["tv", "B", null, "http://127.0.0.1/b.jpg", null, null],
  ]);
  expect(tmdbImage(null, "w500")).toBeNull();
});

test("seasons show what is available and what is requested, without specials", () => {
  const seasons = seasonsFrom({
    id: 8,
    seasons: [{ seasonNumber: 0 }, { seasonNumber: 1, episodeCount: 3 }, { seasonNumber: 2 }, { seasonNumber: 3 }],
    mediaInfo: {
      status: 4,
      seasons: [{ seasonNumber: 1, status: 5 }],
      requests: [
        { id: 1, status: 1, createdAt: "2024-01-01", seasons: [{ seasonNumber: 2 }] },
        { id: 2, status: 2, createdAt: "2024-02-01", seasons: [{ seasonNumber: 2 }] },
        { id: 3, status: 3, createdAt: "2024-03-01", seasons: [{ seasonNumber: 3 }] },
        { id: 4, status: 1, is4k: true, seasons: [{ seasonNumber: 3 }] },
      ],
    },
  });
  expect(seasons.map((s) => [s.number, s.name, s.episodes, s.status, s.requested, s.requested4k])).toEqual([
    [1, "Season 1", 3, "available", null, null],
    [2, "Season 2", null, null, "approved", null],
    [3, "Season 3", null, null, null, "pending"],
  ]);
});

test("a Plex item is the Seerr title only when its TMDB id (or, without ids, its title) agrees", () => {
  const item = (o: Record<string, unknown>) => normalizeMetadata({ ratingKey: "100", type: "movie", title: "Heat", ...o });
  expect(isTmdbTitle(item({ Guid: [{ id: "imdb://tt1" }, { id: "tmdb://949" }] }), "movie", 949, "Heat")).toBe(true);
  // Another server's rating key can name an unrelated title.
  expect(isTmdbTitle(item({ title: "Up", Guid: [{ id: "tmdb://14160" }] }), "movie", 949, "Heat")).toBe(false);
  expect(isTmdbTitle(item({ guid: "com.plexapp.agents.themoviedb://949?lang=en" }), "movie", 949, "Heat")).toBe(true);
  expect(isTmdbTitle(item({}), "movie", 949, "heat")).toBe(true);
  expect(isTmdbTitle(item({}), "tv", 949, "Heat")).toBe(false);
});

test("the Plex copy is checked, but a server that doesn't answer doesn't hold up the page", async () => {
  const [movie] = titlesFrom([{ id: 949, mediaType: "movie", title: "Heat", mediaInfo: { status: 5, ratingKey: "100" } }]);
  const found = normalizeMetadata({ ratingKey: "100", type: "movie", title: "Heat", Guid: [{ id: "tmdb://949" }] });
  expect(await plexCopy({ metadata: async () => found }, movie!)).toBe("100");
  expect(await plexCopy({ metadata: async () => null }, movie!)).toBeNull();
  expect(await plexCopy({ metadata: () => Promise.reject(new Error("offline")) }, movie!)).toBeNull();
  const started = Date.now();
  expect(await plexCopy({ metadata: () => new Promise(() => {}) }, movie!, 30)).toBeNull();
  expect(Date.now() - started).toBeLessThan(1000);
});
