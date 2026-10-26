// Params, ClientInfo, lenient decoding and plex.tv helpers (the Rust crate has no unit tests
// for these; they pin the behaviour other modules rely on).

import { afterAll, beforeAll, expect, test } from "bun:test";

import { ClientInfo, Params, PlexError, PlexTv, normalizeHomeUser } from "./index";
import { redact } from "./client";
import { boolean, DecodeFailure, list, oneOrMany, optF64, optI64, optString } from "./de";

test("Params builders keep order and skip absent values", () => {
  const p = new Params().set("a", 1).opt("b", null).opt("c", "x y").flag("d", false).flag("e", true);
  p.push("f", true);
  expect(p.entries).toEqual([
    ["a", "1"],
    ["c", "x y"],
    ["e", "1"],
    ["f", "true"],
  ]);
  expect(p.toQueryString()).toBe("a=1&c=x+y&e=1&f=true");
  expect(new Params().isEmpty()).toBe(true);
  const q = new Params().set("z", 0).extend(p);
  expect(q.entries.length).toBe(5);
  expect(p.clone().equals(p)).toBe(true);
  expect(new Params().set("k", "a&b=c/d").toQueryString()).toBe("k=a%26b%3Dc%2Fd");
});

test("ClientInfo headers", () => {
  const info = new ClientInfo("id-1", "Patina", "0.9");
  const h = Object.fromEntries(info.headers());
  expect(h["X-Plex-Client-Identifier"]).toBe("id-1");
  expect(h["X-Plex-Model"]).toBe("desktop");
  expect(["macOS", "Linux", "Windows"]).toContain(h["X-Plex-Platform"]!);
  expect(h["X-Plex-Device"]).toBe(h["X-Plex-Platform"]);
  expect(h["X-Plex-Platform-Version"]).toBe("");
  info.deviceName = "Café"; // not visible ASCII: reqwest drops the header
  expect(info.headerMap()["X-Plex-Device-Name"]).toBeUndefined();
  expect(info.headerMap().Accept).toBe("application/json");
});

test("redact hides the token", () => {
  const out = redact(new URL("http://h:1/a?x=1&X-Plex-Token=secret&y=2"));
  expect(out).not.toContain("secret");
  expect(out).toContain("X-Plex-Token=");
  expect(redact(new URL("http://h:1/a"))).toBe("http://h:1/a");
});

test("lenient scalars", () => {
  expect(optString(5)).toBe("5");
  expect(optString(true)).toBe("true");
  expect(optString(null)).toBeNull();
  expect(optString({})).toBeNull();
  expect(optString([1])).toBeNull();
  expect(optI64("12")).toBe(12);
  expect(optI64(" 3.9 ")).toBe(3);
  expect(optI64("abc")).toBeNull();
  expect(optI64("")).toBeNull();
  expect(optI64(true)).toBe(1);
  expect(optI64(-2.7)).toBe(-2);
  expect(optF64("1.5")).toBe(1.5);
  expect(optF64(false)).toBe(0);
  expect(boolean("1")).toBe(true);
  expect(boolean("True")).toBe(true);
  expect(boolean("yes")).toBe(true);
  expect(boolean("0")).toBe(false);
  expect(boolean(2)).toBe(true);
  expect(boolean(0)).toBe(false);
  expect(boolean(undefined)).toBe(false);
});

test("oneOrMany and list", () => {
  const id = (v: unknown) => v;
  expect(oneOrMany(null, id)).toEqual([]);
  expect(oneOrMany([1, 2], id)).toEqual([1, 2]);
  expect(oneOrMany({ a: 1 }, id)).toEqual([{ a: 1 }]);
  expect(list(undefined, id)).toEqual([]);
  expect(() => list({ a: 1 }, id)).toThrow(DecodeFailure);
});

test("PlexError texts", () => {
  expect(PlexError.noConnection("S").message).toBe('no reachable connection for server "S"');
  expect(PlexError.other("boom").message).toBe("boom");
  expect(PlexError.url("relative URL without a base").message).toBe("invalid URL: relative URL without a base");
  expect(PlexError.statusError("PUT", "/x", 500, "  oops  ").message).toBe("PUT /x failed with HTTP 500: oops");
  expect(PlexError.statusError("PUT", "/x", 500, "x".repeat(300)).message).toBe(`PUT /x failed with HTTP 500: ${"x".repeat(200)}`);
  expect(PlexError.statusError("GET", "/x", 404, "").isNotFound()).toBe(true);
  expect(PlexError.decode("/p", "a.b: bad").message).toBe("could not decode response from /p: a.b: bad");
});

test("home user names", () => {
  const u = normalizeHomeUser({ title: "Kid", protected: "1" });
  expect(u.protected).toBe(true);
  expect(u.admin).toBe(false);
});

let fake: ReturnType<typeof Bun.serve>;
beforeAll(() => {
  fake = Bun.serve({
    port: 0,
    fetch(req) {
      const url = new URL(req.url);
      if (url.pathname === "/api/v2/user") return Response.json({ id: 1, username: "me", authToken: "t", extra: 1 });
      if (url.pathname === "/api/v2/resources") {
        return Response.json([
          { name: "A", provides: "server,client", clientIdentifier: "a", connections: [{ uri: "http://x", local: 1 }] },
          { name: "B", provides: "player", clientIdentifier: "b" },
        ]);
      }
      return new Response("{}");
    },
  });
  process.env.PATINA_PLEX_TV_URL = `http://127.0.0.1:${fake.port}`;
});
afterAll(() => {
  delete process.env.PATINA_PLEX_TV_URL;
  fake.stop(true);
});

test("PlexTv user and servers", async () => {
  const tv = new PlexTv(new ClientInfo("c", "P", "1"), "acct");
  const user = await tv.user();
  expect(user.username).toBe("me");
  expect((user as unknown as Record<string, unknown>).extra).toBe(1);
  const servers = await tv.servers();
  expect(servers.map((s) => s.name)).toEqual(["A"]);
  expect(servers[0]!.connections[0]!.local).toBe(true);
  expect(tv.withToken("other").token()).toBe("other");
  const url = tv.authUrl("ABCD");
  expect(url.startsWith("https://app.plex.tv/auth#?clientID=c&code=ABCD&context%5Bdevice%5D%5Bproduct%5D=P")).toBe(true);
});
