import { expect, test } from "bun:test";

import { SeerrClient, encodeQuery, normalizeUrl } from "./client";

test("addresses are normalized", () => {
  expect(normalizeUrl("seerr.local:5055")).toBe("http://seerr.local:5055");
  expect(normalizeUrl(" https://requests.example.com/ ")).toBe("https://requests.example.com");
  expect(normalizeUrl("http://10.0.0.2:5055/api/v1/")).toBe("http://10.0.0.2:5055");
  expect(normalizeUrl("https://example.com/seerr/?x=1#top")).toBe("https://example.com/seerr");
  expect(() => normalizeUrl("")).toThrow("Enter the address");
  expect(() => normalizeUrl("ftp://example.com")).toThrow("http:// or https://");
});

test("queries encode the characters Seerr rejects", () => {
  expect(encodeQuery({ query: "Kid's Own (Director's Cut)!*", page: 1, skip: undefined })).toBe(
    "query=Kid%27s%20Own%20%28Director%27s%20Cut%29%21%2A&page=1",
  );
});

test("a Plex sign-in is kept and renewed once the session is gone", async () => {
  let sessions = 0;
  let valid = "";
  const seen: string[] = [];
  const server = Bun.serve({
    port: 0,
    hostname: "127.0.0.1",
    async fetch(req) {
      const path = new URL(req.url).pathname;
      seen.push(`${req.method} ${path}`);
      if (path === "/api/v1/auth/plex") {
        expect(await req.json()).toEqual({ authToken: "plex-token" });
        valid = `s${++sessions}`;
        return Response.json({ id: 2 }, { headers: { "Set-Cookie": `connect.sid=${valid}; Path=/; HttpOnly` } });
      }
      if (req.headers.get("cookie") !== `connect.sid=${valid}`) return Response.json({ message: "no" }, { status: 403 });
      return Response.json({ id: 2, displayName: "Alice", permissions: 32 });
    },
  });
  try {
    const client = new SeerrClient(`http://127.0.0.1:${server.port}`, { plexToken: "plex-token" });
    expect(client.authKind).toBe("plex");
    await Promise.all([client.me(), client.me()]);
    expect(sessions).toBe(1);
    valid = "expired";
    // Requests that find the session gone at the same time share one new sign-in.
    const both = await Promise.all([client.me(), client.me(), client.me()]);
    expect(both.map((u) => u.displayName)).toEqual(["Alice", "Alice", "Alice"]);
    expect(sessions).toBe(2);
    expect(seen.filter((s) => s === "POST /api/v1/auth/plex")).toHaveLength(2);
  } finally {
    server.stop(true);
  }
});

test("errors carry Seerr's message, and a bad key reads as a sign-in problem", async () => {
  const server = Bun.serve({
    port: 0,
    hostname: "127.0.0.1",
    fetch(req) {
      if (req.headers.get("x-api-key") !== "good") return Response.json({ message: "Unauthorized" }, { status: 401 });
      return Response.json({ message: "Request for this media already exists." }, { status: 409 });
    },
  });
  try {
    const url = `http://127.0.0.1:${server.port}`;
    await expect(new SeerrClient(url, { apiKey: "bad" }).me()).rejects.toThrow("did not accept the sign-in");
    await expect(new SeerrClient(url, { apiKey: "good" }).createRequest({ mediaType: "movie", mediaId: 1, is4k: false })).rejects.toThrow(
      "Request for this media already exists.",
    );
  } finally {
    server.stop(true);
  }
});
